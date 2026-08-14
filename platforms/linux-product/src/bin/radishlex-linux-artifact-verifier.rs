#[cfg(target_os = "linux")]
fn main() {
    use std::fs::{self, File};
    use std::path::Path;

    use radishlex_linux_product_install::VerifiedArtifactRelationship;

    let arguments = match std::env::args_os()
        .skip(1)
        .map(|value| value.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(arguments) => arguments,
        Err(_) => return fail("ArgumentInvalid"),
    };
    let [package_flag, package, evidence_flag, evidence] = arguments.as_slice() else {
        return fail("ArgumentInvalid");
    };
    if package_flag != "--package" || evidence_flag != "--evidence" {
        return fail("ArgumentInvalid");
    }
    let package = Path::new(package);
    let evidence = Path::new(evidence);
    let Some(package_filename) = package.file_name().and_then(|value| value.to_str()) else {
        return fail("ArgumentInvalid");
    };
    let Some(evidence_filename) = evidence.file_name().and_then(|value| value.to_str()) else {
        return fail("ArgumentInvalid");
    };
    if !package.is_absolute()
        || !evidence.is_absolute()
        || evidence_filename != format!("{package_filename}.evidence.json")
    {
        return fail("ArgumentInvalid");
    }
    let mut package = match File::open(package) {
        Ok(package) => package,
        Err(_) => return fail("ArtifactUnavailable"),
    };
    let evidence = match fs::read(evidence) {
        Ok(evidence) => evidence,
        Err(_) => return fail("ArtifactUnavailable"),
    };
    match VerifiedArtifactRelationship::verify_package(
        package_filename,
        evidence_filename,
        &mut package,
        &evidence,
    ) {
        Ok(_) => println!("artifact_verification=ok"),
        Err(error) => fail(&format!("{:?}", error.code())),
    }
}

#[cfg(target_os = "linux")]
fn fail(code: &str) {
    eprintln!("artifact_error={code}");
    std::process::exit(1);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("artifact_error=EnvironmentUnsupported");
    std::process::exit(1);
}
