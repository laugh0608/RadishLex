pub const RADISHLEX_ABI_CONTRACT_VERSION: u32 = 1;
pub const RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD: u32 = 1;
pub const RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexFfiContract {
    pub version: u32,
    pub session_thread_policy: u32,
    pub panic_boundary: u32,
}

impl RadishLexFfiContract {
    pub const fn current() -> Self {
        Self {
            version: RADISHLEX_ABI_CONTRACT_VERSION,
            session_thread_policy: RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD,
            panic_boundary: RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND,
        }
    }

    pub const fn empty() -> Self {
        Self {
            version: 0,
            session_thread_policy: 0,
            panic_boundary: 0,
        }
    }
}
