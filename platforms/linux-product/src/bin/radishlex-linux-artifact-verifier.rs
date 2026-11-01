#[cfg(target_os = "linux")]
fn main() {
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    use radishlex_linux_product_install::{LinuxInstallReceipt, VerifiedArtifactRelationship};
    use sha2::{Digest, Sha256};

    let arguments = match std::env::args_os()
        .skip(1)
        .map(|value| value.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(arguments) => arguments,
        Err(_) => return fail("ArgumentInvalid"),
    };
    let (receipt, artifact_arguments) = match arguments.as_slice() {
        [receipt_flag, receipt, rest @ ..] if receipt_flag == "--seed-receipt" => {
            (Some(receipt), rest)
        }
        rest => (None, rest),
    };
    if receipt.is_some_and(|value| !Path::new(value).is_absolute()) {
        return fail("ArgumentInvalid");
    }
    let [package_flag, package, evidence_flag, evidence] = artifact_arguments else {
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
    let evidence = match File::open(evidence).and_then(|file| {
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        Ok(bytes)
    }) {
        Ok(evidence) => evidence,
        Err(_) => return fail("ArtifactUnavailable"),
    };
    match VerifiedArtifactRelationship::verify_package(
        package_filename,
        evidence_filename,
        &mut package,
        &evidence,
    ) {
        Ok(relationship) => {
            let Some(receipt_path) = receipt else {
                println!("artifact_verification=ok");
                return;
            };
            let raw = match File::open(receipt_path).and_then(|file| {
                let mut bytes = Vec::new();
                file.take(64 * 1024 + 1).read_to_end(&mut bytes)?;
                Ok(bytes)
            }) {
                Ok(raw) => raw,
                Err(_) => return fail("ReceiptUnavailable"),
            };
            let receipt = match LinuxInstallReceipt::decode(&raw) {
                Ok(receipt) => receipt,
                Err(_) => return fail("ReceiptInvalid"),
            };
            let expected = match relationship.to_linux_artifact_identity() {
                Ok(expected) => expected,
                Err(_) => return fail("ArtifactInvalid"),
            };
            let facts = match receipt.source_seed_facts(&expected) {
                Ok(facts) => facts,
                Err(_) => return fail("SeedReceiptInvalid"),
            };
            let root = match serde_json::to_value(receipt.root_identity())
                .and_then(|value| serde_json::to_vec(&value))
            {
                Ok(root) => root,
                Err(_) => return fail("ReceiptInvalid"),
            };
            let result = serde_json::json!({
                "facts": facts,
                "receipt_sha256": format!("{:x}", Sha256::digest(&raw)),
                "operation_sha256": format!("{:x}", Sha256::digest(receipt.operation_id().as_bytes())),
                "root_identity_sha256": format!("{:x}", Sha256::digest(&root))
            });
            match serde_json::to_string_pretty(&result) {
                Ok(result) => println!("{result}"),
                Err(_) => fail("ReceiptInvalid"),
            }
        }
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
