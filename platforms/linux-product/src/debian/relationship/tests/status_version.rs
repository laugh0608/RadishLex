use std::cmp::Ordering;

use super::super::*;
use super::helper::*;

#[test]
fn debian_version_comparison_covers_numeric_epoch_tilde_and_revision_rules() {
    assert_eq!(
        compare_debian_versions("26.9-1", "26.10-1").unwrap(),
        Ordering::Less
    );
    assert_eq!(
        compare_debian_versions("1:1.0-1", "2.0-99").unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        compare_debian_versions("1.0~rc1-1", "1.0-1").unwrap(),
        Ordering::Less
    );
    assert_eq!(
        compare_debian_versions("1.0-2", "1.0-10").unwrap(),
        Ordering::Less
    );
    assert_eq!(
        compare_debian_versions("1.001-01", "1.1-1").unwrap(),
        Ordering::Equal
    );
}

#[test]
fn dpkg_status_accepts_complete_relationship_and_held_dependency() {
    let artifact = ArtifactFixture::new("26.7.1+38-1").verify();
    let status = status_snapshot(&artifact, Some(("libc6", "hold ok installed")), false);
    let snapshot = DpkgStatusSnapshot::parse(status.as_bytes()).expect("parse dpkg snapshot");
    snapshot
        .validate_installed_relationship(&artifact)
        .expect("validate installed dependency relationship");
}

#[test]
fn dependency_preflight_does_not_require_target_product_to_be_installed() {
    let artifact = ArtifactFixture::new("26.7.1+38-1").verify();
    let complete = remove_package_paragraph(&status_snapshot(&artifact, None, false), "radishlex");
    DpkgStatusSnapshot::parse(complete.as_bytes())
        .expect("parse install preflight status")
        .validate_dependencies(&artifact)
        .expect("complete target dependencies pass before install");

    let missing = remove_package_paragraph(&complete, "librime1t64");
    assert_dependency_error(
        &artifact,
        &missing,
        DebianRelationshipErrorCode::DependencyUnavailable,
    );
    let old = complete.replace("Version: 5.1.10\n", "Version: 5.1.8\n");
    assert_dependency_error(
        &artifact,
        &old,
        DebianRelationshipErrorCode::DependencyVersionUnsatisfied,
    );
    let wrong_arch = complete.replacen(
        "Package: librime1t64\nStatus: install ok installed\nArchitecture: arm64\n",
        "Package: librime1t64\nStatus: install ok installed\nArchitecture: amd64\n",
        1,
    );
    assert_dependency_error(
        &artifact,
        &wrong_arch,
        DebianRelationshipErrorCode::DependencyArchitectureMismatch,
    );
    let partial = complete.replacen(
        "Package: libc6\nStatus: install ok installed\n",
        "Package: libc6\nStatus: install ok unpacked\n",
        1,
    );
    assert_dependency_error(
        &artifact,
        &partial,
        DebianRelationshipErrorCode::DependencyUnavailable,
    );
}

#[test]
fn dpkg_status_parses_folded_depends_and_rejects_duplicate_identity() {
    let artifact = ArtifactFixture::new("26.7.1+38-1").verify();
    let folded = status_snapshot(&artifact, None, true);
    DpkgStatusSnapshot::parse(folded.as_bytes())
        .expect("parse folded dpkg status")
        .validate_installed_relationship(&artifact)
        .expect("folded Depends remains identical");

    let libc = dependency_paragraph("libc6", "arm64", "2.36", "install ok installed");
    let duplicate = format!("{}{}", status_snapshot(&artifact, None, false), libc);
    assert_eq!(
        DpkgStatusSnapshot::parse(duplicate.as_bytes())
            .expect_err("duplicate package architecture must fail")
            .code(),
        DebianRelationshipErrorCode::DpkgStatusInvalid
    );
}

#[test]
fn dpkg_status_preserves_unrelated_complex_depends_and_conffiles() {
    let artifact = ArtifactFixture::new("26.7.1+38-1").verify();
    let status = format!(
        "Package: unrelated\n\
         Status: install ok installed\n\
         Architecture: arm64\n\
         Version: 1.0-1\n\
         Depends: alpha:any (>= 1) |\n\
         \x20beta, gamma [linux-any]\n\
         Conffiles:\n\
         \x20/etc/unrelated deadbeef\n\n{}",
        status_snapshot(&artifact, None, false)
    );
    let snapshot = DpkgStatusSnapshot::parse(status.as_bytes()).expect("parse real status fields");
    assert_eq!(
        snapshot.records()[0].raw_dependencies(),
        Some("alpha:any (>= 1) | beta, gamma [linux-any]")
    );
    assert_eq!(snapshot.records()[0].product_dependencies(), None);
    snapshot
        .validate_installed_relationship(&artifact)
        .expect("unrelated Debian grammar must not poison the product profile");
}

#[test]
fn dpkg_status_accepts_digit_leading_package_names_and_rejects_invalid_names() {
    let valid = b"Package: 7zip\nStatus: install ok installed\nArchitecture: arm64\nVersion: 22.01+dfsg-8\n\n";
    DpkgStatusSnapshot::parse(valid).expect("digit-leading Debian package name is valid");

    for invalid in ["x", "-bad"] {
        let status = format!(
            "Package: {invalid}\nStatus: install ok installed\nArchitecture: arm64\nVersion: 1\n\n"
        );
        assert_eq!(
            DpkgStatusSnapshot::parse(status.as_bytes())
                .expect_err("invalid Debian package name must fail")
                .code(),
            DebianRelationshipErrorCode::DpkgStatusInvalid
        );
    }
}

#[test]
fn dependency_relationship_rejects_missing_old_wrong_arch_and_partial_packages() {
    let artifact = ArtifactFixture::new("26.7.1+38-1").verify();
    let complete = status_snapshot(&artifact, None, false);

    let missing = remove_package_paragraph(&complete, "librime1t64");
    assert_relationship_error(
        &artifact,
        &missing,
        DebianRelationshipErrorCode::DependencyUnavailable,
    );

    let old = complete.replace("Version: 5.1.10\n", "Version: 5.1.8\n");
    assert_relationship_error(
        &artifact,
        &old,
        DebianRelationshipErrorCode::DependencyVersionUnsatisfied,
    );

    let wrong_arch = complete.replacen(
        "Package: librime1t64\nStatus: install ok installed\nArchitecture: arm64\n",
        "Package: librime1t64\nStatus: install ok installed\nArchitecture: amd64\n",
        1,
    );
    assert_relationship_error(
        &artifact,
        &wrong_arch,
        DebianRelationshipErrorCode::DependencyArchitectureMismatch,
    );

    let partial = complete.replacen(
        "Package: libc6\nStatus: install ok installed\n",
        "Package: libc6\nStatus: install ok unpacked\n",
        1,
    );
    assert_relationship_error(
        &artifact,
        &partial,
        DebianRelationshipErrorCode::DependencyUnavailable,
    );

    let reinstreq = complete.replacen(
        "Package: libc6\nStatus: install ok installed\n",
        "Package: libc6\nStatus: install reinstreq installed\n",
        1,
    );
    assert_relationship_error(
        &artifact,
        &reinstreq,
        DebianRelationshipErrorCode::DependencyUnavailable,
    );
}

#[test]
fn font_relationship_requires_architecture_all() {
    let artifact = ArtifactFixture::new("26.7.1+38-1").verify();
    let complete = status_snapshot(&artifact, None, false);
    DpkgStatusSnapshot::parse(complete.as_bytes())
        .unwrap()
        .validate_installed_relationship(&artifact)
        .expect("architecture all fonts satisfy the profile");

    let native_font = complete.replacen(
        "Package: fonts-noto-cjk\nStatus: install ok installed\nArchitecture: all\n",
        "Package: fonts-noto-cjk\nStatus: install ok installed\nArchitecture: arm64\n",
        1,
    );
    assert_relationship_error(
        &artifact,
        &native_font,
        DebianRelationshipErrorCode::DependencyArchitectureMismatch,
    );
}

#[test]
fn installed_control_relationship_drift_is_rejected() {
    let artifact = ArtifactFixture::new("26.7.1+38-1").verify();
    let status = status_snapshot(&artifact, None, false)
        .replace("Depends: libc6 (>= 2.34)", "Depends: libc6 (>= 2.33)");
    assert_relationship_error(
        &artifact,
        &status,
        DebianRelationshipErrorCode::PackageIdentityMismatch,
    );
}
