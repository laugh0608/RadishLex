//! Dedicated read-only observation. A legacy target-only response is rejected.
use radishlex_ime_product_upgrade::PreparationDirectoryIdentity;
use serde::Deserialize;

use crate::{HostMode, MacOsProductPreflightAdapter, MacOsUpgradeAdapterError};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancellationPreflight {
    format: String,
    result: String,
    available_bytes: u64,
    quiescent: bool,
    data_root: PreparationDirectoryIdentity,
    manager_bundle_id: String,
    input_method_bundle_id: String,
}

impl MacOsProductPreflightAdapter {
    /// The caller separately verifies installed source bundles and task/proof
    /// authority. This host observes the OS-selected user root and fixed bundle
    /// IDs; it accepts no caller-supplied path and never opens SQLite.
    pub fn inspect_cancellation_quiescence(
        &mut self,
        root: &PreparationDirectoryIdentity,
        target_manifest_sha256: &str,
    ) -> Result<(), MacOsUpgradeAdapterError> {
        self.product.verify_manifest(target_manifest_sha256)?;
        let executable = self
            .product
            .preflight()
            .ok_or(MacOsUpgradeAdapterError::InvalidManifest)?;
        executable.revalidate()?;
        let output = self.runner.run(executable, HostMode::CancellationPreflight);
        executable.revalidate()?;
        self.product.verify_manifest(target_manifest_sha256)?;
        if !output.success || !output.stderr.is_empty() {
            return Err(MacOsUpgradeAdapterError::PreflightRejected);
        }
        let value: CancellationPreflight = serde_json::from_slice(&output.stdout)
            .map_err(|_| MacOsUpgradeAdapterError::PreflightRejected)?;
        if value.format != "radishlex-cancellation-preflight-v1"
            || value.result != "ready"
            || !value.quiescent
            || value.available_bytes == 0
            || &value.data_root != root
            || root.inode == 0
            || root.mode != 0o700
            || value.manager_bundle_id != "dev.radishlex.radishlexManager"
            || value.input_method_bundle_id != "org.radishlex.inputmethod.macos"
        {
            return Err(MacOsUpgradeAdapterError::PreflightRejected);
        }
        Ok(())
    }
}
