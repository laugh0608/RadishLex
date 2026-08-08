use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Value};

use super::super::*;

pub(super) const DEPENDENCIES: [&str; 6] = [
    "libc6 (>= 2.34)",
    "libstdc++6 (>= 11)",
    "fcitx5 (>= 5.1.9)",
    "librime1t64 (>= 1.13.1)",
    "fonts-dejavu-core",
    "fonts-noto-cjk",
];

pub(crate) struct ArtifactFixture {
    pub(super) package_filename: String,
    pub(super) evidence_filename: String,
    pub(super) package: Vec<u8>,
    package_content: PackageContentIdentity,
    pub(super) control: Vec<u8>,
    pub(super) manifest: Vec<u8>,
    pub(super) evidence: Vec<u8>,
}

impl ArtifactFixture {
    pub(crate) fn new(version: &str) -> Self {
        Self::build(version, &DEPENDENCIES, 9)
    }

    pub(super) fn with_dependencies(version: &str, dependencies: &[&str]) -> Self {
        Self::build(version, dependencies, 9)
    }

    pub(super) fn with_ffi_abi(version: &str, ffi_abi_version: u32) -> Self {
        Self::build(version, &DEPENDENCIES, ffi_abi_version)
    }

    pub(super) fn with_rime_lock(version: &str, rime_data_lock_sha256: &str) -> Self {
        Self::build_with_rime_lock(version, &DEPENDENCIES, 9, rime_data_lock_sha256)
    }

    fn build(version: &str, dependencies: &[&str], ffi_abi_version: u32) -> Self {
        Self::build_with_rime_lock(version, dependencies, ffi_abi_version, &"44".repeat(32))
    }

    fn build_with_rime_lock(
        version: &str,
        dependencies: &[&str],
        ffi_abi_version: u32,
        rime_data_lock_sha256: &str,
    ) -> Self {
        let package_filename = format!("radishlex_{version}_arm64.deb");
        let evidence_filename = format!("{package_filename}.evidence.json");
        let package = format!("synthetic canonical package {version}\n").into_bytes();
        let package_content = PackageContentIdentity::from_stable_digest(
            package.len() as u64,
            sha256_bytes(&package),
        )
        .expect("build package content identity");
        let (upstream, debian_revision) = version.rsplit_once('-').expect("Debian revision");
        let (product_version, build_number) = upstream.rsplit_once('+').expect("build number");
        assert_eq!(debian_revision, "1", "manifest profile fixes revision 1");
        let ffi_sha256 = "aa".repeat(32);
        let files = vec![
            manifest_file(
                "fcitx-ffi",
                "0644",
                "/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so",
                &ffi_sha256,
            ),
            manifest_file(
                "fcitx-addon",
                "0644",
                "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so",
                &"01".repeat(32),
            ),
            manifest_file(
                "manager-bundle",
                "0644",
                "/usr/lib/aarch64-linux-gnu/radishlex/manager/data/flutter_assets/fonts/MaterialIcons-Regular.otf",
                &"02".repeat(32),
            ),
            manifest_file(
                "manager-bundle",
                "0644",
                "/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/libradishlex_ime_ffi.so",
                &ffi_sha256,
            ),
            manifest_file(
                "manager-bundle",
                "0755",
                "/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager",
                &"03".repeat(32),
            ),
            manifest_file(
                "manager-desktop-entry",
                "0644",
                "/usr/share/applications/dev.radishlex.radishlexManager.desktop",
                &"04".repeat(32),
            ),
            manifest_file(
                "product-license",
                "0644",
                "/usr/share/doc/radishlex/copyright",
                &"05".repeat(32),
            ),
            manifest_file(
                "fcitx-addon-metadata",
                "0644",
                "/usr/share/fcitx5/addon/radishlex.conf",
                &"06".repeat(32),
            ),
            manifest_file(
                "fcitx-input-method-metadata",
                "0644",
                "/usr/share/fcitx5/inputmethod/radishlex.conf",
                &"07".repeat(32),
            ),
            manifest_file(
                "manager-icon",
                "0644",
                "/usr/share/icons/hicolor/scalable/apps/dev.radishlex.radishlexManager.svg",
                &"08".repeat(32),
            ),
            manifest_file(
                "input-method-icon",
                "0644",
                "/usr/share/icons/hicolor/scalable/apps/fcitx-radishlex.svg",
                &"09".repeat(32),
            ),
            manifest_file(
                "rime-data",
                "0644",
                "/usr/share/radishlex/rime/radishlex_pinyin.schema.yaml",
                &"10".repeat(32),
            ),
        ];
        let directories = manifest_directories(&files);
        let manifest_value = json!({
            "directories": directories,
            "ffi_equivalence": {
                "paths": [
                    "/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/libradishlex_ime_ffi.so",
                    "/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so"
                ],
                "relationship": "same-content-distinct-inode",
                "sha256": ffi_sha256
            },
            "files": files,
            "format_version": 1,
            "inventory_scope": "rootfs-payload-excluding-manifest-self-v1",
            "layout_id": "debian-system-v1",
            "manifest_path": PRODUCT_MANIFEST_PATH,
            "owner_model": "package-install-root-v1",
            "product": {
                "build_number": build_number,
                "data_layout": "xdg-v1",
                "data_removal": "separate-authorized-flow",
                "debian_architecture": "arm64",
                "debian_codename": "trixie",
                "debian_release": "13",
                "default_removal": "programs-only",
                "distribution_identity": DISTRIBUTION_IDENTITY,
                "ffi_abi_version": ffi_abi_version,
                "hard_dependencies": [
                    "${shlibs:Depends}",
                    "${misc:Depends}",
                    "fcitx5 (>= 5.1.9)",
                    "librime1t64 (>= 1.13.1)",
                    "fonts-dejavu-core",
                    "fonts-noto-cjk"
                ],
                "install_layout_format_version": 1,
                "manager_application_id": "dev.radishlex.radishlexManager",
                "multiarch_tuple": "aarch64-linux-gnu",
                "package_name": "radishlex",
                "package_version": version,
                "privacy_format_version": 1,
                "product_id": "radishlex-linux",
                "product_manifest_format_version": 1,
                "product_version": product_version,
                "rime_data_lock_sha256": rime_data_lock_sha256,
                "rime_data_manifest_version": 1,
                "rime_schema_id": "radishlex_pinyin",
                "runtime_layout": "debian-system-v1",
                "settings_format_version": 1,
                "userdb_schema_version": 9
            }
        });
        let mut manifest =
            serde_json::to_vec_pretty(&manifest_value).expect("encode product manifest");
        manifest.push(b'\n');
        let dependency_field = dependencies.join(", ");
        let control = format!(
            "Package: radishlex\n\
             Version: {version}\n\
             Architecture: arm64\n\
             Maintainer: RadishLex <laugh0608@foxmail.com>\n\
             Section: utils\n\
             Priority: optional\n\
             Installed-Size: 42\n\
             Depends: {dependency_field}\n\
             Homepage: https://github.com/laugh0608/RadishLex\n\
             Description: Local-first Chinese input system for Fcitx5\n\
             \x20RadishLex combines a native Fcitx5 addon with a local management application.\n\
             \x20This template describes the Debian 13 ARM64 local acceptance carrier only.\n"
        )
        .into_bytes();
        let evidence_value = ArtifactEvidenceV1 {
            archive: ArchiveEvidence {
                compression: "none".to_owned(),
                format: "debian-binary-2.0-ar-v1".to_owned(),
                members: vec![
                    ArchiveMemberEvidence {
                        name: "debian-binary".to_owned(),
                        sha256: sha256_bytes(b"2.0\n"),
                        size: 4,
                    },
                    ArchiveMemberEvidence {
                        name: "control.tar".to_owned(),
                        sha256: "11".repeat(32),
                        size: 1024,
                    },
                    ArchiveMemberEvidence {
                        name: "data.tar".to_owned(),
                        sha256: "22".repeat(32),
                        size: 2048,
                    },
                ],
                source_date_epoch: 0,
                tar_format: "ustar".to_owned(),
            },
            control: ControlEvidence {
                architecture: "arm64".to_owned(),
                depends: dependencies
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                installed_size_kib: 42,
                md5sums_sha256: "33".repeat(32),
                package: "radishlex".to_owned(),
                sha256: sha256_bytes(&control),
                version: version.to_owned(),
            },
            dependency_analysis: DependencyAnalysisEvidence {
                loader_owner: "libc6:arm64".to_owned(),
                private_libraries: vec![
                    "libflutter_linux_gtk.so".to_owned(),
                    "libradishlex_ime_ffi.so".to_owned(),
                ],
                profile: DEPENDENCY_ANALYSIS_PROFILE.to_owned(),
                stderr_policy: "exact-private-libraries-and-libc6-usrmerge-v1".to_owned(),
                tool: "dpkg-shlibdeps".to_owned(),
            },
            distribution_identity: DISTRIBUTION_IDENTITY.to_owned(),
            format_version: 1,
            package: PackageEvidence {
                filename: package_filename.clone(),
                sha256: sha256_bytes(&package),
                size: package.len() as u64,
            },
            product_manifest: ProductManifestEvidence {
                format_version: 1,
                path: PRODUCT_MANIFEST_PATH.to_owned(),
                sha256: sha256_bytes(&manifest),
            },
        };
        let mut evidence = serde_json::to_vec_pretty(&evidence_value).expect("encode evidence");
        evidence.push(b'\n');
        Self {
            package_filename,
            evidence_filename,
            package,
            package_content,
            control,
            manifest,
            evidence,
        }
    }

    pub(super) fn input(&self) -> ArtifactRelationshipInput<'_> {
        ArtifactRelationshipInput {
            package_filename: &self.package_filename,
            evidence_filename: &self.evidence_filename,
            package_content: &self.package_content,
            binary_control_bytes: &self.control,
            product_manifest_bytes: &self.manifest,
            evidence_bytes: &self.evidence,
        }
    }

    pub(crate) fn verify(&self) -> VerifiedArtifactRelationship {
        VerifiedArtifactRelationship::verify(self.input()).expect("verify artifact relationship")
    }
}

pub(crate) fn status_snapshot(
    artifact: &VerifiedArtifactRelationship,
    status_override: Option<(&str, &str)>,
    folded_depends: bool,
) -> String {
    let dependencies = artifact
        .dependencies()
        .iter()
        .map(DirectDependency::to_control_text)
        .collect::<Vec<_>>();
    let depends = if folded_depends {
        format!(
            "{}, {},\n {}",
            dependencies[0],
            dependencies[1],
            dependencies[2..].join(", ")
        )
    } else {
        dependencies.join(", ")
    };
    let mut value = format!(
        "Package: radishlex\n\
         Status: install ok installed\n\
         Architecture: arm64\n\
         Version: {}\n\
         Depends: {}\n\
         Description: product\n\n",
        artifact.package_version(),
        depends
    );
    for (package, architecture, version) in [
        ("libc6", "arm64", "2.36"),
        ("libstdc++6", "arm64", "12.2"),
        ("fcitx5", "arm64", "5.1.10"),
        ("librime1t64", "arm64", "1.13.1"),
        ("fonts-dejavu-core", "all", "2.37"),
        ("fonts-noto-cjk", "all", "1:20240730+repack1-1"),
    ] {
        let status = status_override
            .filter(|(candidate, _)| *candidate == package)
            .map_or("install ok installed", |(_, status)| status);
        value.push_str(&dependency_paragraph(
            package,
            architecture,
            version,
            status,
        ));
    }
    value
}

pub(super) fn dependency_paragraph(
    package: &str,
    architecture: &str,
    version: &str,
    status: &str,
) -> String {
    format!(
        "Package: {package}\n\
         Status: {status}\n\
         Architecture: {architecture}\n\
         Version: {version}\n\
         Multi-Arch: foreign\n\n"
    )
}

pub(super) fn remove_package_paragraph(value: &str, package: &str) -> String {
    value
        .split("\n\n")
        .filter(|paragraph| !paragraph.starts_with(&format!("Package: {package}\n")))
        .filter(|paragraph| !paragraph.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n\n"
}

pub(super) fn assert_relationship_error(
    artifact: &VerifiedArtifactRelationship,
    status: &str,
    expected: DebianRelationshipErrorCode,
) {
    let snapshot = DpkgStatusSnapshot::parse(status.as_bytes()).expect("parse dpkg status");
    assert_eq!(
        snapshot
            .validate_installed_relationship(artifact)
            .expect_err("relationship must fail")
            .code(),
        expected
    );
}

pub(super) fn assert_dependency_error(
    artifact: &VerifiedArtifactRelationship,
    status: &str,
    expected: DebianRelationshipErrorCode,
) {
    let snapshot = DpkgStatusSnapshot::parse(status.as_bytes()).expect("parse dpkg status");
    assert_eq!(
        snapshot
            .validate_dependencies(artifact)
            .expect_err("dependency preflight must fail")
            .code(),
        expected
    );
}

pub(super) fn manifest_file(component_id: &str, mode: &str, path: &str, sha256: &str) -> Value {
    json!({
        "component_id": component_id,
        "gid": 0,
        "mode": mode,
        "path": path,
        "sha256": sha256,
        "size": 123,
        "uid": 0
    })
}

pub(super) fn manifest_directories(files: &[Value]) -> Vec<Value> {
    let mut paths = BTreeSet::new();
    for path in files
        .iter()
        .map(|record| record["path"].as_str().expect("manifest file path"))
        .chain(std::iter::once(PRODUCT_MANIFEST_PATH))
    {
        let mut parent = Path::new(path).parent();
        while let Some(directory) = parent {
            let value = directory.to_str().expect("UTF-8 manifest directory");
            if value == "/" {
                break;
            }
            paths.insert(value.to_owned());
            parent = directory.parent();
        }
    }
    paths
        .into_iter()
        .map(|path| json!({"gid": 0, "mode": "0755", "path": path, "uid": 0}))
        .collect()
}

pub(super) fn changed_manifest_inputs(
    fixture: &ArtifactFixture,
    value: &Value,
) -> (Vec<u8>, Vec<u8>) {
    let mut manifest = serde_json::to_vec_pretty(value).expect("encode changed manifest");
    manifest.push(b'\n');
    let mut evidence: Value =
        serde_json::from_slice(&fixture.evidence).expect("parse artifact evidence");
    evidence["product_manifest"]["sha256"] = json!(sha256_bytes(&manifest));
    let mut evidence = serde_json::to_vec_pretty(&evidence).expect("encode artifact evidence");
    evidence.push(b'\n');
    (manifest, evidence)
}

pub(super) fn assert_manifest_error(fixture: &ArtifactFixture, value: &Value) {
    let (manifest, evidence) = changed_manifest_inputs(fixture, value);
    let mut input = fixture.input();
    input.product_manifest_bytes = &manifest;
    input.evidence_bytes = &evidence;
    assert_eq!(
        VerifiedArtifactRelationship::verify(input)
            .expect_err("invalid product manifest relationship must fail")
            .code(),
        DebianRelationshipErrorCode::ProductManifestInvalid
    );
}

pub(super) trait ByteReplace {
    fn replace(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
}

impl ByteReplace for Vec<u8> {
    fn replace(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let position = self
            .windows(from.len())
            .position(|window| window == from)
            .expect("fixture bytes contain replacement source");
        let mut value = Vec::with_capacity(self.len() - from.len() + to.len());
        value.extend_from_slice(&self[..position]);
        value.extend_from_slice(to);
        value.extend_from_slice(&self[position + from.len()..]);
        value
    }
}
