use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::super::system_port::inspect_dpkg_status;
use super::super::*;
use super::helper::*;
use crate::debian::{
    status_snapshot, ArchiveFixture, VerifiedArtifactRelationship, ARCHIVE_EVIDENCE_FILENAME,
    ARCHIVE_PACKAGE_FILENAME,
};
use crate::model::{
    ArtifactSlot, DpkgPackageState, LinuxArtifactIdentity, LinuxFailureCode, LinuxInstallState,
    LinuxOperationKind, PackageSnapshot,
};

#[test]
fn dpkg_status_reader_projects_exact_debian_states() {
    let site = TestSite::new("dpkg-reader");
    assert_eq!(
        inspect_dpkg_status(&site.paths)
            .expect_err("missing dpkg status must not mean package absence")
            .code(),
        LinuxStartupPortErrorCode::PackageStateUnavailable
    );

    for (state_text, expected) in [
        ("installed", DpkgPackageState::Installed),
        ("unpacked", DpkgPackageState::Unpacked),
        ("half-configured", DpkgPackageState::HalfConfigured),
        ("half-installed", DpkgPackageState::HalfInstalled),
        ("triggers-awaited", DpkgPackageState::TriggersAwaited),
        ("triggers-pending", DpkgPackageState::TriggersPending),
    ] {
        write_dpkg_status(&site, "install", "ok", state_text);
        let observation = inspect_dpkg_status(&site.paths).expect("inspect dpkg status");
        assert_eq!(observation.state(), expected);
    }
}

#[test]
fn startup_relationship_revalidates_actual_terminal_package_and_dependencies() {
    let fixture = ArchiveFixture::canonical();
    let mut package_reader = fixture.package.as_slice();
    let relationship = VerifiedArtifactRelationship::verify_package(
        ARCHIVE_PACKAGE_FILENAME,
        ARCHIVE_EVIDENCE_FILENAME,
        &mut package_reader,
        &fixture.evidence,
    )
    .expect("verify canonical package fixture");
    let artifact = relationship
        .to_linux_artifact_identity()
        .expect("project actual package identity");

    let site = TestSite::new("startup-relationship");
    let root_identity = site.create_state_root();
    let mut receipt = prepared_receipt(
        root_identity,
        LinuxOperationKind::Install,
        None,
        Some(artifact.clone()),
    );
    stage_exact_bytes(
        &site,
        &mut receipt,
        ArtifactSlot::Target,
        &fixture.package,
        &fixture.evidence,
    );
    receipt
        .advance(LinuxInstallState::ArtifactsStaged)
        .expect("advance exact package staging");
    receipt
        .advance(LinuxInstallState::Quiesced)
        .expect("advance exact package quiescence");
    receipt
        .advance(LinuxInstallState::PackageMutating)
        .expect("advance exact package mutation");
    receipt
        .record_target_proof(PackageSnapshot::exact_installed(artifact.clone()))
        .expect("record exact package proof");
    receipt
        .advance(LinuxInstallState::PackageVerified)
        .expect("advance exact package proof");
    receipt
        .advance(LinuxInstallState::Completed)
        .expect("complete exact package receipt");
    write_mode(
        &site.paths.dpkg_status_path,
        status_snapshot(&relationship, None, false).as_bytes(),
        0o644,
    );
    let port = LinuxSystemStartupPort::new(
        site.paths.clone(),
        site.paths.manager_component_path.clone(),
        9,
    );
    port.validate_package_relationship(&receipt, &artifact)
        .expect("validate actual terminal package relationship");

    let repair_site = TestSite::new("startup-aborted-repair-relationship");
    let root_identity = repair_site.create_state_root();
    let mut repair = prepared_receipt(
        root_identity,
        LinuxOperationKind::Repair,
        Some(artifact.clone()),
        Some(artifact.clone()),
    );
    stage_exact_bytes(
        &repair_site,
        &mut repair,
        ArtifactSlot::Target,
        &fixture.package,
        &fixture.evidence,
    );
    repair
        .advance(LinuxInstallState::ArtifactsStaged)
        .expect("advance repair staging");
    repair
        .abort_preserved(LinuxFailureCode::ProgramsRunning)
        .expect("abort repair before mutation");
    write_mode(
        &repair_site.paths.dpkg_status_path,
        status_snapshot(&relationship, None, false).as_bytes(),
        0o644,
    );
    LinuxSystemStartupPort::new(
        repair_site.paths.clone(),
        repair_site.paths.manager_component_path.clone(),
        9,
    )
    .validate_package_relationship(&repair, &artifact)
    .expect("aborted repair reuses the identical staged target as source proof");

    let status = status_snapshot(&relationship, None, false);
    let missing_font = status
        .split("\n\n")
        .filter(|paragraph| !paragraph.starts_with("Package: fonts-noto-cjk\n"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut missing_font = missing_font.trim_end_matches('\n').to_owned();
    missing_font.push('\n');
    fs::write(&site.paths.dpkg_status_path, missing_font).expect("remove font dependency record");
    fs::set_permissions(
        &site.paths.dpkg_status_path,
        fs::Permissions::from_mode(0o644),
    )
    .expect("restore dpkg status mode");
    assert_eq!(
        port.validate_package_relationship(&receipt, &artifact)
            .expect_err("missing package dependency must close startup")
            .code(),
        LinuxStartupPortErrorCode::DependencyUnavailable
    );
}

#[test]
fn dpkg_status_reader_accepts_held_installed_and_rejects_inconsistent_holds() {
    let site = TestSite::new("dpkg-held-installed");
    for desired in ["install", "hold"] {
        write_dpkg_status(&site, desired, "ok", "installed");
        assert_eq!(
            inspect_dpkg_status(&site.paths)
                .expect("an installed package may be selected for install or hold")
                .state(),
            DpkgPackageState::Installed
        );
    }

    for (desired, error_flag) in [("hold", "reinstreq"), ("deinstall", "ok")] {
        write_dpkg_status(&site, desired, error_flag, "installed");
        assert_eq!(
            inspect_dpkg_status(&site.paths)
                .expect_err("an inconsistent installed tuple must fail closed")
                .code(),
            LinuxStartupPortErrorCode::PackageStateUnknown
        );
    }
}

#[test]
fn system_component_reader_binds_complete_product_inventories() {
    let fixture = SystemProductFixture::new("component-reader");
    fixture.validate_both();

    let manifest_drift = artifact_with_manifest_hash("26.7.1+38-1", 0x7a, "00".repeat(32));
    assert_eq!(
        fixture
            .manager_port(9)
            .validate_component(LinuxStartupComponent::Manager, &manifest_drift)
            .expect_err("manifest identity drift must fail")
            .code(),
        LinuxStartupPortErrorCode::PackageIdentityChanged
    );

    assert_eq!(
        fixture
            .manager_port(10)
            .validate_component(LinuxStartupComponent::Manager, &fixture.artifact)
            .expect_err("ABI drift must fail")
            .code(),
        LinuxStartupPortErrorCode::ComponentIdentityChanged
    );
}

#[test]
fn system_component_reader_rejects_untracked_tree_entries_and_payload_drift() {
    let manager_extra = SystemProductFixture::new("manager-extra");
    write_mode(
        &manager_extra.manager_root().join("untracked.bin"),
        b"untracked",
        0o644,
    );
    assert_component_changed(
        &manager_extra.manager_port(9),
        LinuxStartupComponent::Manager,
        &manager_extra.artifact,
        "an untracked Manager entry must fail",
    );

    let rime_extra = SystemProductFixture::new("rime-extra");
    write_mode(
        &rime_extra.site.paths.rime_data_root.join("untracked.yaml"),
        b"untracked: true\n",
        0o644,
    );
    assert_component_changed(
        &rime_extra.fcitx_port(9),
        LinuxStartupComponent::FcitxAddon,
        &rime_extra.artifact,
        "an untracked RimeData entry must fail",
    );

    let manager_drift = SystemProductFixture::new("manager-payload-drift");
    fs::write(manager_drift.manager_asset(), b"tampered asset").expect("tamper Manager asset");
    assert_component_changed(
        &manager_drift.manager_port(9),
        LinuxStartupComponent::Manager,
        &manager_drift.artifact,
        "a tracked Manager asset drift must fail",
    );

    let rime_drift = SystemProductFixture::new("rime-payload-drift");
    fs::write(
        rime_drift.site.paths.rime_data_root.join("default.yaml"),
        b"tampered: true\n",
    )
    .expect("tamper RimeData asset");
    assert_component_changed(
        &rime_drift.fcitx_port(9),
        LinuxStartupComponent::FcitxAddon,
        &rime_drift.artifact,
        "a tracked RimeData asset drift must fail",
    );

    let metadata_drift = SystemProductFixture::new("metadata-drift");
    fs::write(
        &metadata_drift.site.paths.fcitx_addon_metadata_path,
        b"tampered metadata\n",
    )
    .expect("tamper Fcitx metadata");
    assert_component_changed(
        &metadata_drift.fcitx_port(9),
        LinuxStartupComponent::FcitxAddon,
        &metadata_drift.artifact,
        "Fcitx metadata drift must fail",
    );
}

#[test]
fn system_component_reader_rejects_directory_and_ffi_identity_drift() {
    let directory_drift = SystemProductFixture::new("directory-drift");
    fs::set_permissions(
        directory_drift.manager_root().join("data"),
        fs::Permissions::from_mode(0o775),
    )
    .expect("drift Manager directory mode");
    assert_component_changed(
        &directory_drift.manager_port(9),
        LinuxStartupComponent::Manager,
        &directory_drift.artifact,
        "Manager directory mode drift must fail",
    );

    let hardlinked_ffi = SystemProductFixture::new("hardlinked-ffi");
    fs::remove_file(&hardlinked_ffi.site.paths.fcitx_ffi_path).expect("remove FFI fixture copy");
    fs::hard_link(
        &hardlinked_ffi.site.paths.manager_ffi_path,
        &hardlinked_ffi.site.paths.fcitx_ffi_path,
    )
    .expect("hardlink FFI fixture copies");
    assert_component_changed(
        &hardlinked_ffi.manager_port(9),
        LinuxStartupComponent::Manager,
        &hardlinked_ffi.artifact,
        "FFI copies sharing an inode must fail",
    );
    assert_component_changed(
        &hardlinked_ffi.fcitx_port(9),
        LinuxStartupComponent::FcitxAddon,
        &hardlinked_ffi.artifact,
        "FFI equivalence applies to both startup components",
    );
}

#[test]
fn system_component_reader_rejects_manifest_schema_and_scope_drift() {
    let unknown_field = SystemProductFixture::new("manifest-unknown-field");
    let artifact = unknown_field.rewrite_manifest(|manifest| {
        manifest["files"][0]["unexpected"] = json!(true);
    });
    assert_eq!(
        unknown_field
            .manager_port(9)
            .validate_component(LinuxStartupComponent::Manager, &artifact)
            .expect_err("unknown manifest record fields must fail")
            .code(),
        LinuxStartupPortErrorCode::PackageIdentityChanged
    );

    let wrong_metadata_scope = SystemProductFixture::new("metadata-scope");
    let artifact = wrong_metadata_scope.rewrite_manifest(|manifest| {
        let path = wrong_metadata_scope
            .site
            .paths
            .fcitx_addon_metadata_path
            .to_str()
            .expect("UTF-8 metadata path");
        let record = manifest["files"]
            .as_array_mut()
            .expect("manifest files")
            .iter_mut()
            .find(|record| record["path"] == path)
            .expect("addon metadata record");
        record["component_id"] = json!("fcitx-input-method-metadata");
    });
    assert_component_changed(
        &wrong_metadata_scope.fcitx_port(9),
        LinuxStartupComponent::FcitxAddon,
        &artifact,
        "metadata component scope drift must fail",
    );

    let wrong_ffi_equivalence = SystemProductFixture::new("ffi-equivalence-scope");
    let artifact = wrong_ffi_equivalence.rewrite_manifest(|manifest| {
        manifest["ffi_equivalence"]["paths"] = json!([
            wrong_ffi_equivalence
                .site
                .paths
                .fcitx_ffi_path
                .to_str()
                .expect("UTF-8 FFI path"),
            wrong_ffi_equivalence
                .site
                .paths
                .manager_ffi_path
                .to_str()
                .expect("UTF-8 FFI path"),
        ]);
    });
    assert_component_changed(
        &wrong_ffi_equivalence.manager_port(9),
        LinuxStartupComponent::Manager,
        &artifact,
        "FFI equivalence paths and order must be exact",
    );
}

fn write_dpkg_status(site: &TestSite, desired: &str, error_flag: &str, state: &str) {
    let value = format!(
        "Package: unrelated\nStatus: install ok installed\nVersion: 1\nArchitecture: arm64\nConffiles:\n /etc/unrelated deadbeef\n\nPackage: radishlex\nStatus: {desired} {error_flag} {state}\nVersion: 26.7.1+38-1\nArchitecture: arm64\n"
    );
    fs::write(&site.paths.dpkg_status_path, value).expect("write dpkg status");
    fs::set_permissions(
        &site.paths.dpkg_status_path,
        fs::Permissions::from_mode(0o644),
    )
    .expect("set dpkg status mode");
}

fn assert_component_changed(
    port: &LinuxSystemStartupPort,
    component: LinuxStartupComponent,
    artifact: &LinuxArtifactIdentity,
    message: &str,
) {
    assert_eq!(
        port.validate_component(component, artifact)
            .expect_err(message)
            .code(),
        LinuxStartupPortErrorCode::ComponentIdentityChanged
    );
}

struct SystemProductFixture {
    site: TestSite,
    artifact: LinuxArtifactIdentity,
}

impl SystemProductFixture {
    fn new(label: &str) -> Self {
        let site = TestSite::new(label);
        let manager_root = site
            .paths
            .manager_component_path
            .parent()
            .expect("Manager root")
            .to_path_buf();
        let manager_asset = manager_root.join("data/flutter_assets/kernel_blob.bin");
        let rime_schema = site
            .paths
            .rime_data_root
            .join("schemas/radishlex_pinyin.schema.yaml");
        for (path, value, mode) in [
            (
                site.paths.manager_component_path.as_path(),
                b"synthetic manager".as_slice(),
                0o755,
            ),
            (
                site.paths.manager_ffi_path.as_path(),
                b"synthetic shared ffi".as_slice(),
                0o644,
            ),
            (
                manager_asset.as_path(),
                b"synthetic asset".as_slice(),
                0o644,
            ),
            (
                site.paths.fcitx_component_path.as_path(),
                b"synthetic addon".as_slice(),
                0o644,
            ),
            (
                site.paths.fcitx_ffi_path.as_path(),
                b"synthetic shared ffi".as_slice(),
                0o644,
            ),
            (
                site.paths.rime_data_root.join("default.yaml").as_path(),
                b"schema_list: []\n".as_slice(),
                0o644,
            ),
            (
                rime_schema.as_path(),
                b"schema_id: radishlex_pinyin\n".as_slice(),
                0o644,
            ),
            (
                site.paths.fcitx_addon_metadata_path.as_path(),
                b"[Addon]\nName=RadishLex\n".as_slice(),
                0o644,
            ),
            (
                site.paths.fcitx_input_method_metadata_path.as_path(),
                b"[InputMethod]\nName=RadishLex\n".as_slice(),
                0o644,
            ),
        ] {
            write_mode(path, value, mode);
        }

        let mut directory_paths = vec![
            manager_root.clone(),
            manager_root.join("lib"),
            manager_root.join("data"),
            manager_root.join("data/flutter_assets"),
            site.paths
                .fcitx_component_path
                .parent()
                .expect("Fcitx component root")
                .to_path_buf(),
            site.paths.rime_data_root.clone(),
            site.paths.rime_data_root.join("schemas"),
            site.paths
                .fcitx_addon_metadata_path
                .parent()
                .expect("addon metadata root")
                .to_path_buf(),
            site.paths
                .fcitx_input_method_metadata_path
                .parent()
                .expect("input method metadata root")
                .to_path_buf(),
        ];
        directory_paths.sort();
        directory_paths.dedup();
        for path in &directory_paths {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .expect("set manifest directory mode");
        }

        let mut files = vec![
            manifest_file_record(
                &site.paths.manager_component_path,
                "manager-bundle",
                0o755,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &site.paths.manager_ffi_path,
                "manager-bundle",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &manager_asset,
                "manager-bundle",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &site.paths.fcitx_component_path,
                "fcitx-addon",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &site.paths.fcitx_ffi_path,
                "fcitx-ffi",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &site.paths.rime_data_root.join("default.yaml"),
                "rime-data",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &rime_schema,
                "rime-data",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &site.paths.fcitx_addon_metadata_path,
                "fcitx-addon-metadata",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
            manifest_file_record(
                &site.paths.fcitx_input_method_metadata_path,
                "fcitx-input-method-metadata",
                0o644,
                site.paths.expected_owner_id,
                site.paths.expected_group_id,
            ),
        ];
        files.sort_by(|left, right| {
            left["path"]
                .as_str()
                .expect("left path")
                .cmp(right["path"].as_str().expect("right path"))
        });
        let directories: Vec<_> = directory_paths
            .iter()
            .map(|path| {
                json!({
                    "gid": site.paths.expected_group_id,
                    "mode": "0755",
                    "path": path.to_str().expect("UTF-8 directory path"),
                    "uid": site.paths.expected_owner_id,
                })
            })
            .collect();
        let ffi_hash = sha256(b"synthetic shared ffi");
        let manifest = json!({
            "directories": directories,
            "ffi_equivalence": {
                "paths": [
                    site.paths.manager_ffi_path.to_str().expect("Manager FFI path"),
                    site.paths.fcitx_ffi_path.to_str().expect("Fcitx FFI path"),
                ],
                "relationship": "same-content-distinct-inode",
                "sha256": ffi_hash,
            },
            "files": files,
            "format_version": 1,
            "inventory_scope": "rootfs-payload-excluding-manifest-self-v1",
            "layout_id": "debian-system-v1",
            "manifest_path": site.paths.product_manifest_path.to_str().expect("manifest path"),
            "owner_model": "package-install-root-v1",
            "product": {
                "build_number": "38",
                "data_layout": "xdg-v1",
                "data_removal": "separate-authorized-flow",
                "debian_architecture": "arm64",
                "debian_codename": "trixie",
                "debian_release": "13",
                "default_removal": "programs-only",
                "distribution_identity": "debian-local-deb-v1",
                "ffi_abi_version": 9,
                "hard_dependencies": [
                    "${shlibs:Depends}",
                    "${misc:Depends}",
                    "fcitx5 (>= 5.1.9)",
                    "librime1t64 (>= 1.13.1)",
                    "fonts-dejavu-core",
                    "fonts-noto-cjk",
                ],
                "install_layout_format_version": 1,
                "manager_application_id": "dev.radishlex.radishlexManager",
                "multiarch_tuple": "aarch64-linux-gnu",
                "package_name": "radishlex",
                "package_version": "26.7.1+38-1",
                "privacy_format_version": 1,
                "product_id": "radishlex-linux",
                "product_manifest_format_version": 1,
                "product_version": "26.7.1",
                "rime_data_manifest_version": 1,
                "rime_data_lock_sha256": "55".repeat(32),
                "rime_schema_id": "radishlex_pinyin",
                "runtime_layout": "debian-system-v1",
                "settings_format_version": 1,
                "userdb_schema_version": 9,
            },
        });
        let manifest_bytes = canonical_manifest_bytes(&manifest);
        write_mode(&site.paths.product_manifest_path, &manifest_bytes, 0o644);
        let artifact = artifact_with_manifest_hash("26.7.1+38-1", 0x7a, sha256(&manifest_bytes));
        Self { site, artifact }
    }

    fn manager_root(&self) -> &Path {
        self.site
            .paths
            .manager_component_path
            .parent()
            .expect("Manager root")
    }

    fn manager_asset(&self) -> PathBuf {
        self.manager_root()
            .join("data/flutter_assets/kernel_blob.bin")
    }

    fn manager_port(&self, abi_version: u32) -> LinuxSystemStartupPort {
        LinuxSystemStartupPort::new(
            self.site.paths.clone(),
            self.site.paths.manager_component_path.clone(),
            abi_version,
        )
    }

    fn fcitx_port(&self, abi_version: u32) -> LinuxSystemStartupPort {
        LinuxSystemStartupPort::new(
            self.site.paths.clone(),
            self.site.paths.fcitx_component_path.clone(),
            abi_version,
        )
    }

    fn validate_both(&self) {
        self.manager_port(9)
            .validate_component(LinuxStartupComponent::Manager, &self.artifact)
            .expect("validate complete Manager product inventory");
        self.fcitx_port(9)
            .validate_component(LinuxStartupComponent::FcitxAddon, &self.artifact)
            .expect("validate complete Fcitx product inventory");
    }

    fn rewrite_manifest(&self, mutate: impl FnOnce(&mut Value)) -> LinuxArtifactIdentity {
        let mut manifest: Value = serde_json::from_slice(
            &fs::read(&self.site.paths.product_manifest_path).expect("read product manifest"),
        )
        .expect("parse product manifest");
        mutate(&mut manifest);
        let bytes = canonical_manifest_bytes(&manifest);
        fs::write(&self.site.paths.product_manifest_path, &bytes)
            .expect("rewrite product manifest");
        fs::set_permissions(
            &self.site.paths.product_manifest_path,
            fs::Permissions::from_mode(0o644),
        )
        .expect("restore product manifest mode");
        artifact_with_manifest_hash("26.7.1+38-1", 0x7a, sha256(&bytes))
    }
}

fn canonical_manifest_bytes(manifest: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_string_pretty(manifest)
        .expect("encode manifest")
        .into_bytes();
    bytes.push(b'\n');
    bytes
}
