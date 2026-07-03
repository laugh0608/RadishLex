#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::model::CryptoError;

use super::{
    android_keystore_jni_method_spec, android_keystore_unavailable, AndroidKeystoreBridge,
    AndroidKeystoreBridgeErrorCode, AndroidKeystoreBridgeOperation, AndroidKeystoreBridgeRequest,
    DevicePrivateKeyStoreStatus, ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION,
    ANDROID_KEYSTORE_JNI_BRIDGE_CLASS, ANDROID_KEYSTORE_JNI_BYTE_ARRAY_METHOD_DESCRIPTOR,
    ANDROID_KEYSTORE_JNI_ERROR_CODE_METHOD_DESCRIPTOR, ANDROID_KEYSTORE_JNI_GET_ERROR_CODE_METHOD,
    ANDROID_KEYSTORE_JNI_GET_PUBLIC_KEY_METHOD, ANDROID_KEYSTORE_JNI_GET_SIGNATURE_METHOD,
};

#[allow(non_camel_case_types)]
type jint = i32;
#[allow(non_camel_case_types)]
type jsize = i32;
#[allow(non_camel_case_types)]
type jboolean = u8;
#[allow(non_camel_case_types)]
type jbyte = i8;
#[allow(non_camel_case_types)]
type jobject = *mut c_void;
#[allow(non_camel_case_types)]
type jclass = jobject;
#[allow(non_camel_case_types)]
type jstring = jobject;
#[allow(non_camel_case_types)]
type jbyteArray = jobject;
#[allow(non_camel_case_types)]
type jmethodID = *mut c_void;
#[allow(non_camel_case_types)]
type JNIEnv = *mut c_void;
#[allow(non_camel_case_types)]
type JavaVM = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
union JniValue {
    i: jint,
    l: jobject,
}

type GetEnv = unsafe extern "system" fn(JavaVM, *mut *mut c_void, jint) -> jint;
type AttachCurrentThread = unsafe extern "system" fn(JavaVM, *mut *mut c_void, *mut c_void) -> jint;
type DetachCurrentThread = unsafe extern "system" fn(JavaVM) -> jint;
type FindClass = unsafe extern "system" fn(JNIEnv, *const c_char) -> jclass;
type DeleteLocalRef = unsafe extern "system" fn(JNIEnv, jobject);
type GetObjectClass = unsafe extern "system" fn(JNIEnv, jobject) -> jclass;
type GetMethodId =
    unsafe extern "system" fn(JNIEnv, jclass, *const c_char, *const c_char) -> jmethodID;
type CallObjectMethodA =
    unsafe extern "system" fn(JNIEnv, jobject, jmethodID, *const JniValue) -> jobject;
type GetStaticMethodId =
    unsafe extern "system" fn(JNIEnv, jclass, *const c_char, *const c_char) -> jmethodID;
type CallStaticObjectMethodA =
    unsafe extern "system" fn(JNIEnv, jclass, jmethodID, *const JniValue) -> jobject;
type NewStringUtf = unsafe extern "system" fn(JNIEnv, *const c_char) -> jstring;
type GetStringUtfChars = unsafe extern "system" fn(JNIEnv, jstring, *mut jboolean) -> *const c_char;
type ReleaseStringUtfChars = unsafe extern "system" fn(JNIEnv, jstring, *const c_char);
type GetArrayLength = unsafe extern "system" fn(JNIEnv, jobject) -> jsize;
type NewByteArray = unsafe extern "system" fn(JNIEnv, jsize) -> jbyteArray;
type GetByteArrayRegion = unsafe extern "system" fn(JNIEnv, jbyteArray, jsize, jsize, *mut jbyte);
type SetByteArrayRegion = unsafe extern "system" fn(JNIEnv, jbyteArray, jsize, jsize, *const jbyte);
type ExceptionClear = unsafe extern "system" fn(JNIEnv);
type ExceptionCheck = unsafe extern "system" fn(JNIEnv) -> jboolean;

const JNI_OK: jint = 0;
const JNI_EDETACHED: jint = -2;
const JNI_VERSION_1_6: jint = 0x0001_0006;

const JVM_ATTACH_CURRENT_THREAD: usize = 4;
const JVM_DETACH_CURRENT_THREAD: usize = 5;
const JVM_GET_ENV: usize = 6;

const JNI_ENV_FIND_CLASS: usize = 6;
const JNI_ENV_EXCEPTION_CLEAR: usize = 17;
const JNI_ENV_DELETE_LOCAL_REF: usize = 23;
const JNI_ENV_GET_OBJECT_CLASS: usize = 31;
const JNI_ENV_GET_METHOD_ID: usize = 33;
const JNI_ENV_CALL_OBJECT_METHOD_A: usize = 36;
const JNI_ENV_GET_STATIC_METHOD_ID: usize = 113;
const JNI_ENV_CALL_STATIC_OBJECT_METHOD_A: usize = 116;
const JNI_ENV_NEW_STRING_UTF: usize = 167;
const JNI_ENV_GET_STRING_UTF_CHARS: usize = 169;
const JNI_ENV_RELEASE_STRING_UTF_CHARS: usize = 170;
const JNI_ENV_GET_ARRAY_LENGTH: usize = 171;
const JNI_ENV_NEW_BYTE_ARRAY: usize = 176;
const JNI_ENV_GET_BYTE_ARRAY_REGION: usize = 200;
const JNI_ENV_SET_BYTE_ARRAY_REGION: usize = 208;
const JNI_ENV_EXCEPTION_CHECK: usize = 228;

static ANDROID_JAVA_VM: AtomicUsize = AtomicUsize::new(0);

pub(super) struct JniAndroidKeystoreBridge;

impl JniAndroidKeystoreBridge {
    pub(super) fn new() -> Self {
        Self
    }

    fn call_for_public_key(
        &self,
        operation: AndroidKeystoreBridgeOperation,
        signing_key_id: &str,
        alias: &str,
    ) -> Result<Vec<u8>, CryptoError> {
        let _request = AndroidKeystoreBridgeRequest::new(operation, signing_key_id, alias, 0)?;
        let env = current_env()?;
        let result =
            unsafe { call_bridge_method(env.env(), operation, signing_key_id, alias, &[]) }?;
        let output = unsafe {
            read_result_bytes(
                env.env(),
                result,
                signing_key_id,
                ANDROID_KEYSTORE_JNI_GET_PUBLIC_KEY_METHOD,
            )
        };
        unsafe {
            delete_local_ref(env.env(), result);
        }
        output
    }

    fn call_for_empty_success(
        &self,
        operation: AndroidKeystoreBridgeOperation,
        signing_key_id: &str,
        alias: &str,
    ) -> Result<(), CryptoError> {
        let _request = AndroidKeystoreBridgeRequest::new(operation, signing_key_id, alias, 0)?;
        let env = current_env()?;
        let result =
            unsafe { call_bridge_method(env.env(), operation, signing_key_id, alias, &[]) }?;
        let output = unsafe { read_result_error(env.env(), result, signing_key_id) };
        unsafe {
            delete_local_ref(env.env(), result);
        }
        match output? {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl fmt::Debug for JniAndroidKeystoreBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JniAndroidKeystoreBridge")
    }
}

impl AndroidKeystoreBridge for JniAndroidKeystoreBridge {
    fn backend_status(&self) -> DevicePrivateKeyStoreStatus {
        DevicePrivateKeyStoreStatus::android_keystore_v1()
    }

    fn create_signing_key(
        &self,
        signing_key_id: &str,
        alias: &str,
    ) -> Result<Vec<u8>, CryptoError> {
        self.call_for_public_key(
            AndroidKeystoreBridgeOperation::CreateSigningKey,
            signing_key_id,
            alias,
        )
    }

    fn public_key(&self, signing_key_id: &str, alias: &str) -> Result<Vec<u8>, CryptoError> {
        self.call_for_public_key(
            AndroidKeystoreBridgeOperation::LoadPublicKey,
            signing_key_id,
            alias,
        )
    }

    fn sign(
        &self,
        signing_key_id: &str,
        alias: &str,
        canonical_bytes: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let _request = AndroidKeystoreBridgeRequest::new(
            AndroidKeystoreBridgeOperation::Sign,
            signing_key_id,
            alias,
            canonical_bytes.len(),
        )?;
        let env = current_env()?;
        let result = unsafe {
            call_bridge_method(
                env.env(),
                AndroidKeystoreBridgeOperation::Sign,
                signing_key_id,
                alias,
                canonical_bytes,
            )
        }?;
        let output = unsafe {
            read_result_bytes(
                env.env(),
                result,
                signing_key_id,
                ANDROID_KEYSTORE_JNI_GET_SIGNATURE_METHOD,
            )
        };
        unsafe {
            delete_local_ref(env.env(), result);
        }
        output
    }

    fn delete_signing_key(&self, signing_key_id: &str, alias: &str) -> Result<(), CryptoError> {
        self.call_for_empty_success(
            AndroidKeystoreBridgeOperation::DeleteSigningKey,
            signing_key_id,
            alias,
        )
    }
}

struct JniEnvGuard {
    vm: JavaVM,
    env: JNIEnv,
    attached_current_thread: bool,
}

impl JniEnvGuard {
    fn env(&self) -> JNIEnv {
        self.env
    }
}

impl Drop for JniEnvGuard {
    fn drop(&mut self) {
        if self.attached_current_thread {
            unsafe {
                if let Ok(detach) =
                    java_vm_fn::<DetachCurrentThread>(self.vm, JVM_DETACH_CURRENT_THREAD)
                {
                    let _ = detach(self.vm);
                }
            }
        }
    }
}

fn register_java_vm(vm: JavaVM) -> Result<(), CryptoError> {
    if vm.is_null() {
        return Err(android_keystore_unavailable());
    }
    ANDROID_JAVA_VM.store(vm as usize, Ordering::SeqCst);
    Ok(())
}

fn current_env() -> Result<JniEnvGuard, CryptoError> {
    let vm = ANDROID_JAVA_VM.load(Ordering::SeqCst) as JavaVM;
    if vm.is_null() {
        return Err(android_keystore_unavailable());
    }

    unsafe {
        let mut env = ptr::null_mut();
        let get_env = java_vm_fn::<GetEnv>(vm, JVM_GET_ENV)?;
        match get_env(vm, &mut env, JNI_VERSION_1_6) {
            JNI_OK if !env.is_null() => Ok(JniEnvGuard {
                vm,
                env: env.cast(),
                attached_current_thread: false,
            }),
            JNI_EDETACHED => {
                let attach = java_vm_fn::<AttachCurrentThread>(vm, JVM_ATTACH_CURRENT_THREAD)?;
                let status = attach(vm, &mut env, ptr::null_mut());
                if status == JNI_OK && !env.is_null() {
                    Ok(JniEnvGuard {
                        vm,
                        env: env.cast(),
                        attached_current_thread: true,
                    })
                } else {
                    Err(android_keystore_unavailable())
                }
            }
            _ => Err(android_keystore_unavailable()),
        }
    }
}

unsafe fn call_bridge_method(
    env: JNIEnv,
    operation: AndroidKeystoreBridgeOperation,
    signing_key_id: &str,
    alias: &str,
    canonical_bytes: &[u8],
) -> Result<jobject, CryptoError> {
    let method = android_keystore_jni_method_spec(operation);
    let mut class = ptr::null_mut();
    let mut signing_key_id_ref = ptr::null_mut();
    let mut alias_ref = ptr::null_mut();
    let mut canonical_ref = ptr::null_mut();

    let result = (|| -> Result<jobject, CryptoError> {
        class = find_class(env, ANDROID_KEYSTORE_JNI_BRIDGE_CLASS)?;
        let method_id = get_static_method_id(env, class, method.name, method.descriptor)?;
        signing_key_id_ref = new_string_utf(env, "signing_key_id", signing_key_id)?;
        alias_ref = new_string_utf(env, "android_keystore_alias", alias)?;

        if operation == AndroidKeystoreBridgeOperation::Sign {
            canonical_ref = new_byte_array(env, canonical_bytes)?;
            let args = [
                JniValue {
                    i: jint::from(ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION),
                },
                JniValue {
                    l: signing_key_id_ref,
                },
                JniValue { l: alias_ref },
                JniValue { l: canonical_ref },
            ];
            call_static_object_method_a(env, class, method_id, args.as_ptr())
        } else {
            let args = [
                JniValue {
                    i: jint::from(ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION),
                },
                JniValue {
                    l: signing_key_id_ref,
                },
                JniValue { l: alias_ref },
            ];
            call_static_object_method_a(env, class, method_id, args.as_ptr())
        }
    })();

    delete_local_ref(env, canonical_ref);
    delete_local_ref(env, alias_ref);
    delete_local_ref(env, signing_key_id_ref);
    delete_local_ref(env, class);

    let result = result?;
    if result.is_null() {
        Err(android_keystore_unavailable())
    } else {
        Ok(result)
    }
}

unsafe fn read_result_error(
    env: JNIEnv,
    result: jobject,
    signing_key_id: &str,
) -> Result<Option<CryptoError>, CryptoError> {
    let error_code_ref = call_result_object_method(
        env,
        result,
        ANDROID_KEYSTORE_JNI_GET_ERROR_CODE_METHOD,
        ANDROID_KEYSTORE_JNI_ERROR_CODE_METHOD_DESCRIPTOR,
    )?;
    if error_code_ref.is_null() {
        return Ok(None);
    }

    let error_code = java_string(env, error_code_ref);
    delete_local_ref(env, error_code_ref);
    let error_code = error_code?;
    let error = AndroidKeystoreBridgeErrorCode::parse(&error_code)
        .map(|code| code.to_crypto_error(signing_key_id))
        .unwrap_or_else(|_| CryptoError::PrivateKeyCorrupted {
            key_id: signing_key_id.to_owned(),
        });
    Ok(Some(error))
}

unsafe fn read_result_bytes(
    env: JNIEnv,
    result: jobject,
    signing_key_id: &str,
    getter_name: &str,
) -> Result<Vec<u8>, CryptoError> {
    if let Some(error) = read_result_error(env, result, signing_key_id)? {
        return Err(error);
    }

    let byte_array = call_result_object_method(
        env,
        result,
        getter_name,
        ANDROID_KEYSTORE_JNI_BYTE_ARRAY_METHOD_DESCRIPTOR,
    )?;
    if byte_array.is_null() {
        return Err(CryptoError::PrivateKeyCorrupted {
            key_id: signing_key_id.to_owned(),
        });
    }

    let bytes = java_byte_array(env, byte_array).map_err(|_| CryptoError::PrivateKeyCorrupted {
        key_id: signing_key_id.to_owned(),
    });
    delete_local_ref(env, byte_array);
    bytes
}

unsafe fn call_result_object_method(
    env: JNIEnv,
    result: jobject,
    method_name: &str,
    descriptor: &str,
) -> Result<jobject, CryptoError> {
    let class = get_object_class(env, result)?;
    let output = (|| -> Result<jobject, CryptoError> {
        let method_id = get_method_id(env, class, method_name, descriptor)?;
        call_object_method_a(env, result, method_id, ptr::null())
    })();
    delete_local_ref(env, class);
    output
}

unsafe fn find_class(env: JNIEnv, class_name: &str) -> Result<jclass, CryptoError> {
    let find_class = jni_env_fn::<FindClass>(env, JNI_ENV_FIND_CLASS)?;
    let class_name = c_string("android_keystore_jni_class", class_name)?;
    let class = find_class(env, class_name.as_ptr());
    if exception_pending_then_clear(env)? || class.is_null() {
        return Err(android_keystore_unavailable());
    }
    Ok(class)
}

unsafe fn get_object_class(env: JNIEnv, object: jobject) -> Result<jclass, CryptoError> {
    let get_object_class = jni_env_fn::<GetObjectClass>(env, JNI_ENV_GET_OBJECT_CLASS)?;
    let class = get_object_class(env, object);
    if exception_pending_then_clear(env)? || class.is_null() {
        return Err(android_keystore_unavailable());
    }
    Ok(class)
}

unsafe fn get_method_id(
    env: JNIEnv,
    class: jclass,
    method_name: &str,
    descriptor: &str,
) -> Result<jmethodID, CryptoError> {
    let get_method_id = jni_env_fn::<GetMethodId>(env, JNI_ENV_GET_METHOD_ID)?;
    let method_name = c_string("android_keystore_jni_method", method_name)?;
    let descriptor = c_string("android_keystore_jni_method_descriptor", descriptor)?;
    let method_id = get_method_id(env, class, method_name.as_ptr(), descriptor.as_ptr());
    if exception_pending_then_clear(env)? || method_id.is_null() {
        return Err(android_keystore_unavailable());
    }
    Ok(method_id)
}

unsafe fn get_static_method_id(
    env: JNIEnv,
    class: jclass,
    method_name: &str,
    descriptor: &str,
) -> Result<jmethodID, CryptoError> {
    let get_static_method_id = jni_env_fn::<GetStaticMethodId>(env, JNI_ENV_GET_STATIC_METHOD_ID)?;
    let method_name = c_string("android_keystore_jni_method", method_name)?;
    let descriptor = c_string("android_keystore_jni_method_descriptor", descriptor)?;
    let method_id = get_static_method_id(env, class, method_name.as_ptr(), descriptor.as_ptr());
    if exception_pending_then_clear(env)? || method_id.is_null() {
        return Err(android_keystore_unavailable());
    }
    Ok(method_id)
}

unsafe fn call_static_object_method_a(
    env: JNIEnv,
    class: jclass,
    method_id: jmethodID,
    args: *const JniValue,
) -> Result<jobject, CryptoError> {
    let call_static_object_method_a =
        jni_env_fn::<CallStaticObjectMethodA>(env, JNI_ENV_CALL_STATIC_OBJECT_METHOD_A)?;
    let result = call_static_object_method_a(env, class, method_id, args);
    if exception_pending_then_clear(env)? {
        return Err(android_keystore_unavailable());
    }
    Ok(result)
}

unsafe fn call_object_method_a(
    env: JNIEnv,
    object: jobject,
    method_id: jmethodID,
    args: *const JniValue,
) -> Result<jobject, CryptoError> {
    let call_object_method_a = jni_env_fn::<CallObjectMethodA>(env, JNI_ENV_CALL_OBJECT_METHOD_A)?;
    let result = call_object_method_a(env, object, method_id, args);
    if exception_pending_then_clear(env)? {
        return Err(android_keystore_unavailable());
    }
    Ok(result)
}

unsafe fn new_string_utf(
    env: JNIEnv,
    field: &'static str,
    value: &str,
) -> Result<jstring, CryptoError> {
    let new_string_utf = jni_env_fn::<NewStringUtf>(env, JNI_ENV_NEW_STRING_UTF)?;
    let value = c_string(field, value)?;
    let java_string = new_string_utf(env, value.as_ptr());
    if exception_pending_then_clear(env)? || java_string.is_null() {
        return Err(android_keystore_unavailable());
    }
    Ok(java_string)
}

unsafe fn new_byte_array(env: JNIEnv, value: &[u8]) -> Result<jbyteArray, CryptoError> {
    let len = jsize::try_from(value.len()).map_err(|_| {
        CryptoError::invalid_field("canonical_bytes_len", "value does not fit JNI jsize")
    })?;
    let new_byte_array = jni_env_fn::<NewByteArray>(env, JNI_ENV_NEW_BYTE_ARRAY)?;
    let array = new_byte_array(env, len);
    if exception_pending_then_clear(env)? || array.is_null() {
        return Err(android_keystore_unavailable());
    }
    let set_byte_array_region =
        jni_env_fn::<SetByteArrayRegion>(env, JNI_ENV_SET_BYTE_ARRAY_REGION)?;
    set_byte_array_region(env, array, 0, len, value.as_ptr().cast::<jbyte>());
    if exception_pending_then_clear(env)? {
        delete_local_ref(env, array);
        return Err(android_keystore_unavailable());
    }
    Ok(array)
}

unsafe fn java_string(env: JNIEnv, value: jstring) -> Result<String, CryptoError> {
    let get_string_utf_chars = jni_env_fn::<GetStringUtfChars>(env, JNI_ENV_GET_STRING_UTF_CHARS)?;
    let release_string_utf_chars =
        jni_env_fn::<ReleaseStringUtfChars>(env, JNI_ENV_RELEASE_STRING_UTF_CHARS)?;
    let chars = get_string_utf_chars(env, value, ptr::null_mut());
    if exception_pending_then_clear(env)? || chars.is_null() {
        return Err(android_keystore_unavailable());
    }
    let parsed = CStr::from_ptr(chars)
        .to_str()
        .map(str::to_owned)
        .map_err(|_| android_keystore_unavailable());
    release_string_utf_chars(env, value, chars);
    parsed
}

unsafe fn java_byte_array(env: JNIEnv, value: jbyteArray) -> Result<Vec<u8>, CryptoError> {
    let get_array_length = jni_env_fn::<GetArrayLength>(env, JNI_ENV_GET_ARRAY_LENGTH)?;
    let len = get_array_length(env, value);
    if exception_pending_then_clear(env)? || len < 0 {
        return Err(android_keystore_unavailable());
    }
    let len_usize = usize::try_from(len)
        .map_err(|_| CryptoError::invalid_field("jni_byte_array_len", "negative length"))?;
    let mut bytes = vec![0u8; len_usize];
    let get_byte_array_region =
        jni_env_fn::<GetByteArrayRegion>(env, JNI_ENV_GET_BYTE_ARRAY_REGION)?;
    get_byte_array_region(env, value, 0, len, bytes.as_mut_ptr().cast::<jbyte>());
    if exception_pending_then_clear(env)? {
        return Err(android_keystore_unavailable());
    }
    Ok(bytes)
}

unsafe fn delete_local_ref(env: JNIEnv, object: jobject) {
    if object.is_null() {
        return;
    }
    if let Ok(delete_local_ref) = jni_env_fn::<DeleteLocalRef>(env, JNI_ENV_DELETE_LOCAL_REF) {
        delete_local_ref(env, object);
    }
}

unsafe fn exception_pending_then_clear(env: JNIEnv) -> Result<bool, CryptoError> {
    let exception_check = jni_env_fn::<ExceptionCheck>(env, JNI_ENV_EXCEPTION_CHECK)?;
    if exception_check(env) == 0 {
        return Ok(false);
    }
    let exception_clear = jni_env_fn::<ExceptionClear>(env, JNI_ENV_EXCEPTION_CLEAR)?;
    exception_clear(env);
    Ok(true)
}

fn c_string(field: &'static str, value: &str) -> Result<CString, CryptoError> {
    CString::new(value).map_err(|_| {
        CryptoError::invalid_field(field, "value must not contain an interior NUL byte")
    })
}

unsafe fn java_vm_fn<F: Copy>(vm: JavaVM, index: usize) -> Result<F, CryptoError> {
    jni_table_fn(vm, index)
}

unsafe fn jni_env_fn<F: Copy>(env: JNIEnv, index: usize) -> Result<F, CryptoError> {
    jni_table_fn(env, index)
}

unsafe fn jni_table_fn<F: Copy>(owner: *mut c_void, index: usize) -> Result<F, CryptoError> {
    if owner.is_null() {
        return Err(android_keystore_unavailable());
    }
    let table = *(owner as *mut *const *const c_void);
    if table.is_null() {
        return Err(android_keystore_unavailable());
    }
    let raw = *table.add(index);
    if raw.is_null() {
        return Err(android_keystore_unavailable());
    }
    Ok(std::mem::transmute_copy::<*const c_void, F>(&raw))
}

#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut c_void) -> jint {
    match register_java_vm(vm) {
        Ok(()) => JNI_VERSION_1_6,
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_vm_registration_rejects_null_pointer() {
        let error = register_java_vm(ptr::null_mut()).expect_err("null JavaVM rejected");
        assert!(matches!(
            error,
            CryptoError::StorageBackendUnavailable { .. }
                | CryptoError::UnsupportedStorageBackend { .. }
        ));
    }
}
