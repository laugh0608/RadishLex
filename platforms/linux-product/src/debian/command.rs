//! Typed Debian command and lifecycle contracts.
//!
//! This module deliberately does not execute commands. The privileged adapter
//! must separately prove filesystem identity, read the dpkg configuration, and
//! validate package state before and after consuming one of these invocations.

use std::fmt;
use std::path::{Path, PathBuf};

pub const DPKG_PROGRAM: &str = "/usr/bin/dpkg";
pub const DPKG_PACKAGE: &str = "radishlex";
pub const DPKG_ARCHITECTURE: &str = "arm64";
pub const DPKG_STATE_ROOT: &str = "/var/lib/radishlex/install-v1";
pub const NULL_DEVICE: &str = "/dev/null";
pub const MAX_DPKG_CONFIG_BYTES: usize = 64 * 1024;
pub const MAX_DPKG_CONFIG_LINES: usize = 512;
pub const MAX_DPKG_CONFIG_LINE_BYTES: usize = 512;
pub const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;

const CLEAN_ENVIRONMENT: [(&str, &str); 6] = [
    ("DEBIAN_FRONTEND", "noninteractive"),
    ("DPKG_COLORS", "never"),
    ("DPKG_NLS", "0"),
    ("LANG", "C.UTF-8"),
    ("LC_ALL", "C.UTF-8"),
    ("PATH", "/usr/sbin:/usr/bin:/sbin:/bin"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianInvocationPhase {
    ArchitecturePreflight,
    VersionComparison,
    TargetApply,
    TargetRemove,
    SourceRestore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedDebSlot {
    Source,
    Target,
}

impl StagedDebSlot {
    const fn filename(self) -> &'static str {
        match self {
            Self::Source => "source.deb",
            Self::Target => "target.deb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateStagedDeb {
    operation_id: String,
    slot: StagedDebSlot,
    path: PathBuf,
}

impl PrivateStagedDeb {
    pub fn new(
        operation_id: impl Into<String>,
        slot: StagedDebSlot,
    ) -> Result<Self, DebianCommandContractError> {
        let operation_id = operation_id.into();
        if operation_id.len() != 32
            || !operation_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(DebianCommandContractError::InvalidOperationId);
        }
        let path = Path::new(DPKG_STATE_ROOT)
            .join("operations")
            .join(&operation_id)
            .join(slot.filename());
        Ok(Self {
            operation_id,
            slot,
            path,
        })
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub const fn slot(&self) -> StagedDebSlot {
        self.slot
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebianVersion(String);

impl DebianVersion {
    pub fn new(value: impl Into<String>) -> Result<Self, DebianCommandContractError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 96
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'.' | b'+' | b':' | b'~' | b'_' | b'-')
            })
        {
            return Err(DebianCommandContractError::InvalidDebianVersion);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianVersionComparison {
    Earlier,
    Same,
    Later,
}

impl DebianVersionComparison {
    const fn operator(self) -> &'static str {
        match self {
            Self::Earlier => "lt",
            Self::Same => "eq",
            Self::Later => "gt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardInputPolicy {
    NullDevice,
}

impl StandardInputPolicy {
    pub const fn path(self) -> &'static str {
        match self {
            Self::NullDevice => NULL_DEVICE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedDiagnostics {
    pub stdout_limit: usize,
    pub stderr_limit: usize,
}

impl BoundedDiagnostics {
    const fn dpkg_default() -> Self {
        Self {
            stdout_limit: MAX_DIAGNOSTIC_BYTES,
            stderr_limit: MAX_DIAGNOSTIC_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebianCommandInvocation {
    phase: DebianInvocationPhase,
    program: &'static str,
    arguments: Vec<String>,
    clear_environment: bool,
    environment: Vec<(&'static str, &'static str)>,
    standard_input: StandardInputPolicy,
    diagnostics: BoundedDiagnostics,
}

impl DebianCommandInvocation {
    pub fn print_architecture() -> Self {
        Self::dpkg(
            DebianInvocationPhase::ArchitecturePreflight,
            vec!["--print-architecture".to_owned()],
        )
    }

    pub fn compare_versions(
        left: &DebianVersion,
        comparison: DebianVersionComparison,
        right: &DebianVersion,
    ) -> Self {
        Self::dpkg(
            DebianInvocationPhase::VersionComparison,
            vec![
                "--compare-versions".to_owned(),
                left.as_str().to_owned(),
                comparison.operator().to_owned(),
                right.as_str().to_owned(),
            ],
        )
    }

    pub fn apply_target(staged: &PrivateStagedDeb) -> Result<Self, DebianCommandContractError> {
        if staged.slot() != StagedDebSlot::Target {
            return Err(DebianCommandContractError::WrongStagedSlot);
        }
        Ok(Self::install(DebianInvocationPhase::TargetApply, staged))
    }

    pub fn restore_staged(staged: &PrivateStagedDeb) -> Self {
        Self::install(DebianInvocationPhase::SourceRestore, staged)
    }

    pub fn remove_target() -> Self {
        Self::dpkg(
            DebianInvocationPhase::TargetRemove,
            vec!["--remove".to_owned(), DPKG_PACKAGE.to_owned()],
        )
    }

    pub const fn phase(&self) -> DebianInvocationPhase {
        self.phase
    }

    pub const fn program(&self) -> &str {
        self.program
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    pub const fn clears_environment(&self) -> bool {
        self.clear_environment
    }

    pub fn environment(&self) -> &[(&'static str, &'static str)] {
        &self.environment
    }

    pub const fn standard_input(&self) -> StandardInputPolicy {
        self.standard_input
    }

    pub const fn diagnostics(&self) -> BoundedDiagnostics {
        self.diagnostics
    }

    fn install(phase: DebianInvocationPhase, staged: &PrivateStagedDeb) -> Self {
        Self::dpkg(
            phase,
            vec![
                "--install".to_owned(),
                staged.path().to_string_lossy().into_owned(),
            ],
        )
    }

    fn dpkg(phase: DebianInvocationPhase, arguments: Vec<String>) -> Self {
        Self {
            phase,
            program: DPKG_PROGRAM,
            arguments,
            clear_environment: true,
            environment: CLEAN_ENVIRONMENT.to_vec(),
            standard_input: StandardInputPolicy::NullDevice,
            diagnostics: BoundedDiagnostics::dpkg_default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpkgConfigurationErrorKind {
    TooLarge,
    TooManyLines,
    LineTooLong,
    ProhibitedOption,
    UnknownOption,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpkgConfigurationError {
    kind: DpkgConfigurationErrorKind,
    line: Option<usize>,
}

impl DpkgConfigurationError {
    pub const fn kind(&self) -> DpkgConfigurationErrorKind {
        self.kind
    }

    pub const fn line(&self) -> Option<usize> {
        self.line
    }

    const fn global(kind: DpkgConfigurationErrorKind) -> Self {
        Self { kind, line: None }
    }

    const fn at_line(kind: DpkgConfigurationErrorKind, line: usize) -> Self {
        Self {
            kind,
            line: Some(line),
        }
    }
}

impl fmt::Display for DpkgConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "dpkg configuration rejected at line {line}"),
            None => formatter.write_str("dpkg configuration rejected"),
        }
    }
}

impl std::error::Error for DpkgConfigurationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDpkgConfiguration {
    options: Vec<String>,
}

impl ValidatedDpkgConfiguration {
    pub fn options(&self) -> &[String] {
        &self.options
    }
}

pub fn validate_dpkg_configuration(
    value: &str,
) -> Result<ValidatedDpkgConfiguration, DpkgConfigurationError> {
    if value.len() > MAX_DPKG_CONFIG_BYTES {
        return Err(DpkgConfigurationError::global(
            DpkgConfigurationErrorKind::TooLarge,
        ));
    }
    let line_count = value.lines().count();
    if line_count > MAX_DPKG_CONFIG_LINES {
        return Err(DpkgConfigurationError::global(
            DpkgConfigurationErrorKind::TooManyLines,
        ));
    }

    let mut options = Vec::new();
    for (index, raw_line) in value.lines().enumerate() {
        let line_number = index + 1;
        if raw_line.len() > MAX_DPKG_CONFIG_LINE_BYTES {
            return Err(DpkgConfigurationError::at_line(
                DpkgConfigurationErrorKind::LineTooLong,
                line_number,
            ));
        }
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if prohibited_dpkg_option(line) {
            return Err(DpkgConfigurationError::at_line(
                DpkgConfigurationErrorKind::ProhibitedOption,
                line_number,
            ));
        }
        if !matches!(line, "no-pager" | "log=/var/log/dpkg.log") {
            return Err(DpkgConfigurationError::at_line(
                DpkgConfigurationErrorKind::UnknownOption,
                line_number,
            ));
        }
        if !options.iter().any(|existing| existing == line) {
            options.push(line.to_owned());
        }
    }
    Ok(ValidatedDpkgConfiguration { options })
}

fn prohibited_dpkg_option(value: &str) -> bool {
    let option = value.strip_prefix("--").unwrap_or(value);
    let name = option
        .split_once(['=', ' ', '\t'])
        .map_or(option, |(name, _)| name);
    name == "root"
        || name == "admindir"
        || name == "instdir"
        || name == "pending"
        || name == "path-exclude"
        || name == "path-include"
        || name == "hook"
        || name == "pre-invoke"
        || name == "post-invoke"
        || name == "status-fd"
        || name == "status-logger"
        || name == "command-fd"
        || name.starts_with("force")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianLifecycleObservation {
    InvocationSucceeded,
    InvocationFailed,
    InvocationSignaled,
    TargetStateValidated,
    SourceStateValidated,
    ExternalMaintainerScript,
    ExternalTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianLifecycleProjection {
    PreflightSatisfied,
    AwaitTargetValidation,
    AwaitSourceValidation,
    TargetReadyForReceiptProof,
    SourceReadyForReceiptProof,
    RequireSourceRestore,
    RecoveryBlocked,
    NoProductCompletionEvidence,
    FailedClosed,
}

/// Projects dpkg lifecycle evidence without producing a completed product state.
///
/// In particular, maintainer scripts and triggers owned by dpkg or dependency
/// packages are never sufficient product evidence. Only the outer coordinator
/// may persist a terminal receipt after package and product validation.
pub const fn project_debian_lifecycle(
    phase: DebianInvocationPhase,
    observation: DebianLifecycleObservation,
) -> DebianLifecycleProjection {
    use DebianInvocationPhase as Phase;
    use DebianLifecycleObservation as Observation;
    use DebianLifecycleProjection as Projection;

    match observation {
        Observation::ExternalMaintainerScript | Observation::ExternalTrigger => {
            Projection::NoProductCompletionEvidence
        }
        Observation::InvocationSucceeded => match phase {
            Phase::ArchitecturePreflight | Phase::VersionComparison => {
                Projection::PreflightSatisfied
            }
            Phase::TargetApply | Phase::TargetRemove => Projection::AwaitTargetValidation,
            Phase::SourceRestore => Projection::AwaitSourceValidation,
        },
        Observation::InvocationFailed | Observation::InvocationSignaled => match phase {
            Phase::TargetApply | Phase::TargetRemove => Projection::RequireSourceRestore,
            Phase::SourceRestore => Projection::RecoveryBlocked,
            Phase::ArchitecturePreflight | Phase::VersionComparison => Projection::FailedClosed,
        },
        Observation::TargetStateValidated => match phase {
            Phase::TargetApply | Phase::TargetRemove => Projection::TargetReadyForReceiptProof,
            _ => Projection::FailedClosed,
        },
        Observation::SourceStateValidated => match phase {
            Phase::SourceRestore => Projection::SourceReadyForReceiptProof,
            _ => Projection::FailedClosed,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebianCommandContractError {
    InvalidOperationId,
    InvalidDebianVersion,
    WrongStagedSlot,
}

impl fmt::Display for DebianCommandContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidOperationId => "invalid Linux package operation id",
            Self::InvalidDebianVersion => "invalid Debian package version",
            Self::WrongStagedSlot => "dpkg target apply requires the target artifact slot",
        })
    }
}

impl std::error::Error for DebianCommandContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    const OPERATION_ID: &str = "0123456789abcdef0123456789abcdef";

    fn target() -> PrivateStagedDeb {
        PrivateStagedDeb::new(OPERATION_ID, StagedDebSlot::Target).expect("target staged deb")
    }

    fn source() -> PrivateStagedDeb {
        PrivateStagedDeb::new(OPERATION_ID, StagedDebSlot::Source).expect("source staged deb")
    }

    #[test]
    fn staged_deb_paths_are_fixed_under_the_private_operation_root() {
        assert_eq!(
            target().path(),
            Path::new(DPKG_STATE_ROOT)
                .join("operations")
                .join(OPERATION_ID)
                .join("target.deb")
        );
        assert_eq!(
            source().path(),
            Path::new(DPKG_STATE_ROOT)
                .join("operations")
                .join(OPERATION_ID)
                .join("source.deb")
        );
        for invalid in [
            "",
            "0123456789abcdef",
            "0123456789abcdef0123456789abcdeg",
            "0123456789ABCDEF0123456789ABCDEF",
            "../../../../../../tmp/radishlex",
        ] {
            assert_eq!(
                PrivateStagedDeb::new(invalid, StagedDebSlot::Target),
                Err(DebianCommandContractError::InvalidOperationId)
            );
        }
    }

    #[test]
    fn generated_invocations_use_only_the_fixed_dpkg_surface() {
        let old = DebianVersion::new("26.7.1+37-1").expect("old version");
        let new = DebianVersion::new("26.7.1+38-1").expect("new version");
        let invocations = [
            DebianCommandInvocation::print_architecture(),
            DebianCommandInvocation::compare_versions(&old, DebianVersionComparison::Earlier, &new),
            DebianCommandInvocation::apply_target(&target()).expect("target apply"),
            DebianCommandInvocation::restore_staged(&source()),
            DebianCommandInvocation::restore_staged(&target()),
            DebianCommandInvocation::remove_target(),
        ];

        for invocation in invocations {
            assert_eq!(invocation.program(), DPKG_PROGRAM);
            assert!(invocation.clears_environment());
            assert_eq!(invocation.environment(), CLEAN_ENVIRONMENT);
            for inherited in ["HOME", "TMPDIR", "SHELL", "LD_PRELOAD", "XDG_CONFIG_HOME"] {
                assert!(
                    invocation
                        .environment()
                        .iter()
                        .all(|(name, _)| *name != inherited),
                    "inherited environment must stay cleared: {inherited}"
                );
            }
            assert_eq!(invocation.standard_input(), StandardInputPolicy::NullDevice);
            assert_eq!(invocation.standard_input().path(), NULL_DEVICE);
            assert_eq!(
                invocation.diagnostics(),
                BoundedDiagnostics {
                    stdout_limit: MAX_DIAGNOSTIC_BYTES,
                    stderr_limit: MAX_DIAGNOSTIC_BYTES,
                }
            );
            let joined = invocation.arguments().join(" ");
            for forbidden in [
                "apt",
                "sh -c",
                "--force",
                "--root",
                "--admindir",
                "--instdir",
                "--pending",
                "--purge",
            ] {
                assert!(!joined.contains(forbidden), "forbidden argv: {joined}");
            }
        }

        assert_eq!(
            DebianCommandInvocation::apply_target(&source()),
            Err(DebianCommandContractError::WrongStagedSlot)
        );
    }

    #[test]
    fn command_arguments_and_phases_are_exact() {
        let target = target();
        let apply = DebianCommandInvocation::apply_target(&target).expect("target apply");
        assert_eq!(apply.phase(), DebianInvocationPhase::TargetApply);
        assert_eq!(
            apply.arguments(),
            ["--install", target.path().to_str().expect("ASCII path")]
        );

        let remove = DebianCommandInvocation::remove_target();
        assert_eq!(remove.phase(), DebianInvocationPhase::TargetRemove);
        assert_eq!(remove.arguments(), ["--remove", DPKG_PACKAGE]);

        let architecture = DebianCommandInvocation::print_architecture();
        assert_eq!(
            architecture.phase(),
            DebianInvocationPhase::ArchitecturePreflight
        );
        assert_eq!(architecture.arguments(), ["--print-architecture"]);
    }

    #[test]
    fn versions_are_bounded_before_forming_compare_argv() {
        assert!(DebianVersion::new("1:26.7.1+38-1~local").is_ok());
        for invalid in ["", "1 2", "1/2", "1\n2"] {
            assert_eq!(
                DebianVersion::new(invalid),
                Err(DebianCommandContractError::InvalidDebianVersion)
            );
        }
        assert_eq!(
            DebianVersion::new("1".repeat(97)),
            Err(DebianCommandContractError::InvalidDebianVersion)
        );
    }

    #[test]
    fn dpkg_configuration_accepts_only_inert_bounded_options() {
        let configuration = validate_dpkg_configuration(
            "# Debian default\n\nno-pager\nlog=/var/log/dpkg.log\nno-pager\n",
        )
        .expect("safe dpkg configuration");
        assert_eq!(
            configuration.options(),
            ["no-pager", "log=/var/log/dpkg.log"]
        );

        for prohibited in [
            "force-confnew",
            "--force-all",
            "root=/tmp/root",
            "admindir=/tmp/status",
            "instdir=/tmp/root",
            "pending",
            "path-exclude=/usr/share/doc/*",
            "path-include=/tmp/payload",
            "hook=/tmp/hook",
            "pre-invoke=/tmp/hook",
            "post-invoke=/tmp/hook",
            "status-fd=3",
            "status-logger=/tmp/logger",
            "command-fd=4",
        ] {
            let error = validate_dpkg_configuration(prohibited)
                .expect_err("prohibited option must fail closed");
            assert_eq!(error.kind(), DpkgConfigurationErrorKind::ProhibitedOption);
            assert_eq!(error.line(), Some(1));
        }

        let unknown =
            validate_dpkg_configuration("no-act").expect_err("unknown option must fail closed");
        assert_eq!(unknown.kind(), DpkgConfigurationErrorKind::UnknownOption);
    }

    #[test]
    fn dpkg_configuration_size_and_line_limits_are_fail_closed() {
        let oversized = "#".repeat(MAX_DPKG_CONFIG_BYTES + 1);
        assert_eq!(
            validate_dpkg_configuration(&oversized)
                .expect_err("oversized configuration")
                .kind(),
            DpkgConfigurationErrorKind::TooLarge
        );
        let too_many_lines = "#\n".repeat(MAX_DPKG_CONFIG_LINES + 1);
        assert_eq!(
            validate_dpkg_configuration(&too_many_lines)
                .expect_err("too many configuration lines")
                .kind(),
            DpkgConfigurationErrorKind::TooManyLines
        );
        let long_line = "x".repeat(MAX_DPKG_CONFIG_LINE_BYTES + 1);
        assert_eq!(
            validate_dpkg_configuration(&long_line)
                .expect_err("long configuration line")
                .kind(),
            DpkgConfigurationErrorKind::LineTooLong
        );
    }

    #[test]
    fn lifecycle_projection_never_treats_external_scripts_or_triggers_as_completion() {
        let phases = [
            DebianInvocationPhase::ArchitecturePreflight,
            DebianInvocationPhase::VersionComparison,
            DebianInvocationPhase::TargetApply,
            DebianInvocationPhase::TargetRemove,
            DebianInvocationPhase::SourceRestore,
        ];
        for phase in phases {
            for observation in [
                DebianLifecycleObservation::ExternalMaintainerScript,
                DebianLifecycleObservation::ExternalTrigger,
            ] {
                assert_eq!(
                    project_debian_lifecycle(phase, observation),
                    DebianLifecycleProjection::NoProductCompletionEvidence
                );
            }
        }
    }

    #[test]
    fn lifecycle_projection_requires_outer_target_or_source_validation() {
        use DebianInvocationPhase as Phase;
        use DebianLifecycleObservation as Observation;
        use DebianLifecycleProjection as Projection;

        let cases = [
            (
                Phase::TargetApply,
                Observation::InvocationSucceeded,
                Projection::AwaitTargetValidation,
            ),
            (
                Phase::TargetRemove,
                Observation::InvocationSucceeded,
                Projection::AwaitTargetValidation,
            ),
            (
                Phase::SourceRestore,
                Observation::InvocationSucceeded,
                Projection::AwaitSourceValidation,
            ),
            (
                Phase::TargetApply,
                Observation::InvocationFailed,
                Projection::RequireSourceRestore,
            ),
            (
                Phase::TargetRemove,
                Observation::InvocationSignaled,
                Projection::RequireSourceRestore,
            ),
            (
                Phase::SourceRestore,
                Observation::InvocationFailed,
                Projection::RecoveryBlocked,
            ),
            (
                Phase::SourceRestore,
                Observation::InvocationSignaled,
                Projection::RecoveryBlocked,
            ),
            (
                Phase::TargetApply,
                Observation::TargetStateValidated,
                Projection::TargetReadyForReceiptProof,
            ),
            (
                Phase::TargetRemove,
                Observation::TargetStateValidated,
                Projection::TargetReadyForReceiptProof,
            ),
            (
                Phase::SourceRestore,
                Observation::SourceStateValidated,
                Projection::SourceReadyForReceiptProof,
            ),
            (
                Phase::TargetApply,
                Observation::SourceStateValidated,
                Projection::FailedClosed,
            ),
            (
                Phase::SourceRestore,
                Observation::TargetStateValidated,
                Projection::FailedClosed,
            ),
        ];
        for (phase, observation, expected) in cases {
            assert_eq!(project_debian_lifecycle(phase, observation), expected);
        }
    }
}
