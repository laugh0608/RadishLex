//! Explicit Linux regression in a read-only container with empty disposable tmpfs mounts.
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{env, fs, io, path::Path, process::Command};

use super::helper::*;
use crate::debian::{
    status_snapshot, ArchiveFixture, VerifiedArtifactRelationship, ARCHIVE_EVIDENCE_FILENAME,
    ARCHIVE_PACKAGE_FILENAME,
};
use crate::startup::*;
use crate::{
    run_linux_maintenance, ArtifactSlot, LinuxInstallState, LinuxMaintenanceCommand,
    LinuxOperationKind, PackageSnapshot,
};

const MOUNTS: &[&str] = &[
    "/var/lib/radishlex",
    "/var/lib/dpkg",
    "/run/lock",
    "/usr/share",
    "/usr/lib/aarch64-linux-gnu/fcitx5",
    "/usr/lib/aarch64-linux-gnu/radishlex",
];
const OPERATION: &str = "11111111111111111111111111111111";
const TEST: &str = "startup::tests::unprivileged::both_components_in_a_non_root_process";

fn require_disposable_mounts() {
    let mounts = fs::read_to_string("/proc/self/mountinfo").unwrap();
    let mount = |path: &str| {
        mounts
            .lines()
            .find(|line| line.split_whitespace().nth(4) == Some(path))
            .unwrap()
    };
    assert!(mount("/")
        .split_whitespace()
        .nth(5)
        .unwrap()
        .split(',')
        .any(|s| s == "ro"));
    assert!(mount("/tmp").contains(" - tmpfs "));
    for path in MOUNTS {
        assert!(
            mount(path).contains(" - tmpfs "),
            "{path} must be a dedicated tmpfs"
        );
        assert!(
            fs::read_dir(path).unwrap().next().is_none(),
            "{path} must be empty"
        );
    }
}

fn read_as_desktop(expected_reason: &str) {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    for name in ["Uid:", "Gid:"] {
        let ids = status
            .lines()
            .find_map(|s| s.strip_prefix(name))
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        assert_eq!(ids, ["65534"; 4]);
    }
    for name in ["CapPrm:", "CapEff:", "CapAmb:"] {
        let caps = status
            .lines()
            .find_map(|s| s.strip_prefix(name))
            .unwrap()
            .trim();
        assert_eq!(u64::from_str_radix(caps, 16).unwrap(), 0);
    }
    assert!(status
        .lines()
        .find_map(|s| s.strip_prefix("Groups:"))
        .unwrap()
        .trim()
        .is_empty());
    let paths = LinuxStartupPaths::system();
    let private = paths.state_root.join("operations").join(OPERATION);
    assert_eq!(
        fs::read_dir(&private).unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        fs::File::open(private.join("target.deb"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    fs::File::open(paths.state_root.join("receipt.json")).unwrap();
    for (component, path) in [
        (
            LinuxStartupComponent::Manager,
            &paths.manager_component_path,
        ),
        (
            LinuxStartupComponent::FcitxAddon,
            &paths.fcitx_component_path,
        ),
    ] {
        let port = LinuxSystemStartupPort::new(paths.clone(), path.clone(), 9);
        let outcome = inspect_linux_startup(
            &paths,
            LinuxStartupBuildIdentity::DebianSystemProduct,
            component,
            &port,
        );
        assert_eq!(
            format!("{:?}", outcome.reason()),
            expected_reason,
            "{component:?}"
        );
        assert_eq!(
            outcome.decision() == LinuxStartupDecision::AllowedProduct,
            expected_reason == "InstalledReceiptVerified"
        );
    }
}

fn check_desktop(reason: LinuxStartupReason) {
    let xdg = TestSite::new("writable-xdg");
    fs::set_permissions(&xdg.root, fs::Permissions::from_mode(0o777)).unwrap();
    write_mode(&xdg.root.join("sentinel"), b"synthetic", 0o666);
    let before = xdg.tree_fingerprint();
    let status = Command::new("/usr/bin/setpriv")
        .args([
            "--reuid=65534",
            "--regid=65534",
            "--clear-groups",
            "--no-new-privs",
        ])
        .arg(env::current_exe().unwrap())
        .args(["--ignored", "--exact", TEST])
        .envs(
            [
                "HOME",
                "XDG_DATA_HOME",
                "XDG_CONFIG_HOME",
                "XDG_STATE_HOME",
                "XDG_CACHE_HOME",
            ]
            .map(|key| (key, &xdg.root)),
        )
        .env("RADISHLEX_TEST_EXPECT_REASON", format!("{reason:?}"))
        .status()
        .unwrap();
    assert!(status.success(), "desktop process expected {reason:?}");
    assert_eq!(xdg.tree_fingerprint(), before);
    eprintln!("UID 65534, Manager and Fcitx: {reason:?}");
}

fn resume() -> Result<crate::TransactionOutcome, crate::LinuxMaintenanceHostError> {
    let args = [
        "resume",
        "--operation-id",
        OPERATION,
        "--authorized-system-mutation",
        "--preserve-user-data",
    ]
    .map(str::to_owned);
    run_linux_maintenance(LinuxMaintenanceCommand::parse(&args).unwrap())
}

#[test]
#[ignore = "requires an isolated read-only Linux root and empty tmpfs fixture mounts"]
fn both_components_in_a_non_root_process() {
    if let Ok(reason) = env::var("RADISHLEX_TEST_EXPECT_REASON") {
        return read_as_desktop(&reason);
    }
    require_disposable_mounts();
    let mut site = TestSite::new("two-uids");
    site.paths = LinuxStartupPaths::system();
    let root = site.create_state_root();
    let archive = ArchiveFixture::canonical();
    let rel = VerifiedArtifactRelationship::verify_package(
        ARCHIVE_PACKAGE_FILENAME,
        ARCHIVE_EVIDENCE_FILENAME,
        &mut archive.package.as_slice(),
        &archive.evidence,
    )
    .unwrap();
    let deb = site.root.join(ARCHIVE_PACKAGE_FILENAME);
    write_mode(&deb, &archive.package, 0o600);
    let evidence = site.root.join(ARCHIVE_EVIDENCE_FILENAME);
    write_mode(&evidence, &archive.evidence, 0o600);
    let extracted = site.root.join("payload");
    assert!(Command::new("dpkg-deb")
        .arg("--extract")
        .arg(&deb)
        .arg(&extracted)
        .status()
        .unwrap()
        .success());
    for path in rel.installed_file_paths() {
        let dest = Path::new(path);
        assert!(MOUNTS.iter().any(|root| dest.starts_with(root)));
        let src = extracted.join(dest.strip_prefix("/").unwrap());
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(src, dest).unwrap();
    }
    let artifact = rel.to_linux_artifact_identity().unwrap();
    let mut receipt = prepared_receipt(
        root,
        LinuxOperationKind::Install,
        None,
        Some(artifact.clone()),
    );
    stage_exact_bytes(
        &site,
        &mut receipt,
        ArtifactSlot::Target,
        &archive.package,
        &archive.evidence,
    );
    for phase in [
        LinuxInstallState::ArtifactsStaged,
        LinuxInstallState::Quiesced,
        LinuxInstallState::PackageMutating,
    ] {
        receipt.advance(phase).unwrap();
    }
    receipt
        .record_target_proof(PackageSnapshot::exact_installed(artifact))
        .unwrap();
    receipt.advance(LinuxInstallState::PackageVerified).unwrap();
    receipt.advance(LinuxInstallState::Completed).unwrap();
    site.write_receipt(&receipt);
    let status = status_snapshot(&rel, None, false);
    write_mode(&site.paths.dpkg_status_path, status.as_bytes(), 0o644);
    use LinuxStartupReason::*;
    check_desktop(UnmanagedPackageState);
    resume().unwrap();
    let record_path = site.paths.record_path();
    let record = fs::read(&record_path).unwrap();
    let inode = fs::metadata(&record_path).unwrap().ino();
    assert_eq!(fs::metadata(&record_path).unwrap().mode() & 0o7777, 0o644);
    check_desktop(InstalledReceiptVerified);
    resume().unwrap();
    assert_eq!(fs::metadata(&record_path).unwrap().ino(), inode);

    fs::rename(&record_path, site.paths.record_tmp()).unwrap();
    fs::set_permissions(site.paths.record_tmp(), fs::Permissions::from_mode(0o600)).unwrap();
    check_desktop(InterruptedReceipt);
    resume().unwrap();
    assert_eq!(fs::read(&record_path).unwrap(), record);

    let fields: serde_json::Value = serde_json::from_slice(&record).unwrap();
    let text = std::str::from_utf8(&record).unwrap();
    let replay = text.replace(fields["receipt_sha256"].as_str().unwrap(), &"0".repeat(64));
    fs::write(&record_path, replay).unwrap();
    check_desktop(PackageIdentityChanged);
    assert!(resume().is_err());
    fs::write(&record_path, &record).unwrap();
    let altered = text.replace("libc6 (>= 2.34)", "libc6 (>= 2.33)");
    assert_ne!(altered, text);
    fs::write(&record_path, altered).unwrap();
    assert!(resume().is_err());
    assert!(!record_path.exists());
    check_desktop(UnmanagedPackageState);
    resume().unwrap();
    fs::set_permissions(&record_path, fs::Permissions::from_mode(0o666)).unwrap();
    check_desktop(PackageIdentityChanged);
    fs::set_permissions(&record_path, fs::Permissions::from_mode(0o644)).unwrap();
    let guard = site.store().acquire_guard().unwrap();
    check_desktop(ActiveGuard);
    drop(guard);
    fs::write(
        &site.paths.dpkg_status_path,
        status.replace("Version: 1.13.1", "Version: 1.0"),
    )
    .unwrap();
    check_desktop(DependencyUnavailable);
    fs::write(
        &site.paths.dpkg_status_path,
        status.replace("Package: librime1t64\n", "Package: unrelated\n"),
    )
    .unwrap();
    check_desktop(DependencyUnavailable);
    fs::write(&site.paths.dpkg_status_path, status).unwrap();
    let manifest = fs::read(&site.paths.product_manifest_path).unwrap();
    fs::write(&site.paths.product_manifest_path, b"drift").unwrap();
    check_desktop(PackageIdentityChanged);
    fs::write(&site.paths.product_manifest_path, manifest).unwrap();
    let ffi = fs::read(&site.paths.manager_ffi_path).unwrap();
    fs::write(&site.paths.manager_ffi_path, b"drift").unwrap();
    check_desktop(ComponentIdentityChanged);
    fs::write(&site.paths.manager_ffi_path, ffi).unwrap();

    let (private, _) = staged_paths(&site, ArtifactSlot::Target);
    fs::write(&private, b"private drift").unwrap();
    check_desktop(InstalledReceiptVerified);
    assert!(resume().is_err());
    assert!(!record_path.exists());
    check_desktop(UnmanagedPackageState);
    assert_eq!(fs::read(&private).unwrap(), b"private drift");
    fs::write(&private, &archive.package).unwrap();
    resume().unwrap();
    check_desktop(InstalledReceiptVerified);

    // A new authorized repair must withdraw the previous record before discovering
    // private drift. Its prepared receipt and dpkg mutation must never be reached.
    fs::write(&private, b"changed before repair").unwrap();
    let args = [
        "start",
        "--operation-id",
        "22222222222222222222222222222222",
        "--kind",
        "repair",
        "--target-deb",
        deb.to_str().unwrap(),
        "--target-evidence",
        evidence.to_str().unwrap(),
        "--authorized-system-mutation",
        "--preserve-user-data",
    ]
    .map(str::to_owned);
    let error = run_linux_maintenance(LinuxMaintenanceCommand::parse(&args).unwrap()).unwrap_err();
    assert_eq!(error.code(), crate::LinuxMaintenanceHostErrorCode::Store);
    check_desktop(UnmanagedPackageState);
    assert_eq!(
        fs::read(site.paths.state_root.join("receipt.json")).unwrap(),
        receipt.encode().unwrap()
    );
}
