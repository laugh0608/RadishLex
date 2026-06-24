use std::ffi::c_void;

pub type RimeSessionId = usize;

#[repr(C)]
pub struct RimeApi {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RimeContext {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RimeCommit {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RimeStatus {
    _private: [u8; 0],
}

pub type OpaqueRimePointer = *mut c_void;
