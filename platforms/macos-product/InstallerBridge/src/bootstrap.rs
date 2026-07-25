use std::ffi::CStr;
use std::fs;
use std::os::raw::{c_char, c_int};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use radishlex_macos_product_install::{
    inspect_developer_id_application, CodeSignatureRequirements, RADISHLEX_DEVELOPER_TEAM_ID,
};
use serde::Deserialize;

const INSTALLER_EXECUTABLE_NAME: &str = "RadishLex Installer";
const INSTALLER_BUNDLE_NAME: &str = "RadishLex Installer.app";
const INSTALLER_BUNDLE_ID: &str = "org.radishlex.installer.macos";
const RELEASE_IDENTITY_NAME: &str = "ReleaseIdentity.json";
const PAYLOAD_DIRECTORY_NAME: &str = "InstallPayload";
const MAX_RELEASE_IDENTITY_BYTES: u64 = 16 * 1024;
const PASSWD_BUFFER_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerBootstrapError {
    CurrentUserUnavailable,
    UnsafeCurrentUserHome,
    UnsafeInstallerBundle,
    ReleaseIdentityUnavailable,
}

#[derive(Debug)]
pub struct InstallerBootstrapContext {
    owner_id: u32,
    user_home: PathBuf,
    bundle: PathBuf,
    resources: PathBuf,
    resource_owner_id: u32,
}

impl InstallerBootstrapContext {
    pub fn discover() -> Result<Self, InstallerBootstrapError> {
        let (owner_id, user_home) = current_user()?;
        let executable =
            std::env::current_exe().map_err(|_| InstallerBootstrapError::UnsafeInstallerBundle)?;
        Self::discover_from(&executable, owner_id, &user_home)
    }

    pub(crate) fn discover_from(
        executable: &Path,
        owner_id: u32,
        user_home: &Path,
    ) -> Result<Self, InstallerBootstrapError> {
        verify_user_home(user_home, owner_id)?;
        let executable = fs::canonicalize(executable)
            .map_err(|_| InstallerBootstrapError::UnsafeInstallerBundle)?;
        if executable.file_name().and_then(|value| value.to_str())
            != Some(INSTALLER_EXECUTABLE_NAME)
        {
            return Err(InstallerBootstrapError::UnsafeInstallerBundle);
        }
        let macos = executable
            .parent()
            .filter(|path| path.file_name().and_then(|value| value.to_str()) == Some("MacOS"))
            .ok_or(InstallerBootstrapError::UnsafeInstallerBundle)?;
        let contents = macos
            .parent()
            .filter(|path| path.file_name().and_then(|value| value.to_str()) == Some("Contents"))
            .ok_or(InstallerBootstrapError::UnsafeInstallerBundle)?;
        let bundle = contents
            .parent()
            .filter(|path| {
                path.file_name().and_then(|value| value.to_str()) == Some(INSTALLER_BUNDLE_NAME)
            })
            .ok_or(InstallerBootstrapError::UnsafeInstallerBundle)?;
        if fs::canonicalize(bundle).map_err(|_| InstallerBootstrapError::UnsafeInstallerBundle)?
            != bundle
        {
            return Err(InstallerBootstrapError::UnsafeInstallerBundle);
        }
        let executable_metadata = fs::symlink_metadata(&executable)
            .map_err(|_| InstallerBootstrapError::UnsafeInstallerBundle)?;
        if !executable_metadata.file_type().is_file()
            || executable_metadata.file_type().is_symlink()
            || executable_metadata.ino() == 0
            || executable_metadata.mode() & 0o022 != 0
        {
            return Err(InstallerBootstrapError::UnsafeInstallerBundle);
        }
        let resource_owner_id = executable_metadata.uid();
        let resources = contents.join("Resources");
        verify_directory(
            &resources,
            resource_owner_id,
            InstallerBootstrapError::UnsafeInstallerBundle,
        )?;
        Ok(Self {
            owner_id,
            user_home: user_home.to_path_buf(),
            bundle: bundle.to_path_buf(),
            resources,
            resource_owner_id,
        })
    }

    pub const fn owner_id(&self) -> u32 {
        self.owner_id
    }

    pub fn user_home(&self) -> &Path {
        &self.user_home
    }

    pub fn data_root(&self) -> PathBuf {
        self.user_home.join("Library/Application Support/RadishLex")
    }

    pub fn payload_root(&self) -> PathBuf {
        self.resources.join(PAYLOAD_DIRECTORY_NAME)
    }

    pub fn product_root(&self) -> PathBuf {
        self.payload_root().join("Product")
    }

    pub fn release_requirements(
        &self,
    ) -> Result<CodeSignatureRequirements, InstallerBootstrapError> {
        let identity = self.read_release_identity()?;
        let installer_identity =
            inspect_developer_id_application(&self.bundle, INSTALLER_BUNDLE_ID)
                .map_err(|_| InstallerBootstrapError::ReleaseIdentityUnavailable)?;
        if installer_identity.team_identifier() != identity.team_identifier
            || installer_identity.designated_requirement()
                != identity.installer_designated_requirement
        {
            return Err(InstallerBootstrapError::ReleaseIdentityUnavailable);
        }
        component_requirements(identity)
    }

    #[cfg(test)]
    pub fn unsealed_release_requirements_for_test(
        &self,
    ) -> Result<CodeSignatureRequirements, InstallerBootstrapError> {
        component_requirements(self.read_release_identity()?)
    }

    fn read_release_identity(&self) -> Result<ReleaseIdentity, InstallerBootstrapError> {
        let path = self.resources.join(RELEASE_IDENTITY_NAME);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| InstallerBootstrapError::ReleaseIdentityUnavailable)?;
        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != self.resource_owner_id
            || metadata.nlink() != 1
            || metadata.mode() & 0o022 != 0
            || metadata.len() == 0
            || metadata.len() > MAX_RELEASE_IDENTITY_BYTES
        {
            return Err(InstallerBootstrapError::ReleaseIdentityUnavailable);
        }
        let bytes =
            fs::read(path).map_err(|_| InstallerBootstrapError::ReleaseIdentityUnavailable)?;
        let identity: ReleaseIdentity = serde_json::from_slice(&bytes)
            .map_err(|_| InstallerBootstrapError::ReleaseIdentityUnavailable)?;
        if identity.format_version != 1 || identity.team_identifier != RADISHLEX_DEVELOPER_TEAM_ID {
            return Err(InstallerBootstrapError::ReleaseIdentityUnavailable);
        }
        Ok(identity)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseIdentity {
    format_version: u32,
    team_identifier: String,
    installer_designated_requirement: String,
    manager_designated_requirement: String,
    input_method_designated_requirement: String,
}

fn component_requirements(
    identity: ReleaseIdentity,
) -> Result<CodeSignatureRequirements, InstallerBootstrapError> {
    CodeSignatureRequirements::new(
        identity.team_identifier,
        identity.manager_designated_requirement,
        identity.input_method_designated_requirement,
    )
    .map_err(|_| InstallerBootstrapError::ReleaseIdentityUnavailable)
}

fn verify_user_home(path: &Path, owner_id: u32) -> Result<(), InstallerBootstrapError> {
    verify_directory(
        path,
        owner_id,
        InstallerBootstrapError::UnsafeCurrentUserHome,
    )
}

fn verify_directory(
    path: &Path,
    owner_id: u32,
    error: InstallerBootstrapError,
) -> Result<(), InstallerBootstrapError> {
    if !path.is_absolute() || fs::canonicalize(path).map_err(|_| error)? != path {
        return Err(error);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner_id
        || metadata.ino() == 0
        || metadata.mode() & 0o022 != 0
    {
        return Err(error);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn current_user() -> Result<(u32, PathBuf), InstallerBootstrapError> {
    let owner_id = unsafe { geteuid() };
    let mut password = MacOsPasswd::default();
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0_u8; PASSWD_BUFFER_BYTES];
    let status = unsafe {
        getpwuid_r(
            owner_id,
            &mut password,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() || password.pw_dir.is_null() {
        return Err(InstallerBootstrapError::CurrentUserUnavailable);
    }
    let bytes = unsafe { CStr::from_ptr(password.pw_dir) }.to_bytes();
    if bytes.is_empty() {
        return Err(InstallerBootstrapError::CurrentUserUnavailable);
    }
    use std::os::unix::ffi::OsStrExt;
    Ok((owner_id, PathBuf::from(std::ffi::OsStr::from_bytes(bytes))))
}

#[cfg(not(target_os = "macos"))]
fn current_user() -> Result<(u32, PathBuf), InstallerBootstrapError> {
    Err(InstallerBootstrapError::CurrentUserUnavailable)
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Default)]
struct MacOsPasswd {
    pw_name: *mut c_char,
    pw_passwd: *mut c_char,
    pw_uid: u32,
    pw_gid: u32,
    pw_change: i64,
    pw_class: *mut c_char,
    pw_gecos: *mut c_char,
    pw_dir: *mut c_char,
    pw_shell: *mut c_char,
    pw_expire: i64,
    pw_fields: c_int,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn geteuid() -> u32;
    fn getpwuid_r(
        uid: u32,
        pwd: *mut MacOsPasswd,
        buffer: *mut c_char,
        buffer_size: usize,
        result: *mut *mut MacOsPasswd,
    ) -> c_int;
}
