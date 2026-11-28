mod decision;
mod read_only_state;
pub(crate) mod record;
mod system_port;
mod types;

pub use decision::inspect_linux_startup;

pub(crate) fn public_receipt(
    paths: &LinuxStartupPaths,
) -> Result<Option<crate::model::LinuxInstallReceipt>, LinuxStartupPortError> {
    let view = read_only_state::ReadOnlyStateView::inspect(paths)
        .map_err(read_only_state::StartupInspectionError::port_error)?;
    view.read_receipt(paths)
        .map_err(read_only_state::StartupInspectionError::port_error)
}
pub(crate) use system_port::validate_system_product;
pub use system_port::LinuxSystemStartupPort;
pub(crate) use types::SYSTEM_DPKG_STATUS_PATH;
pub use types::{
    LinuxPackageObservation, LinuxStartupBuildIdentity, LinuxStartupComponent,
    LinuxStartupDecision, LinuxStartupOutcome, LinuxStartupPaths, LinuxStartupPort,
    LinuxStartupPortError, LinuxStartupPortErrorCode, LinuxStartupReason,
};
#[cfg(target_os = "linux")]
pub(crate) use types::{
    SYSTEM_FCITX_ADDON_PATH, SYSTEM_FCITX_FFI_PATH, SYSTEM_MANAGER_FFI_PATH, SYSTEM_MANAGER_PATH,
};

#[cfg(test)]
mod tests;
