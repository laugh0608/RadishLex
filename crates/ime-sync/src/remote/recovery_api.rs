use radishlex_ime_crypto::{
    AlgorithmId, DeviceSignature, Nonce, RecoveryKdfProfile, RecoveryMaterial,
    SignatureAlgorithmId, SignedRecoveryRecordManifest,
};
use serde::{Deserialize, Serialize};

use super::*;
use crate::product_provider::SyncTrustedDomainState;

#[derive(Clone, PartialEq, Eq)]
pub struct RemoteVerifiedRecoveryRecord {
    pub material: RecoveryMaterial,
    pub manifest: SignedRecoveryRecordManifest,
}

impl fmt::Debug for RemoteVerifiedRecoveryRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteVerifiedRecoveryRecord")
            .field("domain_id", &self.material.domain_id)
            .field("recovery_id", &self.material.recovery_id)
            .field("key_epoch", &self.material.key_epoch)
            .field(
                "signer_device_id",
                &self.manifest.signature.signer_device_id,
            )
            .field("wrapped_material", &"[redacted]")
            .finish()
    }
}

impl<T: SyncRemoteTransport> SyncRemoteClient<T> {
    pub fn upload_recovery_record(
        &self,
        trusted_domain: &SyncTrustedDomainState,
        material: &RecoveryMaterial,
        manifest: &SignedRecoveryRecordManifest,
    ) -> Result<(), SyncRemoteError> {
        verify_recovery_record(trusted_domain, material, manifest, true)?;
        let path = recovery_records_path(&self.api_prefix, &material.domain_id)?;
        let response: RecoveryUploadResponseDto = self.send_json(
            SyncRemoteMethod::Post,
            path,
            &RecoveryRecordUploadDto::new(material, manifest),
        )?;
        if response.domain_id != material.domain_id
            || response.recovery_record_id != material.recovery_id
        {
            return invalid_response("recovery upload response does not match request");
        }
        Ok(())
    }

    pub fn latest_verified_recovery_record(
        &self,
        trusted_domain: &SyncTrustedDomainState,
        domain_id: &str,
    ) -> Result<RemoteVerifiedRecoveryRecord, SyncRemoteError> {
        validate_path_segment("domain_id", domain_id)?;
        if trusted_domain.domain().domain_id != domain_id {
            return invalid_request("recovery domain does not match trusted lifecycle");
        }
        let path = format!(
            "{}/latest",
            recovery_records_path(&self.api_prefix, domain_id)?
        );
        let dto: RecoveryRecordResponseDto = self.send_empty(SyncRemoteMethod::Get, path)?;
        if dto.domain_id != domain_id || dto.status != "active" {
            return invalid_response("latest recovery response locator or status is invalid");
        }
        let ciphertext_hash = dto.ciphertext_hash.clone();
        let wrapped_material_len = dto.wrapped_material_len;
        let signature_schema_version = dto.signature_schema_version;
        let signature_algorithm = dto.signature_algorithm.clone();
        let signature_key_id = dto.signature_key_id.clone();
        let signer_device_id = dto.signer_device_id.clone();
        let signature_bytes = dto.signature.clone();
        let material = RecoveryMaterial::new(
            dto.record_schema_version,
            dto.recovery_record_id,
            dto.previous_recovery_id,
            dto.domain_id,
            dto.key_epoch,
            dto.kdf_profile,
            dto.kdf_version,
            dto.salt,
            dto.memory_kib,
            dto.iterations,
            dto.parallelism,
            dto.output_len,
            AlgorithmId::new(dto.algorithm).map_err(invalid_crypto_response)?,
            Nonce::new(dto.nonce).map_err(invalid_crypto_response)?,
            dto.wrapped_material,
            dto.activation_algorithm,
            dto.activation_public_key_id,
            dto.activation_public_key,
            dto.created_at_ms,
            dto.updated_at_ms,
        )
        .map_err(invalid_crypto_response)?;
        RecoveryKdfProfile::from_recovery_material(&material)
            .and_then(|profile| profile.validate())
            .map_err(invalid_crypto_response)?;
        if material.encrypted_recovery_key.len() != wrapped_material_len {
            return invalid_response("recovery wrapped material length does not match metadata");
        }
        if signature_schema_version != 1 {
            return invalid_response("recovery signature schema is unsupported");
        }
        let signature = DeviceSignature::new_for_algorithm(
            SignatureAlgorithmId::new(signature_algorithm).map_err(invalid_crypto_response)?,
            signature_key_id,
            signer_device_id,
            signature_bytes,
        )
        .map_err(invalid_crypto_response)?;
        let manifest = SignedRecoveryRecordManifest::new(&material, signature)
            .map_err(invalid_crypto_response)?;
        if manifest.ciphertext_hash != ciphertext_hash {
            return invalid_response("recovery ciphertext hash does not match wrapped material");
        }
        verify_recovery_record(trusted_domain, &material, &manifest, false)?;
        Ok(RemoteVerifiedRecoveryRecord { material, manifest })
    }
}

fn verify_recovery_record(
    trusted_domain: &SyncTrustedDomainState,
    material: &RecoveryMaterial,
    manifest: &SignedRecoveryRecordManifest,
    request_side: bool,
) -> Result<(), SyncRemoteError> {
    RecoveryKdfProfile::from_recovery_material(material)
        .and_then(|profile| profile.validate())
        .map_err(|error| recovery_validation_error(error, request_side))?;
    let derived = SignedRecoveryRecordManifest::new(material, manifest.signature.clone())
        .map_err(|error| recovery_validation_error(error, request_side))?;
    if &derived != manifest {
        return recovery_error("recovery manifest does not match material", request_side);
    }
    if trusted_domain.domain().domain_id != material.domain_id
        || trusted_domain.domain().current_key_epoch != material.key_epoch
    {
        return recovery_error(
            "recovery record does not match trusted domain current epoch",
            request_side,
        );
    }
    let signer = trusted_domain
        .device_profile(&manifest.signature.signer_device_id)
        .ok_or_else(|| recovery_error_value("recovery signer is not trusted", request_side))?;
    if signer.device().status != crate::SyncDeviceStatus::Active {
        return recovery_error("recovery signer is not active", request_side);
    }
    manifest
        .verify(signer.signing_public_key())
        .map_err(|error| recovery_validation_error(error, request_side))
}

fn recovery_validation_error(
    error: radishlex_ime_crypto::CryptoError,
    request_side: bool,
) -> SyncRemoteError {
    recovery_error_value(error.to_string(), request_side)
}

fn recovery_error<T>(message: impl Into<String>, request_side: bool) -> Result<T, SyncRemoteError> {
    Err(recovery_error_value(message, request_side))
}

fn recovery_error_value(message: impl Into<String>, request_side: bool) -> SyncRemoteError {
    if request_side {
        SyncRemoteError::InvalidRequest {
            message: message.into(),
        }
    } else {
        SyncRemoteError::InvalidResponse {
            message: message.into(),
        }
    }
}

fn invalid_crypto_response(error: radishlex_ime_crypto::CryptoError) -> SyncRemoteError {
    SyncRemoteError::InvalidResponse {
        message: error.to_string(),
    }
}

fn recovery_records_path(api_prefix: &str, domain_id: &str) -> Result<String, SyncRemoteError> {
    validate_path_segment("domain_id", domain_id)?;
    Ok(format!("{api_prefix}/domains/{domain_id}/recovery-records"))
}

#[derive(Debug, Serialize)]
struct RecoveryRecordUploadDto<'a> {
    record_schema_version: u16,
    recovery_record_id: &'a str,
    previous_recovery_id: &'a str,
    key_epoch: u64,
    kdf_profile: &'a str,
    kdf_version: u16,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    output_len: usize,
    #[serde(with = "base64_bytes")]
    salt: &'a [u8],
    algorithm: &'a str,
    #[serde(with = "base64_bytes")]
    nonce: &'a [u8],
    wrapped_material_len: usize,
    ciphertext_hash: &'a str,
    activation_algorithm: &'a str,
    activation_public_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    activation_public_key: &'a [u8],
    status: &'static str,
    created_at_ms: i64,
    updated_at_ms: i64,
    signer_device_id: &'a str,
    signature_schema_version: u16,
    signature_algorithm: &'a str,
    signature_key_id: &'a str,
    #[serde(with = "base64_bytes")]
    signature: &'a [u8],
    #[serde(with = "base64_bytes")]
    wrapped_material: &'a [u8],
}

impl<'a> RecoveryRecordUploadDto<'a> {
    fn new(material: &'a RecoveryMaterial, manifest: &'a SignedRecoveryRecordManifest) -> Self {
        Self {
            record_schema_version: material.record_schema_version,
            recovery_record_id: &material.recovery_id,
            previous_recovery_id: &material.previous_recovery_id,
            key_epoch: material.key_epoch,
            kdf_profile: &material.kdf_id,
            kdf_version: material.kdf_version,
            memory_kib: material.memory_kib,
            iterations: material.iterations,
            parallelism: material.parallelism,
            output_len: material.output_len,
            salt: &material.salt,
            algorithm: material.envelope_algorithm.as_str(),
            nonce: material.envelope_nonce.as_bytes(),
            wrapped_material_len: material.encrypted_recovery_key.len(),
            ciphertext_hash: &manifest.ciphertext_hash,
            activation_algorithm: &material.activation_algorithm,
            activation_public_key_id: &material.activation_public_key_id,
            activation_public_key: &material.activation_public_key,
            status: "active",
            created_at_ms: material.created_at_ms,
            updated_at_ms: material.updated_at_ms,
            signer_device_id: &manifest.signature.signer_device_id,
            signature_schema_version: manifest.signature.signature_schema_version,
            signature_algorithm: manifest.signature.signature_algorithm.as_str(),
            signature_key_id: &manifest.signature.signature_key_id,
            signature: &manifest.signature.signature,
            wrapped_material: &material.encrypted_recovery_key,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RecoveryUploadResponseDto {
    domain_id: String,
    recovery_record_id: String,
}

#[derive(Debug, Deserialize)]
struct RecoveryRecordResponseDto {
    record_schema_version: u16,
    domain_id: String,
    recovery_record_id: String,
    previous_recovery_id: String,
    key_epoch: u64,
    kdf_profile: String,
    kdf_version: u16,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    output_len: usize,
    #[serde(with = "base64_bytes")]
    salt: Vec<u8>,
    algorithm: String,
    #[serde(with = "base64_bytes")]
    nonce: Vec<u8>,
    wrapped_material_len: usize,
    ciphertext_hash: String,
    activation_algorithm: String,
    activation_public_key_id: String,
    #[serde(with = "base64_bytes")]
    activation_public_key: Vec<u8>,
    status: String,
    created_at_ms: i64,
    updated_at_ms: i64,
    signer_device_id: String,
    signature_schema_version: u16,
    signature_algorithm: String,
    signature_key_id: String,
    #[serde(with = "base64_bytes")]
    signature: Vec<u8>,
    #[serde(with = "base64_bytes")]
    wrapped_material: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use base64ct::{Base64, Encoding};
    use radishlex_ime_crypto::{TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN};

    use super::*;
    use crate::{SyncDevice, SyncDomain, SyncTrustedDeviceProfile};

    struct RecordingTransport {
        response: RefCell<Option<SyncRemoteResponse>>,
        request: RefCell<Option<SyncRemoteRequest>>,
    }

    impl RecordingTransport {
        fn json(value: serde_json::Value) -> Self {
            Self {
                response: RefCell::new(Some(
                    SyncRemoteResponse::json(200, &value).expect("response"),
                )),
                request: RefCell::new(None),
            }
        }
    }

    impl SyncRemoteTransport for RecordingTransport {
        fn send(&self, request: SyncRemoteRequest) -> Result<SyncRemoteResponse, SyncRemoteError> {
            self.request.replace(Some(request));
            Ok(self.response.borrow_mut().take().expect("response queued"))
        }
    }

    #[test]
    fn latest_recovery_record_verifies_ciphertext_signature_and_current_epoch() {
        let (domain, material, manifest) = signed_recovery_fixture();
        let transport = RecordingTransport::json(recovery_response(&material, &manifest));
        let client = SyncRemoteClient::new(transport);

        let verified = client
            .latest_verified_recovery_record(&domain, "domain-a")
            .expect("verified recovery");

        assert_eq!(verified.material, material);
        assert_eq!(verified.manifest, manifest);
        let debug = format!("{verified:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains(&Base64::encode_string(&[42u8; 48])));
    }

    #[test]
    fn latest_recovery_record_rejects_ciphertext_hash_tampering() {
        let (domain, material, manifest) = signed_recovery_fixture();
        let mut response = recovery_response(&material, &manifest);
        response["ciphertext_hash"] = serde_json::json!("sha256:tampered");
        let client = SyncRemoteClient::new(RecordingTransport::json(response));

        let error = client
            .latest_verified_recovery_record(&domain, "domain-a")
            .expect_err("tampered hash fails");

        assert!(matches!(error, SyncRemoteError::InvalidResponse { .. }));
    }

    #[test]
    fn recovery_upload_rejects_stale_epoch_before_network() {
        let (domain, mut material, manifest) = signed_recovery_fixture();
        material.key_epoch = 2;
        let client = SyncRemoteClient::new(RecordingTransport::json(serde_json::json!({})));

        let error = client
            .upload_recovery_record(&domain, &material, &manifest)
            .expect_err("stale manifest fails");

        assert!(matches!(error, SyncRemoteError::InvalidRequest { .. }));
        assert!(client.transport().request.borrow().is_none());
    }

    fn signed_recovery_fixture() -> (
        SyncTrustedDomainState,
        RecoveryMaterial,
        SignedRecoveryRecordManifest,
    ) {
        let mut store = TestMemoryDeviceKeyStore::new();
        let public = store
            .insert_signing_key("device-a", "signing-key-a", [8; 32], 100)
            .expect("signing key");
        let handle = store.handle("device-a", "signing-key-a").expect("handle");
        let profile = SyncTrustedDeviceProfile::active(
            SyncDevice::pending("device-a", "signing-key-a", 99)
                .expect("pending")
                .activate(100)
                .expect("active"),
            public,
        )
        .expect("profile");
        let domain = SyncTrustedDomainState::new(
            SyncDomain::new("domain-a", 1, "object-key-v1", 100, 200).expect("domain"),
            [profile],
        )
        .expect("trusted domain");
        let material = RecoveryMaterial::new(
            2,
            "recovery-a",
            "",
            "domain-a",
            1,
            "argon2id-v1",
            1,
            vec![3; 16],
            65_536,
            3,
            4,
            32,
            AlgorithmId::xchacha20poly1305_hkdf_sha256(),
            Nonce::new(vec![4; 24]).expect("nonce"),
            vec![42; 48],
            "ed25519-v1",
            "recovery-activation-sha256:test",
            vec![5; 32],
            150,
            150,
        )
        .expect("material");
        let placeholder =
            DeviceSignature::new("signing-key-a", "device-a", vec![1; ED25519_SIGNATURE_LEN])
                .expect("placeholder");
        let unsigned = SignedRecoveryRecordManifest::new(&material, placeholder)
            .expect("unsigned manifest shape");
        let signature = store
            .sign(&handle, &unsigned.canonical_bytes())
            .expect("signature");
        let manifest = SignedRecoveryRecordManifest::new(&material, signature).expect("manifest");
        (domain, material, manifest)
    }

    fn recovery_response(
        material: &RecoveryMaterial,
        manifest: &SignedRecoveryRecordManifest,
    ) -> serde_json::Value {
        serde_json::json!({
            "record_schema_version": material.record_schema_version,
            "domain_id": material.domain_id,
            "recovery_record_id": material.recovery_id,
            "previous_recovery_id": material.previous_recovery_id,
            "key_epoch": material.key_epoch,
            "kdf_profile": material.kdf_id,
            "kdf_version": material.kdf_version,
            "memory_kib": material.memory_kib,
            "iterations": material.iterations,
            "parallelism": material.parallelism,
            "output_len": material.output_len,
            "salt": Base64::encode_string(&material.salt),
            "algorithm": material.envelope_algorithm.as_str(),
            "nonce": Base64::encode_string(material.envelope_nonce.as_bytes()),
            "wrapped_material_len": material.encrypted_recovery_key.len(),
            "ciphertext_hash": manifest.ciphertext_hash,
            "activation_algorithm": material.activation_algorithm,
            "activation_public_key_id": material.activation_public_key_id,
            "activation_public_key": Base64::encode_string(&material.activation_public_key),
            "status": "active",
            "created_at_ms": material.created_at_ms,
            "updated_at_ms": material.updated_at_ms,
            "signer_device_id": manifest.signature.signer_device_id,
            "signature_schema_version": manifest.signature.signature_schema_version,
            "signature_algorithm": manifest.signature.signature_algorithm.as_str(),
            "signature_key_id": manifest.signature.signature_key_id,
            "signature": Base64::encode_string(&manifest.signature.signature),
            "wrapped_material": Base64::encode_string(&material.encrypted_recovery_key),
        })
    }
}
