use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use super::helper::*;
use crate::model::{LinuxInstallReceipt, LinuxInstallState, LinuxOperationKind};
use crate::startup::record::StartupRecord;
use crate::startup::*;

fn terminal_site(label: &str) -> (TestSite, LinuxInstallReceipt) {
    let site = TestSite::new(label);
    let root = site.create_state_root();
    let mut receipt = prepared_receipt(
        root,
        LinuxOperationKind::Install,
        None,
        Some(artifact("26.7.1+38-1", 0x52)),
    );
    advance_to(&site, &mut receipt, LinuxInstallState::Completed);
    site.write_qualified_receipt(&receipt);
    (site, receipt)
}

fn check(site: &TestSite, port: &impl LinuxStartupPort) -> LinuxStartupOutcome {
    inspect_linux_startup(
        &site.paths,
        LinuxStartupBuildIdentity::DebianSystemProduct,
        LinuxStartupComponent::Manager,
        port,
    )
}

#[test]
fn legacy_missing_record_and_replayed_terminal_fail_before_component_start() {
    let (site, receipt) = terminal_site("missing-proof");
    let (other, _) = terminal_site("other-proof");
    let port = FakeStartupPort::new(LinuxPackageObservation::installed(
        receipt.installed_artifact().unwrap(),
    ));
    fs::remove_file(site.paths.record_path()).unwrap();
    assert_eq!(
        check(&site, &port).reason(),
        LinuxStartupReason::UnmanagedPackageState
    );
    fs::copy(other.paths.record_path(), site.paths.record_path()).unwrap();
    assert_eq!(
        check(&site, &port).reason(),
        LinuxStartupReason::PackageIdentityChanged
    );
    assert_eq!(port.component_calls.get(), 0);
}

#[test]
fn orphan_record_blocks_development_with_no_installed_package() {
    let (old, _) = terminal_site("old-record");
    let fresh = TestSite::new("orphan-record");
    fs::copy(old.paths.record_path(), fresh.paths.record_path()).unwrap();
    let port = FakeStartupPort::new(LinuxPackageObservation::not_installed());
    let result = inspect_linux_startup(
        &fresh.paths,
        LinuxStartupBuildIdentity::DevelopmentStaged,
        LinuxStartupComponent::Manager,
        &port,
    );
    assert_eq!(result.reason(), LinuxStartupReason::UnexpectedStateObject);
    assert_eq!(result.decision(), LinuxStartupDecision::FailedClosed);
    assert_eq!(port.component_calls.get(), 0);
}

#[test]
fn malformed_records_remain_untouched_and_never_grant_startup() {
    for case in [
        "mode",
        "symlink",
        "hardlink",
        "extra",
        "duplicate",
        "noncanonical",
        "format",
        "dependency",
        "repeated",
        "oversize",
    ] {
        let (site, receipt) = terminal_site(case);
        let path = site.paths.record_path();
        let text = fs::read_to_string(&path).unwrap();
        match case {
            "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap(),
            "symlink" => {
                let displaced = site.root.join("other");
                fs::rename(&path, &displaced).unwrap();
                symlink(displaced, &path).unwrap();
            }
            "hardlink" => fs::hard_link(&path, site.root.join("other")).unwrap(),
            "extra" => fs::write(&path, text.replacen('{', "{\"extra\":1,", 1)).unwrap(),
            "duplicate" => {
                fs::write(&path, text.replacen('{', "{\"format\":\"other\",", 1)).unwrap()
            }
            "noncanonical" => fs::write(&path, text.trim_end()).unwrap(),
            "format" => fs::write(&path, text.replace("startup-v1", "startup-v2")).unwrap(),
            "dependency" => fs::write(&path, text.replace("libc6", "invalid dependency")).unwrap(),
            "repeated" => {
                fs::write(&path, text.replace("\"libc6\"", "\"libc6\",\"libc6\"")).unwrap()
            }
            "oversize" => fs::write(&path, vec![b' '; 128 * 1024 + 1]).unwrap(),
            _ => unreachable!(),
        }
        let port = FakeStartupPort::new(LinuxPackageObservation::installed(
            receipt.installed_artifact().unwrap(),
        ));
        let before = site.tree_fingerprint();
        assert_eq!(
            check(&site, &port).decision(),
            LinuxStartupDecision::FailedClosed,
            "{case}"
        );
        assert_eq!(port.component_calls.get(), 0);
        assert_eq!(site.tree_fingerprint(), before);
    }
}

#[test]
fn interrupted_publication_only_resumes_exact_complete_bytes() {
    let (site, receipt) = terminal_site("partial-record");
    let record = StartupRecord::read(&site.paths).unwrap().unwrap();
    let bytes = fs::read(site.paths.record_path()).unwrap();
    record.revoke(&site.paths).unwrap();
    let tmp = site.paths.record_tmp();
    write_mode(&tmp, &bytes[..bytes.len() / 2], 0o600);
    let port = FakeStartupPort::new(LinuxPackageObservation::installed(
        receipt.installed_artifact().unwrap(),
    ));
    assert_eq!(
        check(&site, &port).reason(),
        LinuxStartupReason::InterruptedReceipt
    );
    assert!(record.publish(&site.paths).is_err());
    assert_eq!(fs::read(&tmp).unwrap(), bytes[..bytes.len() / 2]);
    assert!(!site.paths.record_path().exists());
    fs::write(&tmp, bytes).unwrap();
    record.publish(&site.paths).unwrap();
    assert!(!tmp.exists());
    assert_eq!(
        check(&site, &port).decision(),
        LinuxStartupDecision::AllowedProduct
    );
}

#[test]
fn guard_or_record_replacement_during_component_validation_revokes_the_decision() {
    for case in ["guard", "receipt", "record", "status"] {
        let (site, receipt) = terminal_site(case);
        let port = FakeStartupPort::new(LinuxPackageObservation::installed(
            receipt.installed_artifact().unwrap(),
        ));
        let path = match case {
            "receipt" => site.paths.state_root.join("receipt.json"),
            "record" => site.paths.record_path(),
            "status" => site.paths.dpkg_status_path.clone(),
            _ => site.paths.guard_path.clone(),
        };
        let changed = site.root.join("replacement");
        let port = port.during_component(move || {
            if case == "guard" {
                write_mode(&path, &[], 0o600);
            } else {
                let data = if case == "status" {
                    b"install state".to_vec()
                } else {
                    fs::read(&path).unwrap()
                };
                write_mode(&changed, &data, 0o644);
                fs::rename(&changed, &path).unwrap();
            }
        });
        assert_ne!(
            check(&site, &port).decision(),
            LinuxStartupDecision::AllowedProduct,
            "{case}"
        );
        assert_eq!(port.component_calls.get(), 1);
    }
}
