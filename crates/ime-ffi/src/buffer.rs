#[repr(C)]
pub struct RadishLexBuffer {
    data: *mut u8,
    len: usize,
    capacity: usize,
}

impl RadishLexBuffer {
    pub fn from_bytes(mut bytes: Vec<u8>) -> *mut Self {
        let buffer = Self {
            data: bytes.as_mut_ptr(),
            len: bytes.len(),
            capacity: bytes.capacity(),
        };
        std::mem::forget(bytes);
        Box::into_raw(Box::new(buffer))
    }

    pub fn from_string(value: impl Into<String>) -> *mut Self {
        Self::from_bytes(value.into().into_bytes())
    }

    pub fn data(&self) -> *const u8 {
        self.data.cast_const()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub unsafe fn free(buffer: *mut Self) {
        if buffer.is_null() {
            return;
        }

        let buffer = Box::from_raw(buffer);
        if !buffer.data.is_null() {
            let _ = Vec::from_raw_parts(buffer.data, buffer.len, buffer.capacity);
        }
    }
}
