#![allow(dead_code)]

use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::slice;
use std::str;
use std::sync::Mutex;

use crate::error::{FfiError, RadishLexStatusCode};
use crate::snapshot::RadishLexStringView;

pub(crate) const MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1: u32 = 1;
pub(crate) const MANAGER_SYNC_ACTION_RECOVERY_SETUP: u32 = 1;
pub(crate) const MANAGER_SYNC_ACTION_RECOVERY_RESTORE: u32 = 2;
pub(crate) const MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION: u32 = 3;
pub(crate) const MANAGER_SYNC_ACTION_DEVICE_REVOCATION: u32 = 4;

const CURRENT_PHASE_COMMAND_ERROR: &str = "sync_command_not_enabled_current_phase";
const CURRENT_PHASE_USER_SUMMARY: &str = "user_sync_entry_closed_current_phase";
const CURRENT_PHASE_DIAGNOSTICS_SUMMARY: &str = "not_executable_current_phase";
const RESULT_HANDLE_RELEASED_ERROR: &str = "manager_sync_command_result_handle_released";
const PANIC_BOUNDARY_ERROR: &str = "manager_sync_command_panic_boundary";
const SYNC_COMMAND_CONTEXT_LOCK_ERROR: &str = "manager_sync_command_context_lock_poisoned";
const SYNC_COMMAND_DOMAIN_DRAFT: &str = "manager_sync_write_domain";
const SYNC_COMMAND_DOMAIN_BUSY_ERROR: &str = "sync_domain_command_in_progress";

const FORBIDDEN_SUMMARY_FRAGMENTS: &[&str] = &[
    "secret-token",
    "Bearer ",
    "RADISHLEX-RECOVERY-CODE-SECRET",
    "short_code=",
    "signature_bytes",
    "wrapped_material_bytes",
    "payload_bytes=",
    "request_body",
    "response_body",
    "/synthetic/private",
];

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ManagerSyncCommandRequestRawDraft {
    pub schema_version: u32,
    pub action_id: u32,
    pub operation_id: RadishLexStringView,
    pub readiness_snapshot_id: RadishLexStringView,
    pub deployment_evidence_source_tag: RadishLexStringView,
    pub device_backend_gate: RadishLexStringView,
    pub explicit_user_start: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncCommandActionDraft {
    RecoverySetup,
    RecoveryRestore,
    JoinRequestAuthorization,
    DeviceRevocation,
}

impl ManagerSyncCommandActionDraft {
    fn from_id(action_id: u32) -> Result<Self, FfiError> {
        match action_id {
            MANAGER_SYNC_ACTION_RECOVERY_SETUP => Ok(Self::RecoverySetup),
            MANAGER_SYNC_ACTION_RECOVERY_RESTORE => Ok(Self::RecoveryRestore),
            MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION => Ok(Self::JoinRequestAuthorization),
            MANAGER_SYNC_ACTION_DEVICE_REVOCATION => Ok(Self::DeviceRevocation),
            _ => Err(FfiError::invalid_argument("unknown manager sync action id")),
        }
    }

    fn as_summary_code(self) -> &'static str {
        match self {
            Self::RecoverySetup => "recovery_setup",
            Self::RecoveryRestore => "recovery_restore",
            Self::JoinRequestAuthorization => "join_request_authorization",
            Self::DeviceRevocation => "device_revocation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandRequestDraft {
    schema_version: u32,
    action: ManagerSyncCommandActionDraft,
    operation_id: String,
    readiness_snapshot_id: String,
    deployment_evidence_source_tag: String,
    device_backend_gate: String,
    explicit_user_start: bool,
}

impl ManagerSyncCommandRequestDraft {
    pub(crate) fn from_raw(raw: ManagerSyncCommandRequestRawDraft) -> Result<Self, FfiError> {
        if raw.schema_version != MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1 {
            return Err(FfiError::invalid_argument(
                "unknown manager sync command schema version",
            ));
        }

        let action = ManagerSyncCommandActionDraft::from_id(raw.action_id)?;
        let explicit_user_start = read_ffi_bool(raw.explicit_user_start, "explicit_user_start")?;
        let operation_id = read_required_safe_utf8_view(raw.operation_id, "operation_id")?;
        let readiness_snapshot_id =
            read_required_safe_utf8_view(raw.readiness_snapshot_id, "readiness_snapshot_id")?;
        let deployment_evidence_source_tag = read_required_safe_utf8_view(
            raw.deployment_evidence_source_tag,
            "deployment_evidence_source_tag",
        )?;
        let device_backend_gate =
            read_required_safe_utf8_view(raw.device_backend_gate, "device_backend_gate")?;

        Ok(Self {
            schema_version: raw.schema_version,
            action,
            operation_id,
            readiness_snapshot_id,
            deployment_evidence_source_tag,
            device_backend_gate,
            explicit_user_start,
        })
    }

    pub(crate) fn action(&self) -> ManagerSyncCommandActionDraft {
        self.action
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncCommandStatusDraft {
    BlockedByReadiness,
}

impl ManagerSyncCommandStatusDraft {
    fn as_summary_code(self) -> &'static str {
        match self {
            Self::BlockedByReadiness => "blocked_by_readiness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerSyncRetryPolicyDraft {
    NotRetryable,
}

impl ManagerSyncRetryPolicyDraft {
    fn as_summary_code(self) -> &'static str {
        match self {
            Self::NotRetryable => "not_retryable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandEnvelopeDraft {
    pub action_summary_code: &'static str,
    pub command_status: &'static str,
    pub error_code: &'static str,
    pub retry_policy: &'static str,
    pub user_visible_summary_code: &'static str,
    pub diagnostics_summary_code: &'static str,
    pub next_required_evidence: &'static str,
    pub object_type_summary: &'static str,
    pub object_count_summary: usize,
    pub object_version_summary: &'static str,
    pub recorded_at_summary: &'static str,
}

impl ManagerSyncCommandEnvelopeDraft {
    fn current_phase_closed(action: ManagerSyncCommandActionDraft) -> Self {
        Self {
            action_summary_code: action.as_summary_code(),
            command_status: ManagerSyncCommandStatusDraft::BlockedByReadiness.as_summary_code(),
            error_code: CURRENT_PHASE_COMMAND_ERROR,
            retry_policy: ManagerSyncRetryPolicyDraft::NotRetryable.as_summary_code(),
            user_visible_summary_code: CURRENT_PHASE_USER_SUMMARY,
            diagnostics_summary_code: CURRENT_PHASE_DIAGNOSTICS_SUMMARY,
            next_required_evidence: CURRENT_PHASE_USER_SUMMARY,
            object_type_summary: "none",
            object_count_summary: 0,
            object_version_summary: "none",
            recorded_at_summary: "not_recorded_current_phase",
        }
    }

    fn summary_values(&self) -> [&str; 10] {
        [
            self.action_summary_code,
            self.command_status,
            self.error_code,
            self.retry_policy,
            self.user_visible_summary_code,
            self.diagnostics_summary_code,
            self.next_required_evidence,
            self.object_type_summary,
            self.object_version_summary,
            self.recorded_at_summary,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultDraft {
    pub abi_status: RadishLexStatusCode,
    pub envelope: ManagerSyncCommandEnvelopeDraft,
}

impl ManagerSyncCommandResultDraft {
    pub(crate) fn current_phase_closed(action: ManagerSyncCommandActionDraft) -> Self {
        Self {
            abi_status: RadishLexStatusCode::InvalidState,
            envelope: ManagerSyncCommandEnvelopeDraft::current_phase_closed(action),
        }
    }

    pub(crate) fn assert_safe_summary(&self) -> Result<(), FfiError> {
        for value in self.envelope.summary_values() {
            ensure_safe_summary_value(value, "manager_sync_command_result")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultViewDraft<'a> {
    pub abi_status: RadishLexStatusCode,
    pub action_summary_code: &'a str,
    pub command_status: &'a str,
    pub error_code: &'a str,
    pub retry_policy: &'a str,
    pub user_visible_summary_code: &'a str,
    pub diagnostics_summary_code: &'a str,
    pub next_required_evidence: &'a str,
    pub object_type_summary: &'a str,
    pub object_count_summary: usize,
    pub object_version_summary: &'a str,
    pub recorded_at_summary: &'a str,
}

impl<'a> ManagerSyncCommandResultViewDraft<'a> {
    fn from_result(result: &'a ManagerSyncCommandResultDraft) -> Self {
        Self {
            abi_status: result.abi_status,
            action_summary_code: result.envelope.action_summary_code,
            command_status: result.envelope.command_status,
            error_code: result.envelope.error_code,
            retry_policy: result.envelope.retry_policy,
            user_visible_summary_code: result.envelope.user_visible_summary_code,
            diagnostics_summary_code: result.envelope.diagnostics_summary_code,
            next_required_evidence: result.envelope.next_required_evidence,
            object_type_summary: result.envelope.object_type_summary,
            object_count_summary: result.envelope.object_count_summary,
            object_version_summary: result.envelope.object_version_summary,
            recorded_at_summary: result.envelope.recorded_at_summary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagerSyncCommandResultHandleDraft {
    result: Option<ManagerSyncCommandResultDraft>,
}

impl ManagerSyncCommandResultHandleDraft {
    pub(crate) fn new(result: ManagerSyncCommandResultDraft) -> Result<Self, FfiError> {
        result.assert_safe_summary()?;
        Ok(Self {
            result: Some(result),
        })
    }

    pub(crate) fn view(&self) -> Result<ManagerSyncCommandResultViewDraft<'_>, FfiError> {
        let result = self
            .result
            .as_ref()
            .ok_or_else(|| FfiError::invalid_state(RESULT_HANDLE_RELEASED_ERROR))?;
        Ok(ManagerSyncCommandResultViewDraft::from_result(result))
    }

    pub(crate) fn release(&mut self) {
        self.result = None;
    }
}

pub(crate) fn release_manager_sync_command_result_handle_draft(
    handle: Option<&mut ManagerSyncCommandResultHandleDraft>,
) {
    if let Some(handle) = handle {
        handle.release();
    }
}

#[derive(Debug, Default)]
pub(crate) struct ManagerSyncCommandContextDraft {
    active_domains: Mutex<HashSet<&'static str>>,
}

impl ManagerSyncCommandContextDraft {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn enter_write_domain(
        &self,
    ) -> Result<ManagerSyncCommandDomainGuardDraft<'_>, FfiError> {
        let mut active_domains = self
            .active_domains
            .lock()
            .map_err(|_| FfiError::internal(SYNC_COMMAND_CONTEXT_LOCK_ERROR))?;

        if active_domains.contains(SYNC_COMMAND_DOMAIN_DRAFT) {
            return Err(FfiError::invalid_state(SYNC_COMMAND_DOMAIN_BUSY_ERROR));
        }

        active_domains.insert(SYNC_COMMAND_DOMAIN_DRAFT);
        Ok(ManagerSyncCommandDomainGuardDraft {
            context: self,
            domain: SYNC_COMMAND_DOMAIN_DRAFT,
        })
    }

    fn evaluate_current_phase(
        &self,
        raw: ManagerSyncCommandRequestRawDraft,
    ) -> Result<ManagerSyncCommandResultDraft, FfiError> {
        let request = ManagerSyncCommandRequestDraft::from_raw(raw)?;
        let _domain_guard = self.enter_write_domain()?;
        let result = ManagerSyncCommandResultDraft::current_phase_closed(request.action());
        result.assert_safe_summary()?;
        Ok(result)
    }
}

#[derive(Debug)]
pub(crate) struct ManagerSyncCommandDomainGuardDraft<'a> {
    context: &'a ManagerSyncCommandContextDraft,
    domain: &'static str,
}

impl Drop for ManagerSyncCommandDomainGuardDraft<'_> {
    fn drop(&mut self) {
        if let Ok(mut active_domains) = self.context.active_domains.lock() {
            active_domains.remove(self.domain);
        }
    }
}

pub(crate) fn execute_manager_sync_command_current_phase_with_context_draft(
    context: &ManagerSyncCommandContextDraft,
    raw: ManagerSyncCommandRequestRawDraft,
) -> Result<ManagerSyncCommandResultHandleDraft, FfiError> {
    capture_manager_sync_command_panic_boundary_draft(|| context.evaluate_current_phase(raw))
}

pub(crate) fn capture_manager_sync_command_panic_boundary_draft<F>(
    operation: F,
) -> Result<ManagerSyncCommandResultHandleDraft, FfiError>
where
    F: FnOnce() -> Result<ManagerSyncCommandResultDraft, FfiError>,
{
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(result)) => ManagerSyncCommandResultHandleDraft::new(result),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(FfiError::internal(PANIC_BOUNDARY_ERROR)),
    }
}

pub(crate) fn evaluate_manager_sync_command_current_phase_draft(
    raw: ManagerSyncCommandRequestRawDraft,
) -> Result<ManagerSyncCommandResultDraft, FfiError> {
    let request = ManagerSyncCommandRequestDraft::from_raw(raw)?;
    let result = ManagerSyncCommandResultDraft::current_phase_closed(request.action());
    result.assert_safe_summary()?;
    Ok(result)
}

fn read_required_safe_utf8_view(
    view: RadishLexStringView,
    field: &'static str,
) -> Result<String, FfiError> {
    let value = read_utf8_view(view, field)?;
    if value.is_empty() {
        return Err(FfiError::invalid_argument(format!(
            "{field} cannot be empty"
        )));
    }
    ensure_safe_summary_value(value, field)?;
    Ok(value.to_owned())
}

fn read_utf8_view<'a>(view: RadishLexStringView, field: &'static str) -> Result<&'a str, FfiError> {
    if view.data.is_null() {
        if view.len == 0 {
            return Ok("");
        }
        return Err(FfiError::invalid_argument(format!(
            "{field} pointer is null with non-zero length"
        )));
    }

    let bytes = unsafe { slice::from_raw_parts(view.data, view.len) };
    str::from_utf8(bytes)
        .map_err(|_| FfiError::invalid_argument(format!("{field} must be valid UTF-8")))
}

fn read_ffi_bool(value: u8, field: &'static str) -> Result<bool, FfiError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(FfiError::invalid_argument(format!(
            "{field} must be 0 or 1"
        ))),
    }
}

fn ensure_safe_summary_value(value: &str, field: &'static str) -> Result<(), FfiError> {
    if FORBIDDEN_SUMMARY_FRAGMENTS
        .iter()
        .any(|fragment| value.contains(fragment))
    {
        return Err(FfiError::invalid_argument(format!(
            "{field} contains forbidden material"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(value: &str) -> RadishLexStringView {
        RadishLexStringView {
            data: value.as_ptr(),
            len: value.len(),
        }
    }

    fn bytes_view(bytes: &[u8]) -> RadishLexStringView {
        RadishLexStringView {
            data: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    fn valid_raw(action_id: u32) -> ManagerSyncCommandRequestRawDraft {
        ManagerSyncCommandRequestRawDraft {
            schema_version: MANAGER_SYNC_COMMAND_SCHEMA_VERSION_V1,
            action_id,
            operation_id: view("op_test_non_secret_001"),
            readiness_snapshot_id: view("readiness_snapshot_test_001"),
            deployment_evidence_source_tag: view("local_smoke"),
            device_backend_gate: view("blocked"),
            explicit_user_start: 1,
        }
    }

    #[test]
    fn rejects_unknown_schema_before_command_context() {
        let raw = ManagerSyncCommandRequestRawDraft {
            schema_version: 999,
            ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
        };

        let error = ManagerSyncCommandRequestDraft::from_raw(raw).unwrap_err();

        assert_eq!(error.code, RadishLexStatusCode::InvalidArgument);
        assert_eq!(error.message, "unknown manager sync command schema version");
    }

    #[test]
    fn rejects_unknown_action_before_command_context() {
        let raw = ManagerSyncCommandRequestRawDraft {
            action_id: 999,
            ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
        };

        let error = ManagerSyncCommandRequestDraft::from_raw(raw).unwrap_err();

        assert_eq!(error.code, RadishLexStatusCode::InvalidArgument);
        assert_eq!(error.message, "unknown manager sync action id");
    }

    #[test]
    fn rejects_invalid_bool_and_utf8_without_echoing_input() {
        let invalid_bool = ManagerSyncCommandRequestRawDraft {
            explicit_user_start: 7,
            ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
        };
        let invalid_bool_error =
            ManagerSyncCommandRequestDraft::from_raw(invalid_bool).unwrap_err();
        assert_eq!(
            invalid_bool_error.code,
            RadishLexStatusCode::InvalidArgument
        );
        assert_eq!(
            invalid_bool_error.message,
            "explicit_user_start must be 0 or 1"
        );

        let invalid_utf8_bytes = [0xff, 0xfe, 0xfd];
        let invalid_utf8 = ManagerSyncCommandRequestRawDraft {
            operation_id: bytes_view(&invalid_utf8_bytes),
            ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
        };
        let invalid_utf8_error =
            ManagerSyncCommandRequestDraft::from_raw(invalid_utf8).unwrap_err();
        assert_eq!(
            invalid_utf8_error.code,
            RadishLexStatusCode::InvalidArgument
        );
        assert_eq!(
            invalid_utf8_error.message,
            "operation_id must be valid UTF-8"
        );
    }

    #[test]
    fn current_phase_gate_returns_stable_invalid_state_summary() {
        let result = evaluate_manager_sync_command_current_phase_draft(valid_raw(
            MANAGER_SYNC_ACTION_JOIN_REQUEST_AUTHORIZATION,
        ))
        .unwrap();

        assert_eq!(result.abi_status, RadishLexStatusCode::InvalidState);
        assert_eq!(
            result.envelope.action_summary_code,
            "join_request_authorization"
        );
        assert_eq!(result.envelope.command_status, "blocked_by_readiness");
        assert_eq!(result.envelope.error_code, CURRENT_PHASE_COMMAND_ERROR);
        assert_eq!(result.envelope.retry_policy, "not_retryable");
        assert_eq!(
            result.envelope.diagnostics_summary_code,
            CURRENT_PHASE_DIAGNOSTICS_SUMMARY
        );
        assert_eq!(result.envelope.object_type_summary, "none");
        assert_eq!(result.envelope.object_count_summary, 0);
        result.assert_safe_summary().unwrap();
    }

    #[test]
    fn result_handle_views_are_copyable_until_release() {
        let context = ManagerSyncCommandContextDraft::new();
        let mut handle = execute_manager_sync_command_current_phase_with_context_draft(
            &context,
            valid_raw(MANAGER_SYNC_ACTION_RECOVERY_RESTORE),
        )
        .unwrap();

        let view = handle.view().unwrap();
        assert_eq!(view.abi_status, RadishLexStatusCode::InvalidState);
        assert_eq!(view.action_summary_code, "recovery_restore");
        assert_eq!(view.command_status, "blocked_by_readiness");
        assert_eq!(view.error_code, CURRENT_PHASE_COMMAND_ERROR);
        assert_eq!(view.retry_policy, "not_retryable");
        assert_eq!(view.object_count_summary, 0);
        let copied_summary = view.user_visible_summary_code.to_owned();

        release_manager_sync_command_result_handle_draft(Some(&mut handle));
        let released_error = handle.view().unwrap_err();
        assert_eq!(released_error.code, RadishLexStatusCode::InvalidState);
        assert_eq!(released_error.message, RESULT_HANDLE_RELEASED_ERROR);
        assert_eq!(copied_summary, CURRENT_PHASE_USER_SUMMARY);

        release_manager_sync_command_result_handle_draft(None);
    }

    #[test]
    fn panic_boundary_maps_unwind_to_internal_error() {
        let error = capture_manager_sync_command_panic_boundary_draft(|| {
            panic!("synthetic manager sync panic with secret-token")
        })
        .unwrap_err();

        assert_eq!(error.code, RadishLexStatusCode::InternalError);
        assert_eq!(error.message, PANIC_BOUNDARY_ERROR);
        assert!(!error.message.contains("secret-token"));
    }

    #[test]
    fn command_context_rejects_same_domain_overlap_and_releases_guard() {
        let context = ManagerSyncCommandContextDraft::new();
        let first_guard = context.enter_write_domain().unwrap();

        let busy_error = context.enter_write_domain().unwrap_err();
        assert_eq!(busy_error.code, RadishLexStatusCode::InvalidState);
        assert_eq!(busy_error.message, SYNC_COMMAND_DOMAIN_BUSY_ERROR);

        drop(first_guard);
        let released_guard = context.enter_write_domain().unwrap();
        drop(released_guard);
    }

    #[test]
    fn forbidden_material_is_rejected_without_echoing_value() {
        for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
            let raw = ManagerSyncCommandRequestRawDraft {
                operation_id: view(forbidden),
                ..valid_raw(MANAGER_SYNC_ACTION_RECOVERY_SETUP)
            };
            let error = ManagerSyncCommandRequestDraft::from_raw(raw).unwrap_err();

            assert_eq!(error.code, RadishLexStatusCode::InvalidArgument);
            assert_eq!(error.message, "operation_id contains forbidden material");
            assert!(!error.message.contains(forbidden));
        }
    }

    #[test]
    fn result_debug_does_not_include_request_fields_or_forbidden_material() {
        let result = evaluate_manager_sync_command_current_phase_draft(valid_raw(
            MANAGER_SYNC_ACTION_DEVICE_REVOCATION,
        ))
        .unwrap();
        let debug = format!("{result:?}");

        assert!(!debug.contains("op_test_non_secret_001"));
        assert!(!debug.contains("readiness_snapshot_test_001"));
        assert!(!debug.contains("local_smoke"));
        assert!(!debug.contains("blocked\""));
        for forbidden in FORBIDDEN_SUMMARY_FRAGMENTS {
            assert!(!debug.contains(forbidden), "{forbidden}");
        }
    }
}
