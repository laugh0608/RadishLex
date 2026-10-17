use super::*;
use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::sync::atomic::AtomicU64;

#[path = "qualification_reader_request_tests.rs"]
mod request_tests;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
    manifest: model::Manifest,
    catalog: Catalog,
}

impl Fixture {
    fn new() -> Self {
        let (manifest, cases, files) = model_tests::complete_fixture();
        let path = PathBuf::from("/private/tmp").join(format!(
            "radishlex-evidence-reader-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg(target_os = "linux")]
        let path = PathBuf::from("/tmp").join(path.file_name().unwrap());
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        for (relative, bytes) in files {
            let file = path.join(relative);
            let mut directories = Vec::new();
            let mut parent = file.parent().unwrap();
            while parent != path {
                directories.push(parent.to_owned());
                parent = parent.parent().unwrap();
            }
            for parent in directories.into_iter().rev() {
                if !parent.exists() {
                    DirBuilder::new().mode(0o700).create(parent).unwrap();
                }
            }
            fs::write(&file, bytes).unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let entry = ApprovedEntry {
            id: "synthetic-reviewed-entry".into(),
            manifest_sha256: String::new(),
            review_record_sha256: "1".repeat(64),
            source: manifest.source.clone(),
            target: manifest.source.product.clone(),
            environment: manifest.environment.clone(),
            schema: manifest.schema.source.clone(),
            collector: manifest.collector.clone(),
            profile: manifest.profile.clone(),
            cases,
        };
        let mut fixture = Self {
            path,
            manifest,
            catalog: Catalog {
                revision: "test-only-1".into(),
                entries: vec![entry],
                revoked: BTreeSet::new(),
            },
        };
        fixture.publish_test_manifest();
        fixture
    }
    // Test-only catalog changes stand in for an independent reviewed trust root;
    // production callers cannot construct a Catalog or approve arbitrary bytes.
    fn publish_test_manifest(&mut self) {
        let bytes = serde_json::to_vec(&self.manifest).unwrap();
        fs::write(self.path.join("manifest.json"), &bytes).unwrap();
        fs::set_permissions(
            self.path.join("manifest.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        self.catalog.entries[0].manifest_sha256 = digest(&bytes);
    }
    fn inspect(&self) -> EvidenceReview {
        let (control, _) = ReadControl::new(Duration::from_secs(300)).unwrap();
        review_with_catalog(
            &self.catalog,
            "synthetic-reviewed-entry",
            &self.path,
            &self.manifest.source.product,
            &self.manifest.environment,
            None,
            &control,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

fn snapshot(path: &Path) -> BTreeMap<PathBuf, (u64, u32, u64, String)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![path.to_owned()];
    while let Some(item) = pending.pop() {
        let meta = fs::symlink_metadata(&item).unwrap();
        let hash = if meta.is_file() {
            digest(&fs::read(&item).unwrap())
        } else {
            String::new()
        };
        result.insert(
            item.strip_prefix(path).unwrap().to_owned(),
            (meta.ino(), meta.mode(), meta.len(), hash),
        );
        if meta.is_dir() {
            pending.extend(
                fs::read_dir(&item)
                    .unwrap()
                    .map(|value| value.unwrap().path()),
            );
        }
    }
    result
}

#[test]
fn reviewed_pack_is_read_only_and_production_catalog_rejects_it() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.path);
    let report = fixture.inspect();
    assert_eq!(
        report.status(),
        ReviewStatus::ReviewedUnbound,
        "{:?}",
        report.rejection()
    );
    assert_eq!(report.cases().len(), 7);
    assert_eq!(report.files().len(), fixture.manifest.files.len() + 1);
    assert!(report.request_binding().is_none());
    assert_eq!(snapshot(&fixture.path), before);
    let (control, _) = ReadControl::new(Duration::from_secs(30)).unwrap();
    let public = review_evidence(
        "synthetic-reviewed-entry",
        &fixture.path,
        &fixture.manifest.source.product,
        &fixture.manifest.environment,
        None,
        &control,
    );
    assert_eq!(public.rejection(), Some(ReadError::UntrustedSource));
    assert_eq!(snapshot(&fixture.path), before);
}

#[test]
fn missing_rows_and_relabelled_failure_do_not_inherit_reviewed_manifest_trust() {
    let mut fixture = Fixture::new();
    let removed = fixture.manifest.rows.pop().unwrap().case_id;
    fixture.publish_test_manifest();
    let report = fixture.inspect();
    assert_eq!(report.rejection(), Some(ReadError::CoverageMissing));
    assert!(report
        .cases()
        .iter()
        .any(|row| row.case_id == removed && row.status == CaseReviewStatus::Missing));
    let mut fixture = Fixture::new();
    fixture.manifest.rows[0].actual = model::Outcome::Rejected;
    fixture.publish_test_manifest();
    assert_eq!(
        fixture.inspect().rejection(),
        Some(ReadError::AssertionFailed)
    );
}

#[test]
fn every_attachment_is_hashed_and_unknown_objects_are_not_ignored() {
    for change in 0..3 {
        let fixture = Fixture::new();
        let target = fixture.path.join(&fixture.manifest.files[0].path);
        match change {
            0 => {
                let mut bytes = fs::read(&target).unwrap();
                bytes[0] ^= 1;
                fs::write(&target, bytes).unwrap();
            }
            1 => fs::remove_file(&target).unwrap(),
            _ => {
                fs::write(fixture.path.join("unexpected.tmp"), b"not evidence").unwrap();
                fs::set_permissions(
                    fixture.path.join("unexpected.tmp"),
                    fs::Permissions::from_mode(0o600),
                )
                .unwrap();
            }
        }
        let before = snapshot(&fixture.path);
        assert_eq!(
            fixture.inspect().rejection(),
            Some(ReadError::EvidenceChanged)
        );
        assert_eq!(snapshot(&fixture.path), before);
    }
}

#[test]
fn revoked_untrusted_source_environment_and_schema_all_fail_closed() {
    let mut fixture = Fixture::new();
    fixture
        .catalog
        .revoked
        .insert("synthetic-reviewed-entry".into());
    assert_eq!(fixture.inspect().rejection(), Some(ReadError::Revoked));
    fixture.catalog.revoked.clear();
    fixture.catalog.entries[0].environment.os_build.push('x');
    assert_eq!(
        fixture.inspect().rejection(),
        Some(ReadError::EnvironmentUnsupported)
    );
    fixture.catalog.entries[0].environment = fixture.manifest.environment.clone();
    fixture.catalog.entries[0].schema.version += 1;
    assert_eq!(
        fixture.inspect().rejection(),
        Some(ReadError::SchemaUnsupported)
    );
    fixture.catalog.entries[0].schema = fixture.manifest.schema.source.clone();
    fixture.catalog.entries[0].source.programs[0]
        .executable
        .sha256 = "2".repeat(64);
    assert_eq!(
        fixture.inspect().rejection(),
        Some(ReadError::IdentityMismatch)
    );
}

#[test]
fn duplicate_and_unknown_json_fields_are_rejected_even_under_a_test_digest() {
    for unknown in [false, true] {
        let mut fixture = Fixture::new();
        let original = serde_json::to_string(&fixture.manifest).unwrap();
        let prefix = if unknown {
            "\"trusted\":true,".to_owned()
        } else {
            format!("\"format\":{:?},", model::FORMAT)
        };
        let bytes = format!("{{{prefix}{}", &original[1..]).into_bytes();
        fs::write(fixture.path.join("manifest.json"), &bytes).unwrap();
        fixture.catalog.entries[0].manifest_sha256 = digest(&bytes);
        assert_eq!(fixture.inspect().rejection(), Some(ReadError::Malformed));
    }
}

#[test]
fn cancellation_deadline_and_budget_are_not_authorization_or_success() {
    assert!(matches!(
        ReadControl::new(Duration::ZERO),
        Err(ReadError::ResourceLimit)
    ));
    assert!(matches!(
        ReadControl::new(Duration::from_secs(301)),
        Err(ReadError::ResourceLimit)
    ));
    let fixture = Fixture::new();
    let (control, cancel) = ReadControl::new(Duration::from_secs(30)).unwrap();
    cancel.cancel();
    assert_eq!(
        review_with_catalog(
            &fixture.catalog,
            "synthetic-reviewed-entry",
            &fixture.path,
            &fixture.manifest.source.product,
            &fixture.manifest.environment,
            None,
            &control
        )
        .rejection(),
        Some(ReadError::Cancelled)
    );
    let expired = ReadControl {
        deadline: Instant::now() - Duration::from_secs(1),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    assert_eq!(
        review_with_catalog(
            &fixture.catalog,
            "synthetic-reviewed-entry",
            &fixture.path,
            &fixture.manifest.source.product,
            &fixture.manifest.environment,
            None,
            &expired
        )
        .rejection(),
        Some(ReadError::DeadlineExceeded)
    );
}

#[test]
fn largest_supported_pack_is_streamed_and_preserved() {
    let mut fixture = Fixture::new();
    let original_bytes: u64 = fixture.manifest.files.iter().map(|file| file.bytes).sum();
    let payload_digest = zero_digest(model::MAX_FILE_BYTES);
    for index in 0..16 {
        fixture.manifest.files.push(model::FileEntry {
            path: format!("payload-{index:02}.bin"),
            bytes: model::MAX_FILE_BYTES,
            sha256: payload_digest.clone(),
            role: model::FileRole::SyntheticData,
        });
    }
    // Exercise simultaneous manifest, attachment-count and total-byte ceilings.
    // JSON whitespace is legal; pad to exactly 1 MiB without inventing fields.
    let payload_index = fixture.manifest.files.len() - 1;
    let final_length = model::MAX_FILE_BYTES - original_bytes - model::MAX_MANIFEST_BYTES as u64;
    fixture.manifest.files[payload_index].bytes = final_length;
    fixture.manifest.files[payload_index].sha256 = zero_digest(final_length);
    let empty_digest = digest(b"");
    while fixture.manifest.files.len() < model::MAX_FILES {
        let index = fixture.manifest.files.len();
        fixture.manifest.files.push(model::FileEntry {
            path: format!("empty-{index:04}.bin"),
            bytes: 0,
            sha256: empty_digest.clone(),
            role: model::FileRole::SyntheticData,
        });
    }
    for file in fixture
        .manifest
        .files
        .iter()
        .filter(|file| file.path.starts_with("payload-") || file.path.starts_with("empty-"))
    {
        let path = fixture.path.join(&file.path);
        let output = fs::File::create(&path).unwrap();
        output
            .set_permissions(fs::Permissions::from_mode(0o600))
            .unwrap();
        output.set_len(file.bytes).unwrap();
    }
    publish_maximum_manifest(&mut fixture);
    let total = fixture
        .manifest
        .files
        .iter()
        .map(|file| file.bytes)
        .sum::<u64>()
        + fs::metadata(fixture.path.join("manifest.json"))
            .unwrap()
            .len();
    assert_eq!(total, model::MAX_TOTAL_BYTES);
    assert_eq!(fixture.manifest.files.len(), model::MAX_FILES);
    let started = Instant::now();
    let report = fixture.inspect();
    assert_eq!(
        report.status(),
        ReviewStatus::ReviewedUnbound,
        "{:?}",
        report.rejection()
    );
    assert_eq!(
        report.files().iter().map(|file| file.bytes).sum::<u64>(),
        model::MAX_TOTAL_BYTES
    );
    eprintln!(
        "reader streamed {} bytes, 4096 attachments, 1 MiB manifest in {:?}; no attachment buffers retained",
        total,
        started.elapsed()
    );
    // A changed catalog cannot turn a pack beyond the fixed budget into success.
    let last = &mut fixture.manifest.files[payload_index];
    last.bytes += 1;
    fs::OpenOptions::new()
        .write(true)
        .open(fixture.path.join(&last.path))
        .unwrap()
        .set_len(last.bytes)
        .unwrap();
    last.sha256 = zero_digest(last.bytes);
    publish_maximum_manifest(&mut fixture);
    assert_eq!(
        fixture.inspect().rejection(),
        Some(ReadError::ResourceLimit)
    );
}

fn publish_maximum_manifest(fixture: &mut Fixture) {
    let mut bytes = serde_json::to_vec(&fixture.manifest).unwrap();
    assert!(bytes.len() <= model::MAX_MANIFEST_BYTES);
    bytes.resize(model::MAX_MANIFEST_BYTES, b' ');
    fs::write(fixture.path.join("manifest.json"), &bytes).unwrap();
    fixture.catalog.entries[0].manifest_sha256 = digest(&bytes);
}

fn zero_digest(length: u64) -> String {
    let buffer = [0u8; 64 * 1024];
    let mut hash = Sha256::new();
    let mut remaining = length;
    while remaining > 0 {
        let count = remaining.min(buffer.len() as u64) as usize;
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    hex(&hash.finalize())
}
