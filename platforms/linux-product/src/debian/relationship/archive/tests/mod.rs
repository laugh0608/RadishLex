mod fixture;

use std::io::Read;

use serde_json::{json, Value};

use crate::debian::relationship::{
    DebianRelationshipErrorCode, VerifiedArtifactRelationship, PRODUCT_MANIFEST_PATH,
};
use fixture::{
    build_ar, build_evidence, build_md5sums, build_tar, canonical_parts, pretty_json,
    ArchiveFixture, PanicReader, TarEntrySpec, EVIDENCE_FILENAME, MAX_PACKAGE_BYTES,
    PACKAGE_FILENAME, VERSION,
};

fn verify_error(package: &[u8], evidence: &[u8]) -> DebianRelationshipErrorCode {
    let mut reader = package;
    VerifiedArtifactRelationship::verify_package(
        PACKAGE_FILENAME,
        EVIDENCE_FILENAME,
        &mut reader,
        evidence,
    )
    .expect_err("invalid package relationship must fail")
    .code()
}

#[test]
fn canonical_deb_stream_is_the_only_verified_artifact_entry() {
    let fixture = ArchiveFixture::canonical();
    let mut reader = fixture.package.as_slice();
    let verified = VerifiedArtifactRelationship::verify_package(
        PACKAGE_FILENAME,
        EVIDENCE_FILENAME,
        &mut reader,
        &fixture.evidence,
    )
    .expect("canonical Debian package stream");
    assert_eq!(verified.package_name(), "radishlex");
    assert_eq!(verified.package_version(), VERSION);
    assert_eq!(verified.package_size(), fixture.package.len() as u64);
    assert!(
        reader.is_empty(),
        "verification consumes the exact package stream"
    );
}

#[test]
fn package_a_cannot_be_combined_with_detached_b_manifest_evidence() {
    let mut parts_a = canonical_parts(VERSION);
    let mut manifest_a: Value = serde_json::from_slice(&parts_a.manifest).unwrap();
    manifest_a["product"]["rime_data_lock_sha256"] = json!("55".repeat(32));
    parts_a.manifest = pretty_json(&manifest_a);
    parts_a
        .data_entries
        .iter_mut()
        .find(|entry| entry.absolute_path() == PRODUCT_MANIFEST_PATH)
        .unwrap()
        .content = parts_a.manifest.clone();
    let fixture_a = ArchiveFixture::from_parts(parts_a, 0);
    let fixture_b = ArchiveFixture::canonical();

    let actual: Value = serde_json::from_slice(&fixture_a.evidence).unwrap();
    let mut mixed: Value = serde_json::from_slice(&fixture_b.evidence).unwrap();
    mixed["package"]["size"] = actual["package"]["size"].clone();
    mixed["package"]["sha256"] = actual["package"]["sha256"].clone();
    mixed["archive"]["members"] = actual["archive"]["members"].clone();
    let mixed = pretty_json(&mixed);

    assert_eq!(
        verify_error(&fixture_a.package, &mixed),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
}

#[test]
fn custom_maintainer_and_configuration_members_are_rejected() {
    for forbidden in [
        "preinst",
        "postinst",
        "prerm",
        "postrm",
        "config",
        "templates",
        "conffiles",
        "triggers",
    ] {
        let mut parts = canonical_parts(VERSION);
        parts.control_entries.push(TarEntrySpec::file(
            &format!("/{forbidden}"),
            0o644,
            b"forbidden".to_vec(),
        ));
        let fixture = ArchiveFixture::from_parts(parts, 0);
        assert_eq!(
            fixture.verify().unwrap_err().code(),
            DebianRelationshipErrorCode::ArtifactContentMismatch,
            "{forbidden} must remain absent"
        );
    }
}

#[test]
fn md5sums_must_exactly_describe_the_actual_regular_payload() {
    let mut parts = canonical_parts(VERSION);
    let mut md5sums = build_md5sums(&parts.data_entries);
    md5sums[0] = if md5sums[0] == b'0' { b'1' } else { b'0' };
    parts.md5sums = md5sums.clone();
    parts
        .control_entries
        .iter_mut()
        .find(|entry| entry.absolute_path() == "/md5sums")
        .expect("md5sums control member")
        .content = md5sums;
    assert_eq!(
        ArchiveFixture::from_parts(parts, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
}

#[test]
fn installed_size_must_be_derived_from_the_actual_regular_payload() {
    let mut parts = canonical_parts(VERSION);
    let control = String::from_utf8(parts.control.clone()).expect("UTF-8 control");
    let declared = control
        .lines()
        .find_map(|line| line.strip_prefix("Installed-Size: "))
        .expect("Installed-Size field")
        .parse::<u64>()
        .expect("decimal Installed-Size");
    parts.control = control
        .replace(
            &format!("Installed-Size: {declared}\n"),
            &format!("Installed-Size: {}\n", declared + 1),
        )
        .into_bytes();
    parts
        .control_entries
        .iter_mut()
        .find(|entry| entry.absolute_path() == "/control")
        .expect("control member")
        .content = parts.control.clone();
    assert_eq!(
        ArchiveFixture::from_parts(parts, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
}

#[test]
fn actual_payload_must_match_manifest_without_extra_or_missing_files() {
    let mut extra = canonical_parts(VERSION);
    extra.data_entries.push(TarEntrySpec::file(
        "/usr/share/radishlex/rime/zz-extra.yaml",
        0o644,
        b"extra".to_vec(),
    ));
    extra.data_entries.sort_by_key(TarEntrySpec::absolute_path);
    assert_eq!(
        ArchiveFixture::from_parts(extra, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut missing = canonical_parts(VERSION);
    missing.data_entries.retain(|entry| {
        entry.absolute_path() != "/usr/share/radishlex/rime/radishlex_pinyin.schema.yaml"
    });
    assert_eq!(
        ArchiveFixture::from_parts(missing, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut missing_manifest = canonical_parts(VERSION);
    missing_manifest
        .data_entries
        .retain(|entry| entry.absolute_path() != PRODUCT_MANIFEST_PATH);
    assert_eq!(
        ArchiveFixture::from_parts(missing_manifest, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut duplicate_manifest = canonical_parts(VERSION);
    let manifest = duplicate_manifest
        .data_entries
        .iter()
        .find(|entry| entry.absolute_path() == PRODUCT_MANIFEST_PATH)
        .unwrap()
        .clone();
    duplicate_manifest.data_entries.push(manifest);
    duplicate_manifest
        .data_entries
        .sort_by_key(TarEntrySpec::absolute_path);
    assert_eq!(
        ArchiveFixture::from_parts(duplicate_manifest, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
}

#[test]
fn actual_payload_file_size_and_hash_must_match_manifest_records() {
    let payload_path = "/usr/share/radishlex/rime/radishlex_pinyin.schema.yaml";
    let mut changed_hash = canonical_parts(VERSION);
    let content = &mut changed_hash
        .data_entries
        .iter_mut()
        .find(|entry| entry.absolute_path() == payload_path)
        .unwrap()
        .content;
    content[0] ^= 1;
    assert_eq!(
        ArchiveFixture::from_parts(changed_hash, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut changed_size = canonical_parts(VERSION);
    changed_size
        .data_entries
        .iter_mut()
        .find(|entry| entry.absolute_path() == payload_path)
        .unwrap()
        .content
        .push(b'x');
    assert_eq!(
        ArchiveFixture::from_parts(changed_size, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
}

#[test]
fn package_member_hash_size_and_order_are_bound_to_actual_ar_bytes() {
    let fixture = ArchiveFixture::canonical();

    let mut hash_evidence: Value = serde_json::from_slice(&fixture.evidence).unwrap();
    hash_evidence["archive"]["members"][1]["sha256"] = json!("66".repeat(32));
    assert_eq!(
        verify_error(&fixture.package, &pretty_json(&hash_evidence)),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut size_evidence: Value = serde_json::from_slice(&fixture.evidence).unwrap();
    let size = size_evidence["archive"]["members"][2]["size"]
        .as_u64()
        .unwrap();
    size_evidence["archive"]["members"][2]["size"] = json!(size + 1);
    assert_eq!(
        verify_error(&fixture.package, &pretty_json(&size_evidence)),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut reversed_members = fixture.members.clone();
    reversed_members.swap(1, 2);
    let reversed_package = build_ar(&reversed_members);
    let reversed_evidence = build_evidence(
        &reversed_package,
        &reversed_members,
        &fixture.control,
        &fixture.md5sums,
        &fixture.manifest,
    );
    assert_eq!(
        verify_error(&reversed_package, &reversed_evidence),
        DebianRelationshipErrorCode::ArtifactEvidenceInvalid
    );
}

#[test]
fn noncanonical_ar_member_size_field_is_rejected_even_with_rewritten_package_digest() {
    let mut fixture = ArchiveFixture::canonical();
    let control_header = 8 + 60 + 4;
    let size_field = control_header + 48;
    assert_eq!(&fixture.package[size_field..size_field + 5], b"10240");
    fixture.package[size_field..size_field + 10].copy_from_slice(b"010240    ");
    fixture.rebuild_evidence();
    assert_eq!(
        fixture.verify().unwrap_err().code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
}

#[test]
fn tar_padding_path_type_mode_owner_order_and_duplicates_are_rejected() {
    let padded = ArchiveFixture::from_parts(canonical_parts(VERSION), b'x');
    assert_eq!(
        padded.verify().unwrap_err().code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut invalid_path = canonical_parts(VERSION);
    invalid_path.data_entries[0].raw_path = "./../escape".to_owned();
    assert_structural_data_rejection(invalid_path);

    let mut invalid_type = canonical_parts(VERSION);
    let file = invalid_type
        .data_entries
        .iter_mut()
        .find(|entry| entry.kind == b'0')
        .unwrap();
    file.kind = b'2';
    assert_structural_data_rejection(invalid_type);

    let mut invalid_mode = canonical_parts(VERSION);
    invalid_mode
        .data_entries
        .iter_mut()
        .find(|entry| entry.kind == b'0')
        .unwrap()
        .mode = 0o600;
    assert_structural_data_rejection(invalid_mode);

    let mut invalid_owner = canonical_parts(VERSION);
    invalid_owner.data_entries[0].uid = 1;
    assert_structural_data_rejection(invalid_owner);

    let mut invalid_order = canonical_parts(VERSION);
    invalid_order.data_entries.swap(0, 1);
    assert_structural_data_rejection(invalid_order);

    let mut duplicate = canonical_parts(VERSION);
    duplicate
        .data_entries
        .insert(1, duplicate.data_entries[0].clone());
    assert_structural_data_rejection(duplicate);
}

fn assert_structural_data_rejection(parts: fixture::CanonicalParts) {
    assert_eq!(
        ArchiveFixture::from_parts(parts, 0)
            .verify()
            .unwrap_err()
            .code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
}

#[test]
fn nonzero_trailer_and_bytes_after_the_third_member_are_rejected() {
    let parts = canonical_parts(VERSION);
    let last = parts.data_entries.len() - 1;
    let mut data_tar = build_tar(&parts.data_entries, 0);
    let trailer_index = data_tar.len() - 1;
    data_tar[trailer_index] = b'x';
    let control_tar = build_tar(&parts.control_entries, 0);
    let malformed = ArchiveFixture::from_member_bytes(
        parts.control,
        parts.md5sums,
        parts.manifest,
        control_tar,
        data_tar,
    );
    assert_eq!(
        malformed.verify().unwrap_err().code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );

    let mut trailing = ArchiveFixture::canonical();
    trailing.package.extend_from_slice(b"trailing");
    trailing.rebuild_evidence();
    assert_eq!(
        trailing.verify().unwrap_err().code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
    assert!(last > 0);
}

#[test]
fn oversized_sidecar_identity_fails_before_the_reader_is_touched() {
    let fixture = ArchiveFixture::canonical();
    let mut evidence: Value = serde_json::from_slice(&fixture.evidence).unwrap();
    evidence["package"]["size"] = json!(MAX_PACKAGE_BYTES + 1);
    let evidence = pretty_json(&evidence);
    let mut reader = PanicReader { reads: 0 };
    let error = VerifiedArtifactRelationship::verify_package(
        PACKAGE_FILENAME,
        EVIDENCE_FILENAME,
        &mut reader,
        &evidence,
    )
    .expect_err("oversized identity must fail closed");
    assert_eq!(
        error.code(),
        DebianRelationshipErrorCode::ArtifactContentMismatch
    );
    assert_eq!(reader.reads, 0);
}

#[test]
fn streaming_reader_stops_at_its_byte_bound_without_allocating_the_source() {
    struct RepeatingReader;

    impl Read for RepeatingReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            buffer.fill(b'x');
            Ok(buffer.len())
        }
    }

    let mut reader = super::ar::PackageReader::with_test_limit(RepeatingReader, 64);
    let mut output = Vec::new();
    assert!(reader.read_to_end(&mut output).is_err());
    assert_eq!(output, vec![b'x'; 64]);
}
