use std::process::{Command, Stdio};

use radishlex_ime_product_install::ProgramComponent;
use sha2::{Digest, Sha256};

use crate::{error, MacOsInstallAdapterError, MacOsInstallAdapterErrorCode};

const CODESIGN_PATH: &str = "/usr/bin/codesign";
const MAX_CODESIGN_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_REQUIREMENT_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsCodeIdentity {
    bundle_id: String,
    evidence_sha256: String,
}

impl MacOsCodeIdentity {
    pub fn new(
        bundle_id: impl Into<String>,
        evidence_sha256: impl Into<String>,
    ) -> Result<Self, MacOsInstallAdapterError> {
        let identity = Self {
            bundle_id: bundle_id.into(),
            evidence_sha256: evidence_sha256.into(),
        };
        if !valid_bundle_id(&identity.bundle_id) || !valid_sha256(&identity.evidence_sha256) {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        Ok(identity)
    }

    pub fn bundle_id(&self) -> &str {
        &self.bundle_id
    }

    pub fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }
}

pub trait MacOsCodeSignatureVerifier: Send + Sync {
    fn verify(
        &self,
        bundle: &std::path::Path,
        component: ProgramComponent,
        expected_bundle_id: &str,
    ) -> Result<MacOsCodeIdentity, MacOsInstallAdapterError>;
}

#[derive(Debug, Clone)]
pub struct CodeSignatureRequirements {
    team_identifier: String,
    manager_designated_requirement: String,
    input_method_designated_requirement: String,
}

impl CodeSignatureRequirements {
    pub fn new(
        team_identifier: impl Into<String>,
        manager_designated_requirement: impl Into<String>,
        input_method_designated_requirement: impl Into<String>,
    ) -> Result<Self, MacOsInstallAdapterError> {
        let requirements = Self {
            team_identifier: team_identifier.into(),
            manager_designated_requirement: manager_designated_requirement.into(),
            input_method_designated_requirement: input_method_designated_requirement.into(),
        };
        if requirements.team_identifier.len() != 10
            || !requirements
                .team_identifier
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            || !valid_developer_id_requirement(
                &requirements.manager_designated_requirement,
                &requirements.team_identifier,
            )
            || !valid_developer_id_requirement(
                &requirements.input_method_designated_requirement,
                &requirements.team_identifier,
            )
            || requirements.manager_designated_requirement
                == requirements.input_method_designated_requirement
        {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        Ok(requirements)
    }

    fn requirement(&self, component: ProgramComponent) -> &str {
        match component {
            ProgramComponent::Manager => &self.manager_designated_requirement,
            ProgramComponent::InputMethod => &self.input_method_designated_requirement,
        }
    }
}

#[derive(Debug)]
pub struct CodesignCodeSignatureVerifier {
    requirements: CodeSignatureRequirements,
}

#[cfg(feature = "qualification-harness")]
#[derive(Debug, Default)]
pub(crate) struct CodesignQualificationCodeSignatureVerifier;

impl CodesignCodeSignatureVerifier {
    pub const fn new(requirements: CodeSignatureRequirements) -> Self {
        Self { requirements }
    }
}

#[derive(Debug, Default)]
pub struct CodesignRunningIdentityInspector;

impl CodesignRunningIdentityInspector {
    pub fn inspect(
        &self,
        bundle: &std::path::Path,
        component: ProgramComponent,
    ) -> Result<MacOsCodeIdentity, MacOsInstallAdapterError> {
        if !bundle.is_absolute() {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        let verified = Command::new(CODESIGN_PATH)
            .env_clear()
            .args(["--verify", "--deep", "--strict"])
            .arg(bundle)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
        if !verified.success() {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        let parsed = inspect_code_identity(bundle)?;
        if !valid_developer_id_requirement(&parsed.designated_requirement, &parsed.team_identifier)
        {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        let evidence_sha256 = parsed.evidence_sha256(component);
        MacOsCodeIdentity::new(parsed.identifier, evidence_sha256)
    }
}

impl MacOsCodeSignatureVerifier for CodesignCodeSignatureVerifier {
    fn verify(
        &self,
        bundle: &std::path::Path,
        component: ProgramComponent,
        expected_bundle_id: &str,
    ) -> Result<MacOsCodeIdentity, MacOsInstallAdapterError> {
        if !bundle.is_absolute() || !valid_bundle_id(expected_bundle_id) {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        let requirement = self.requirements.requirement(component);
        let requirement_argument = format!("-R={requirement}");
        let verified = Command::new(CODESIGN_PATH)
            .env_clear()
            .args(["--verify", "--deep", "--strict"])
            .arg(requirement_argument)
            .arg(bundle)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
        if !verified.success() {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }

        let parsed = inspect_code_identity(bundle)?;
        if parsed.identifier != expected_bundle_id
            || parsed.team_identifier != self.requirements.team_identifier
            || parsed.designated_requirement != requirement
        {
            return Err(error(
                MacOsInstallAdapterErrorCode::SignatureIdentityChanged,
            ));
        }
        let evidence_sha256 = parsed.evidence_sha256(component);
        MacOsCodeIdentity::new(parsed.identifier, evidence_sha256)
    }
}

#[cfg(feature = "qualification-harness")]
impl MacOsCodeSignatureVerifier for CodesignQualificationCodeSignatureVerifier {
    fn verify(
        &self,
        bundle: &std::path::Path,
        component: ProgramComponent,
        expected_bundle_id: &str,
    ) -> Result<MacOsCodeIdentity, MacOsInstallAdapterError> {
        if !bundle.is_absolute() || !valid_bundle_id(expected_bundle_id) {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        let verified = Command::new(CODESIGN_PATH)
            .env_clear()
            .args(["--verify", "--deep", "--strict"])
            .arg(bundle)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
        if !verified.success() {
            return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
        }
        let parsed = inspect_qualification_code_identity(bundle)?;
        if parsed.identifier != expected_bundle_id {
            return Err(error(
                MacOsInstallAdapterErrorCode::SignatureIdentityChanged,
            ));
        }
        let evidence_sha256 = parsed.evidence_sha256(component);
        MacOsCodeIdentity::new(parsed.identifier, evidence_sha256)
    }
}

fn inspect_code_identity(
    bundle: &std::path::Path,
) -> Result<ParsedCodeIdentity, MacOsInstallAdapterError> {
    let output = Command::new(CODESIGN_PATH)
        .env_clear()
        .args(["-d", "--verbose=4", "-r-"])
        .arg(bundle)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
    if !output.status.success()
        || output.stdout.is_empty()
        || output.stdout.len() > MAX_REQUIREMENT_BYTES
        || output.stderr.len() > MAX_CODESIGN_OUTPUT_BYTES
    {
        return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
    }
    let details = combined_codesign_details(&output.stderr, &output.stdout)?;
    ParsedCodeIdentity::parse(&details)
}

fn combined_codesign_details(
    identity_output: &[u8],
    requirement_output: &[u8],
) -> Result<String, MacOsInstallAdapterError> {
    let identity = std::str::from_utf8(identity_output)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
    let requirement = std::str::from_utf8(requirement_output)
        .map_err(|_| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
    if identity.as_bytes().contains(&0)
        || requirement.as_bytes().contains(&0)
        || identity.contains('\r')
        || requirement.contains('\r')
    {
        return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
    }
    Ok(format!("{identity}\n{requirement}"))
}

#[cfg(feature = "qualification-harness")]
fn inspect_qualification_code_identity(
    bundle: &std::path::Path,
) -> Result<ParsedCodeIdentity, MacOsInstallAdapterError> {
    let output = Command::new(CODESIGN_PATH)
        .env_clear()
        .args(["-d", "--verbose=4", "-r-"])
        .arg(bundle)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
    if !output.status.success()
        || output.stdout.is_empty()
        || output.stdout.len() > MAX_REQUIREMENT_BYTES
        || output.stderr.len() > MAX_CODESIGN_OUTPUT_BYTES
    {
        return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
    }
    let details = combined_codesign_details(&output.stderr, &output.stdout)?;
    ParsedCodeIdentity::parse_qualification(&details)
}

struct ParsedCodeIdentity {
    identifier: String,
    team_identifier: String,
    cdhash: String,
    signature: String,
    code_directory: String,
    designated_requirement: String,
}

impl ParsedCodeIdentity {
    fn parse(details: &str) -> Result<Self, MacOsInstallAdapterError> {
        let parsed = Self {
            identifier: unique_value(details, "Identifier=")?,
            team_identifier: unique_value(details, "TeamIdentifier=")?,
            cdhash: unique_value(details, "CDHash=")?,
            signature: unique_line(details, "Signature")?,
            code_directory: unique_line(details, "CodeDirectory ")?,
            designated_requirement: unique_designated_requirement(details)?,
        };
        if parsed.has_valid_common_fields() && parsed.team_identifier.len() == 10 {
            Ok(parsed)
        } else {
            Err(error(MacOsInstallAdapterErrorCode::SignatureRejected))
        }
    }

    #[cfg(feature = "qualification-harness")]
    fn parse_qualification(details: &str) -> Result<Self, MacOsInstallAdapterError> {
        let parsed = Self {
            identifier: unique_value(details, "Identifier=")?,
            team_identifier: unique_value(details, "TeamIdentifier=")?,
            cdhash: unique_value(details, "CDHash=")?,
            signature: unique_line(details, "Signature")?,
            code_directory: unique_line(details, "CodeDirectory ")?,
            designated_requirement: unique_designated_requirement(details)?,
        };
        if parsed.has_valid_common_fields()
            && parsed.team_identifier == "not set"
            && parsed.signature == "Signature=adhoc"
            && parsed.code_directory.contains(" flags=0x2(adhoc) ")
            && valid_ad_hoc_requirement(&parsed.designated_requirement, &parsed.cdhash)
        {
            Ok(parsed)
        } else {
            Err(error(MacOsInstallAdapterErrorCode::SignatureRejected))
        }
    }

    fn has_valid_common_fields(&self) -> bool {
        valid_bundle_id(&self.identifier)
            && valid_cdhash(&self.cdhash)
            && !self.signature.is_empty()
            && self.signature.len() <= 256
            && self.code_directory.len() <= 1024
            && valid_requirement(&self.designated_requirement)
    }

    fn evidence_sha256(&self, component: ProgramComponent) -> String {
        let mut digest = Sha256::new();
        update_field(&mut digest, b"radishlex-macos-code-identity-v1");
        update_field(
            &mut digest,
            match component {
                ProgramComponent::Manager => b"manager",
                ProgramComponent::InputMethod => b"input_method",
            },
        );
        for field in [
            self.identifier.as_bytes(),
            self.team_identifier.as_bytes(),
            self.cdhash.as_bytes(),
            self.signature.as_bytes(),
            self.code_directory.as_bytes(),
            self.designated_requirement.as_bytes(),
        ] {
            update_field(&mut digest, field);
        }
        format!("{:x}", digest.finalize())
    }
}

#[cfg(feature = "qualification-harness")]
fn valid_ad_hoc_requirement(value: &str, primary_cdhash: &str) -> bool {
    let mut found_primary = false;
    let clauses: Vec<_> = value.split(" or ").collect();
    if clauses.is_empty() || clauses.len() > 16 {
        return false;
    }
    for clause in clauses {
        let Some(hash) = clause
            .strip_prefix("cdhash H\"")
            .and_then(|value| value.strip_suffix('"'))
        else {
            return false;
        };
        if !valid_cdhash(hash) {
            return false;
        }
        found_primary |= hash.eq_ignore_ascii_case(primary_cdhash);
    }
    found_primary
}

fn unique_value(details: &str, prefix: &str) -> Result<String, MacOsInstallAdapterError> {
    let line = unique_line(details, prefix)?;
    Ok(line[prefix.len()..].to_owned())
}

fn unique_designated_requirement(details: &str) -> Result<String, MacOsInstallAdapterError> {
    let mut matching = details.lines().filter_map(|line| {
        line.strip_prefix("# designated => ")
            .or_else(|| line.strip_prefix("designated => "))
    });
    let requirement = matching
        .next()
        .ok_or_else(|| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
    if matching.next().is_some() || !valid_requirement(requirement) {
        return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
    }
    Ok(requirement.to_owned())
}

fn unique_line(details: &str, prefix: &str) -> Result<String, MacOsInstallAdapterError> {
    let mut matching = details.lines().filter(|line| line.starts_with(prefix));
    let line = matching
        .next()
        .ok_or_else(|| error(MacOsInstallAdapterErrorCode::SignatureRejected))?;
    if matching.next().is_some()
        || line.len() == prefix.len()
        || line.as_bytes().contains(&0)
        || line.contains('\r')
    {
        return Err(error(MacOsInstallAdapterErrorCode::SignatureRejected));
    }
    Ok(line.to_owned())
}

fn valid_requirement(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUIREMENT_BYTES
        && !value.contains('\n')
        && !value.contains('\r')
        && !value.as_bytes().contains(&0)
}

fn valid_developer_id_requirement(value: &str, team_identifier: &str) -> bool {
    if !valid_requirement(value) {
        return false;
    }
    let clauses: Vec<_> = value.split(" and ").collect();
    if clauses.len() != 5
        || !valid_identifier_clause(clauses[0])
        || clauses[1] != "anchor apple generic"
        || strip_exists_comment(clauses[2]) != "certificate 1[field.1.2.840.113635.100.6.2.6]"
        || strip_exists_comment(clauses[3]) != "certificate leaf[field.1.2.840.113635.100.6.1.13]"
    {
        return false;
    }
    let quoted_team = format!("certificate leaf[subject.OU] = \"{team_identifier}\"");
    let unquoted_team = format!("certificate leaf[subject.OU] = {team_identifier}");
    clauses[4] == quoted_team || clauses[4] == unquoted_team
}

fn valid_identifier_clause(clause: &str) -> bool {
    clause
        .strip_prefix("identifier \"")
        .and_then(|value| value.strip_suffix('"'))
        .is_some_and(valid_bundle_id)
}

fn strip_exists_comment(clause: &str) -> &str {
    clause.strip_suffix(" /* exists */").unwrap_or(clause)
}

fn valid_cdhash(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_bundle_id(value: &str) -> bool {
    value.len() <= 255
        && value.split('.').count() >= 2
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn update_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_codesign_fields_into_stable_redacted_evidence() {
        let details = concat!(
            "Executable=/synthetic/Manager\n",
            "Identifier=dev.radishlex.radishlexManager\n",
            "Format=app bundle with Mach-O thin (arm64)\n",
            "CodeDirectory v=20500 size=100 flags=0x10000(runtime) hashes=1+7 location=embedded\n",
            "Signature size=9000\n",
            "Authority=Developer ID Application: Synthetic\n",
            "TeamIdentifier=ABCDEFGHIJ\n",
            "CDHash=0123456789abcdef0123456789abcdef01234567\n",
            "designated => identifier \"dev.radishlex.radishlexManager\" and anchor apple generic\n",
        );
        let parsed = ParsedCodeIdentity::parse(details).expect("parsed identity");
        let hash = parsed.evidence_sha256(ProgramComponent::Manager);
        assert!(valid_sha256(&hash));
        assert!(!hash.contains("ABCDEFGHIJ"));
        assert!(ParsedCodeIdentity::parse(&format!("{details}Identifier=duplicate\n")).is_err());
    }

    #[test]
    fn release_requirements_reject_ad_hoc_or_ambiguous_inputs() {
        let manager = concat!(
            "identifier \"dev.radishlex.radishlexManager\" and anchor apple generic and ",
            "certificate 1[field.1.2.840.113635.100.6.2.6] /* exists */ and ",
            "certificate leaf[field.1.2.840.113635.100.6.1.13] /* exists */ and ",
            "certificate leaf[subject.OU] = ABCDEFGHIJ",
        );
        let input_method = concat!(
            "identifier \"dev.radishlex.RadishLexInputMethod\" and anchor apple generic and ",
            "certificate 1[field.1.2.840.113635.100.6.2.6] /* exists */ and ",
            "certificate leaf[field.1.2.840.113635.100.6.1.13] /* exists */ and ",
            "certificate leaf[subject.OU] = \"ABCDEFGHIJ\"",
        );
        assert!(CodeSignatureRequirements::new("ABCDEFGHIJ", manager, input_method,).is_ok());
        assert!(CodeSignatureRequirements::new(
            "ABCDEFGHIJ",
            "identifier \"dev.radishlex.radishlexManager\" and anchor apple generic",
            input_method,
        )
        .is_err());
        assert!(CodeSignatureRequirements::new(
            "ABCDEFGHIJ",
            format!("{manager} or true"),
            input_method,
        )
        .is_err());
        assert!(CodeSignatureRequirements::new("ZZZZZZZZZZ", manager, input_method,).is_err());
    }

    #[cfg(feature = "qualification-harness")]
    #[test]
    fn qualification_parser_accepts_only_bounded_ad_hoc_identity() {
        let details = concat!(
            "Identifier=dev.radishlex.radishlexManager\n",
            "CodeDirectory v=20400 size=663 flags=0x2(adhoc) hashes=10+7 location=embedded\n",
            "CDHash=0123456789abcdef0123456789abcdef01234567\n",
            "Signature=adhoc\n",
            "TeamIdentifier=not set\n",
            "# designated => cdhash H\"0123456789abcdef0123456789abcdef01234567\"\n",
        );
        assert!(ParsedCodeIdentity::parse_qualification(details).is_ok());
        assert!(ParsedCodeIdentity::parse(details).is_err());
        assert!(ParsedCodeIdentity::parse_qualification(
            &details.replace("Signature=adhoc", "Signature size=9000")
        )
        .is_err());
        assert!(ParsedCodeIdentity::parse_qualification(
            &details.replace("flags=0x2(adhoc)", "flags=0x10000(runtime)")
        )
        .is_err());
    }

    #[test]
    fn combines_real_codesign_output_streams_without_ambiguity() {
        let stderr = concat!(
            "Identifier=dev.radishlex.radishlexManager\n",
            "CodeDirectory v=20400 size=663 flags=0x2(adhoc) hashes=10+7 location=embedded\n",
            "CDHash=0123456789abcdef0123456789abcdef01234567\n",
            "Signature=adhoc\n",
            "TeamIdentifier=not set\n",
        );
        let stdout = "# designated => cdhash H\"0123456789abcdef0123456789abcdef01234567\"\n";
        let details = combined_codesign_details(stderr.as_bytes(), stdout.as_bytes())
            .expect("combine codesign streams");
        #[cfg(feature = "qualification-harness")]
        assert!(ParsedCodeIdentity::parse_qualification(&details).is_ok());
        assert!(unique_designated_requirement(&format!("{details}{stdout}")).is_err());
    }
}
