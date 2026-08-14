mod decision;
mod read_only_state;
mod system_port;
mod types;

pub use decision::inspect_linux_startup;
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
