use std::collections::BTreeSet;
use std::io::{self, Read};

use md5::Md5;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::debian::relationship::{
    DEPENDENCY_ANALYSIS_PROFILE, DISTRIBUTION_IDENTITY, PRODUCT_MANIFEST_PATH,
};

pub(super) const VERSION: &str = "26.7.1+39-1";
pub(crate) const PACKAGE_FILENAME: &str = "radishlex_26.7.1+39-1_arm64.deb";
pub(crate) const EVIDENCE_FILENAME: &str = "radishlex_26.7.1+39-1_arm64.deb.evidence.json";
pub(super) const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;

const DEPENDENCIES: [&str; 6] = [
    "libc6 (>= 2.34)",
    "libstdc++6 (>= 11)",
    "fcitx5 (>= 5.1.9)",
    "librime1t64 (>= 1.13.1)",
    "fonts-dejavu-core",
    "fonts-noto-cjk",
];

#[derive(Clone)]
pub(super) struct TarEntrySpec {
    pub(super) raw_path: String,
    pub(super) kind: u8,
    pub(super) mode: u32,
    pub(super) uid: u32,
    pub(super) gid: u32,
    pub(super) content: Vec<u8>,
}

impl TarEntrySpec {
    pub(super) fn directory(path: &str) -> Self {
        Self {
            raw_path: format!(".{path}/"),
            kind: b'5',
            mode: 0o755,
            uid: 0,
            gid: 0,
            content: Vec::new(),
        }
    }

    pub(super) fn file(path: &str, mode: u32, content: impl Into<Vec<u8>>) -> Self {
        Self {
            raw_path: format!(".{path}"),
            kind: b'0',
            mode,
            uid: 0,
            gid: 0,
            content: content.into(),
        }
    }

    pub(super) fn absolute_path(&self) -> String {
        let without_prefix = self.raw_path.strip_prefix('.').unwrap_or(&self.raw_path);
        without_prefix.trim_end_matches('/').to_owned()
    }
}

pub(super) struct CanonicalParts {
    pub(super) control: Vec<u8>,
    pub(super) md5sums: Vec<u8>,
    pub(super) manifest: Vec<u8>,
    pub(super) control_entries: Vec<TarEntrySpec>,
    pub(super) data_entries: Vec<TarEntrySpec>,
}

pub(crate) struct ArchiveFixture {
    pub(crate) package: Vec<u8>,
    pub(crate) evidence: Vec<u8>,
    pub(super) members: Vec<(String, Vec<u8>)>,
    pub(super) control: Vec<u8>,
    pub(super) md5sums: Vec<u8>,
    pub(super) manifest: Vec<u8>,
}

impl ArchiveFixture {
    pub(crate) fn canonical() -> Self {
        let parts = canonical_parts(VERSION);
        Self::from_parts(parts, 0)
    }

    pub(super) fn from_parts(parts: CanonicalParts, padding_byte: u8) -> Self {
        let control_tar = build_tar(&parts.control_entries, padding_byte);
        let data_tar = build_tar(&parts.data_entries, padding_byte);
        Self::from_member_bytes(
            parts.control,
            parts.md5sums,
            parts.manifest,
            control_tar,
            data_tar,
        )
    }

    pub(super) fn from_member_bytes(
        control: Vec<u8>,
        md5sums: Vec<u8>,
        manifest: Vec<u8>,
        control_tar: Vec<u8>,
        data_tar: Vec<u8>,
    ) -> Self {
        let members = vec![
            ("debian-binary".to_owned(), b"2.0\n".to_vec()),
            ("control.tar".to_owned(), control_tar),
            ("data.tar".to_owned(), data_tar),
        ];
        let package = build_ar(&members);
        let evidence = build_evidence(&package, &members, &control, &md5sums, &manifest);
        Self {
            package,
            evidence,
            members,
            control,
            md5sums,
            manifest,
        }
    }

    pub(super) fn verify(
        &self,
    ) -> Result<(), crate::debian::relationship::DebianRelationshipError> {
        let mut reader = self.package.as_slice();
        crate::debian::relationship::VerifiedArtifactRelationship::verify_package(
            PACKAGE_FILENAME,
            EVIDENCE_FILENAME,
            &mut reader,
            &self.evidence,
        )
        .map(|_| ())
    }

    pub(super) fn rebuild_evidence(&mut self) {
        self.evidence = build_evidence(
            &self.package,
            &self.members,
            &self.control,
            &self.md5sums,
            &self.manifest,
        );
    }
}

pub(super) fn canonical_parts(version: &str) -> CanonicalParts {
    let ffi = b"synthetic ffi".to_vec();
    let file_specs = [
        (
            "fcitx-ffi",
            "/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so",
            0o644,
            ffi.clone(),
        ),
        (
            "fcitx-addon",
            "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so",
            0o644,
            b"synthetic fcitx addon".to_vec(),
        ),
        (
            "manager-bundle",
            "/usr/lib/aarch64-linux-gnu/radishlex/manager/data/flutter_assets/fonts/MaterialIcons-Regular.otf",
            0o644,
            b"synthetic material icons".to_vec(),
        ),
        (
            "manager-bundle",
            "/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/libradishlex_ime_ffi.so",
            0o644,
            ffi,
        ),
        (
            "manager-bundle",
            "/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager",
            0o755,
            b"synthetic manager".to_vec(),
        ),
        (
            "manager-desktop-entry",
            "/usr/share/applications/dev.radishlex.radishlexManager.desktop",
            0o644,
            b"synthetic desktop entry".to_vec(),
        ),
        (
            "product-license",
            "/usr/share/doc/radishlex/copyright",
            0o644,
            b"synthetic license".to_vec(),
        ),
        (
            "fcitx-addon-metadata",
            "/usr/share/fcitx5/addon/radishlex.conf",
            0o644,
            b"synthetic addon metadata".to_vec(),
        ),
        (
            "fcitx-input-method-metadata",
            "/usr/share/fcitx5/inputmethod/radishlex.conf",
            0o644,
            b"synthetic input method metadata".to_vec(),
        ),
        (
            "manager-icon",
            "/usr/share/icons/hicolor/scalable/apps/dev.radishlex.radishlexManager.svg",
            0o644,
            b"synthetic manager icon".to_vec(),
        ),
        (
            "input-method-icon",
            "/usr/share/icons/hicolor/scalable/apps/fcitx-radishlex.svg",
            0o644,
            b"synthetic input icon".to_vec(),
        ),
        (
            "rime-data",
            "/usr/share/radishlex/rime/radishlex_pinyin.schema.yaml",
            0o644,
            b"synthetic rime schema".to_vec(),
        ),
    ];
    let mut files = file_specs
        .iter()
        .map(|(component, path, mode, content)| {
            json!({
                "component_id": component,
                "gid": 0,
                "mode": format!("{mode:04o}"),
                "path": path,
                "sha256": sha256(content),
                "size": content.len(),
                "uid": 0
            })
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    let directories = manifest_directories(&files);
    let manifest = build_manifest(version, &files, &directories);

    let mut data_entries = directories
        .iter()
        .map(|record| TarEntrySpec::directory(record["path"].as_str().unwrap()))
        .collect::<Vec<_>>();
    data_entries.extend(
        file_specs
            .iter()
            .map(|(_, path, mode, content)| TarEntrySpec::file(path, *mode, content.clone())),
    );
    data_entries.push(TarEntrySpec::file(
        PRODUCT_MANIFEST_PATH,
        0o644,
        manifest.clone(),
    ));
    data_entries.sort_by_key(TarEntrySpec::absolute_path);

    let installed_size_kib = data_entries
        .iter()
        .filter(|entry| entry.kind == b'0')
        .map(|entry| (entry.content.len() as u64).div_ceil(1024))
        .sum();
    let control = build_control(version, installed_size_kib);
    let md5sums = build_md5sums(&data_entries);
    let control_entries = vec![
        TarEntrySpec::file("/control", 0o644, control.clone()),
        TarEntrySpec::file("/md5sums", 0o644, md5sums.clone()),
    ];
    CanonicalParts {
        control,
        md5sums,
        manifest,
        control_entries,
        data_entries,
    }
}

pub(super) fn build_md5sums(entries: &[TarEntrySpec]) -> Vec<u8> {
    let mut value = String::new();
    for entry in entries.iter().filter(|entry| entry.kind == b'0') {
        let path = entry
            .absolute_path()
            .strip_prefix('/')
            .expect("fixture path is absolute")
            .to_owned();
        value.push_str(&format!("{:x}  {path}\n", Md5::digest(&entry.content)));
    }
    value.into_bytes()
}

fn build_control(version: &str, installed_size_kib: u64) -> Vec<u8> {
    format!(
        "Package: radishlex\n\
         Version: {version}\n\
         Architecture: arm64\n\
         Maintainer: RadishLex <laugh0608@foxmail.com>\n\
         Section: utils\n\
         Priority: optional\n\
         Installed-Size: {installed_size_kib}\n\
         Depends: {}\n\
         Homepage: https://github.com/laugh0608/RadishLex\n\
         Description: Local-first Chinese input system for Fcitx5\n\
         \x20RadishLex combines a native Fcitx5 addon with a local management application.\n\
         \x20This template describes the Debian 13 ARM64 local acceptance carrier only.\n",
        DEPENDENCIES.join(", ")
    )
    .into_bytes()
}

fn build_manifest(version: &str, files: &[Value], directories: &[Value]) -> Vec<u8> {
    let ffi_sha256 = files
        .iter()
        .find(|record| {
            record["path"] == "/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so"
        })
        .unwrap()["sha256"]
        .as_str()
        .unwrap();
    let manifest = json!({
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
            "build_number": "39",
            "data_layout": "xdg-v1",
            "data_removal": "separate-authorized-flow",
            "debian_architecture": "arm64",
            "debian_codename": "trixie",
            "debian_release": "13",
            "default_removal": "programs-only",
            "distribution_identity": DISTRIBUTION_IDENTITY,
            "ffi_abi_version": 9,
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
            "product_version": "26.7.1",
            "rime_data_lock_sha256": "44".repeat(32),
            "rime_data_manifest_version": 1,
            "rime_schema_id": "radishlex_pinyin",
            "runtime_layout": "debian-system-v1",
            "settings_format_version": 1,
            "userdb_schema_version": 9
        }
    });
    pretty_json(&manifest)
}

fn manifest_directories(files: &[Value]) -> Vec<Value> {
    let mut paths = BTreeSet::new();
    for path in files
        .iter()
        .map(|record| record["path"].as_str().unwrap())
        .chain(std::iter::once(PRODUCT_MANIFEST_PATH))
    {
        let mut value = path;
        while let Some((parent, _)) = value.rsplit_once('/') {
            if parent.is_empty() {
                break;
            }
            paths.insert(parent.to_owned());
            value = parent;
        }
    }
    paths
        .into_iter()
        .map(|path| json!({"gid": 0, "mode": "0755", "path": path, "uid": 0}))
        .collect()
}

pub(super) fn build_tar(entries: &[TarEntrySpec], padding_byte: u8) -> Vec<u8> {
    let mut output = Vec::new();
    for entry in entries {
        output.extend_from_slice(&ustar_header(entry));
        output.extend_from_slice(&entry.content);
        let padding = (512 - entry.content.len() % 512) % 512;
        output.extend(std::iter::repeat(padding_byte).take(padding));
    }
    output.extend_from_slice(&[0_u8; 1024]);
    let record_padding = (10_240 - output.len() % 10_240) % 10_240;
    output.extend(std::iter::repeat(0).take(record_padding));
    output
}

fn ustar_header(entry: &TarEntrySpec) -> [u8; 512] {
    let mut header = [0_u8; 512];
    let (prefix, name) = ustar_split(&entry.raw_path);
    write_text(&mut header[..100], name);
    write_octal(&mut header[100..108], entry.mode as u64, 7);
    write_octal(&mut header[108..116], entry.uid as u64, 7);
    write_octal(&mut header[116..124], entry.gid as u64, 7);
    write_octal(&mut header[124..136], entry.content.len() as u64, 11);
    write_octal(&mut header[136..148], 0, 11);
    header[148..156].fill(b' ');
    header[156] = entry.kind;
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    write_text(&mut header[265..297], "root");
    write_text(&mut header[297..329], "root");
    write_text(&mut header[345..500], prefix);
    let checksum = header.iter().map(|byte| u64::from(*byte)).sum::<u64>();
    let encoded = format!("{checksum:06o}");
    header[148..154].copy_from_slice(encoded.as_bytes());
    header[154] = 0;
    header[155] = b' ';
    header
}

fn ustar_split(value: &str) -> (&str, &str) {
    if value.len() <= 100 {
        return ("", value);
    }
    value
        .match_indices('/')
        .rev()
        .find_map(|(index, _)| {
            (index + 1 < value.len() && index <= 155 && value.len() - index - 1 <= 100)
                .then_some((&value[..index], &value[index + 1..]))
        })
        .expect("fixture path fits USTAR")
}

fn write_text(field: &mut [u8], value: &str) {
    assert!(value.len() <= field.len());
    field[..value.len()].copy_from_slice(value.as_bytes());
}

fn write_octal(field: &mut [u8], value: u64, digits: usize) {
    let encoded = format!("{value:0digits$o}");
    assert_eq!(encoded.len(), digits);
    field[..digits].copy_from_slice(encoded.as_bytes());
    field[digits] = 0;
}

pub(super) fn build_ar(members: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut output = b"!<arch>\n".to_vec();
    for (name, content) in members {
        let stored_name = format!("{name}/");
        let header = format!(
            "{stored_name:<16}{:<12}{:<6}{:<6}{:<8o}{:<10}`\n",
            0,
            0,
            0,
            0o100644,
            content.len()
        );
        assert_eq!(header.len(), 60);
        output.extend_from_slice(header.as_bytes());
        output.extend_from_slice(content);
        if content.len() % 2 == 1 {
            output.push(b'\n');
        }
    }
    output
}

pub(super) fn build_evidence(
    package: &[u8],
    members: &[(String, Vec<u8>)],
    control: &[u8],
    md5sums: &[u8],
    manifest: &[u8],
) -> Vec<u8> {
    let evidence = json!({
        "archive": {
            "compression": "none",
            "format": "debian-binary-2.0-ar-v1",
            "members": members.iter().map(|(name, content)| json!({
                "name": name,
                "sha256": sha256(content),
                "size": content.len()
            })).collect::<Vec<_>>(),
            "source_date_epoch": 0,
            "tar_format": "ustar"
        },
        "control": {
            "architecture": "arm64",
            "depends": DEPENDENCIES,
            "installed_size_kib": installed_size_from_control(control),
            "md5sums_sha256": sha256(md5sums),
            "package": "radishlex",
            "sha256": sha256(control),
            "version": VERSION
        },
        "dependency_analysis": {
            "loader_owner": "libc6:arm64",
            "private_libraries": ["libflutter_linux_gtk.so", "libradishlex_ime_ffi.so"],
            "profile": DEPENDENCY_ANALYSIS_PROFILE,
            "stderr_policy": "exact-private-libraries-and-libc6-usrmerge-v1",
            "tool": "dpkg-shlibdeps"
        },
        "distribution_identity": DISTRIBUTION_IDENTITY,
        "format_version": 1,
        "package": {
            "filename": PACKAGE_FILENAME,
            "sha256": sha256(package),
            "size": package.len()
        },
        "product_manifest": {
            "format_version": 1,
            "path": PRODUCT_MANIFEST_PATH,
            "sha256": sha256(manifest)
        }
    });
    pretty_json(&evidence)
}

pub(super) fn pretty_json(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn installed_size_from_control(control: &[u8]) -> u64 {
    std::str::from_utf8(control)
        .expect("fixture control is UTF-8")
        .lines()
        .find_map(|line| line.strip_prefix("Installed-Size: "))
        .expect("fixture control has Installed-Size")
        .parse()
        .expect("fixture Installed-Size is decimal")
}

pub(super) fn sha256(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

pub(super) struct PanicReader {
    pub(super) reads: usize,
}

impl Read for PanicReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        panic!("bounded evidence must be rejected before the package reader is touched")
    }
}
