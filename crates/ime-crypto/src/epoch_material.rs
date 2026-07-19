use std::fmt;

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use p256::{
    ecdh::EphemeralSecret,
    elliptic_curve::{rand_core::OsRng, sec1::ToEncodedPoint},
    PublicKey,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::model::{push_aad_field, validate_required};
use crate::{CryptoError, KeyDescriptor, KeyRole, Nonce, SyncMasterKeyMaterial};

pub const WRAPPED_EPOCH_SCHEMA_VERSION: u16 = 1;
pub const KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1: &str = "p256-ecdh-v1";
pub const WRAPPED_EPOCH_ALGORITHM_P256_ECDH_HKDF_SHA256_XCHACHA20POLY1305_V1: &str =
    "p256-ecdh-hkdf-sha256-xchacha20poly1305-v1";
pub const P256_KEY_AGREEMENT_PUBLIC_KEY_LEN: usize = 65;

const ENVELOPE_MAGIC: &[u8; 4] = b"RLXE";
const ENVELOPE_HEADER_LEN: usize = 8;
const SYNC_MASTER_KEY_LEN: usize = 32;
const AEAD_TAG_LEN: usize = 16;
const MAX_KEY_ID_LEN: usize = 512;

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceKeyAgreementPublicKey {
    pub device_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub public_key: Vec<u8>,
    pub created_at_ms: i64,
    pub revoked_at_ms: Option<i64>,
}

impl DeviceKeyAgreementPublicKey {
    pub fn p256(
        device_id: impl Into<String>,
        key_id: impl Into<String>,
        public_key: impl Into<Vec<u8>>,
        created_at_ms: i64,
        revoked_at_ms: Option<i64>,
    ) -> Result<Self, CryptoError> {
        let value = Self {
            device_id: device_id.into(),
            key_id: key_id.into(),
            algorithm: KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1.to_owned(),
            public_key: public_key.into(),
            created_at_ms,
            revoked_at_ms,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("key_agreement_device_id", &self.device_id)?;
        validate_required("key_agreement_key_id", &self.key_id)?;
        if self.algorithm != KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1 {
            return Err(CryptoError::invalid_field(
                "key_agreement_algorithm",
                "unsupported key-agreement algorithm",
            ));
        }
        validate_p256_public_key(&self.public_key)?;
        if self
            .revoked_at_ms
            .is_some_and(|revoked_at_ms| revoked_at_ms < self.created_at_ms)
        {
            return Err(CryptoError::invalid_field(
                "key_agreement_revoked_at_ms",
                "value must not precede created_at_ms",
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for DeviceKeyAgreementPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceKeyAgreementPublicKey")
            .field("device_id", &self.device_id)
            .field("key_id", &self.key_id)
            .field("algorithm", &self.algorithm)
            .field("public_key_len", &self.public_key.len())
            .field("created_at_ms", &self.created_at_ms)
            .field("revoked_at_ms", &self.revoked_at_ms)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyAgreementKeyHandle {
    pub device_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub storage_backend: String,
}

impl DeviceKeyAgreementKeyHandle {
    pub fn p256(
        device_id: impl Into<String>,
        key_id: impl Into<String>,
        storage_backend: impl Into<String>,
    ) -> Result<Self, CryptoError> {
        let value = Self {
            device_id: device_id.into(),
            key_id: key_id.into(),
            algorithm: KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1.to_owned(),
            storage_backend: storage_backend.into(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        validate_required("key_agreement_device_id", &self.device_id)?;
        validate_required("key_agreement_key_id", &self.key_id)?;
        validate_required("key_agreement_storage_backend", &self.storage_backend)?;
        if self.algorithm != KEY_AGREEMENT_ALGORITHM_P256_ECDH_V1 {
            return Err(CryptoError::invalid_field(
                "key_agreement_algorithm",
                "unsupported key-agreement algorithm",
            ));
        }
        Ok(())
    }
}

pub struct EcdhSharedSecret([u8; SYNC_MASTER_KEY_LEN]);

impl EcdhSharedSecret {
    pub fn new(bytes: [u8; SYNC_MASTER_KEY_LEN]) -> Result<Self, CryptoError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(CryptoError::KeyDerivationFailed);
        }
        Ok(Self(bytes))
    }

    fn as_bytes(&self) -> &[u8; SYNC_MASTER_KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for EcdhSharedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EcdhSharedSecret([redacted])")
    }
}

impl Drop for EcdhSharedSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct WrappedEpochMaterial {
    pub schema_version: u16,
    pub algorithm: String,
    pub domain_id: String,
    pub recipient_device_id: String,
    pub recipient_key_agreement_key_id: String,
    pub wrapping_key_id: String,
    pub key_epoch: u64,
    pub nonce: Nonce,
    pub wrapped_key: Vec<u8>,
    pub ciphertext_hash: String,
    pub created_at_ms: i64,
}

impl WrappedEpochMaterial {
    #[allow(clippy::too_many_arguments)]
    pub fn seal_for_recipient(
        domain_id: impl Into<String>,
        recipient: &DeviceKeyAgreementPublicKey,
        wrapping_key_id: impl Into<String>,
        object_key: &KeyDescriptor,
        sync_master_key: &SyncMasterKeyMaterial,
        nonce: Nonce,
        created_at_ms: i64,
    ) -> Result<Self, CryptoError> {
        recipient.validate()?;
        validate_object_key(object_key)?;
        let ephemeral_secret = EphemeralSecret::random(&mut OsRng);
        let ephemeral_public = ephemeral_secret.public_key();
        let shared = ephemeral_secret.diffie_hellman(
            &PublicKey::from_sec1_bytes(&recipient.public_key).map_err(|_| invalid_public_key())?,
        );
        let shared_secret = EcdhSharedSecret::new((*shared.raw_secret_bytes()).into())?;
        let ephemeral_bytes = ephemeral_public.to_encoded_point(false);
        let mut record = Self {
            schema_version: WRAPPED_EPOCH_SCHEMA_VERSION,
            algorithm: WRAPPED_EPOCH_ALGORITHM_P256_ECDH_HKDF_SHA256_XCHACHA20POLY1305_V1
                .to_owned(),
            domain_id: domain_id.into(),
            recipient_device_id: recipient.device_id.clone(),
            recipient_key_agreement_key_id: recipient.key_id.clone(),
            wrapping_key_id: wrapping_key_id.into(),
            key_epoch: object_key.key_epoch,
            nonce,
            wrapped_key: Vec::new(),
            ciphertext_hash: String::new(),
            created_at_ms,
        };
        record.validate_metadata()?;
        let plaintext = encode_plaintext(object_key, sync_master_key)?;
        let ciphertext = encrypt(
            &shared_secret,
            &record.associated_data(ephemeral_bytes.as_bytes()),
            &record.nonce,
            &plaintext,
        )?;
        record.wrapped_key = encode_envelope(ephemeral_bytes.as_bytes(), &ciphertext)?;
        record.ciphertext_hash = ciphertext_hash(&record.wrapped_key);
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        self.validate_metadata()?;
        let (_, ciphertext) = decode_envelope(&self.wrapped_key)?;
        if ciphertext.len() <= AEAD_TAG_LEN {
            return Err(CryptoError::invalid_field(
                "wrapped_key",
                "ciphertext is too short",
            ));
        }
        if self.ciphertext_hash != ciphertext_hash(&self.wrapped_key) {
            return Err(CryptoError::CiphertextHashMismatch);
        }
        Ok(())
    }

    pub fn ephemeral_public_key(&self) -> Result<&[u8], CryptoError> {
        decode_envelope(&self.wrapped_key).map(|(public_key, _)| public_key)
    }

    pub fn unwrap(
        &self,
        shared_secret: &EcdhSharedSecret,
    ) -> Result<(KeyDescriptor, SyncMasterKeyMaterial), CryptoError> {
        self.validate()?;
        let (ephemeral_public_key, ciphertext) = decode_envelope(&self.wrapped_key)?;
        let plaintext = decrypt(
            shared_secret,
            &self.associated_data(ephemeral_public_key),
            &self.nonce,
            ciphertext,
        )?;
        decode_plaintext(&plaintext, self.key_epoch)
    }

    fn validate_metadata(&self) -> Result<(), CryptoError> {
        if self.schema_version != WRAPPED_EPOCH_SCHEMA_VERSION {
            return Err(CryptoError::invalid_field(
                "wrapped_epoch_schema_version",
                "unsupported schema version",
            ));
        }
        if self.algorithm != WRAPPED_EPOCH_ALGORITHM_P256_ECDH_HKDF_SHA256_XCHACHA20POLY1305_V1 {
            return Err(CryptoError::invalid_field(
                "wrapped_epoch_algorithm",
                "unsupported algorithm",
            ));
        }
        validate_required("domain_id", &self.domain_id)?;
        validate_required("recipient_device_id", &self.recipient_device_id)?;
        validate_required(
            "recipient_key_agreement_key_id",
            &self.recipient_key_agreement_key_id,
        )?;
        validate_required("wrapping_key_id", &self.wrapping_key_id)?;
        if self.key_epoch == 0 {
            return Err(CryptoError::invalid_field(
                "key_epoch",
                "value must be greater than zero",
            ));
        }
        Ok(())
    }

    fn associated_data(&self, ephemeral_public_key: &[u8]) -> Vec<u8> {
        let mut aad = Vec::new();
        push_aad_field(&mut aad, "purpose", b"radishlex-wrapped-epoch-material-v1");
        push_aad_field(
            &mut aad,
            "schema_version",
            self.schema_version.to_string().as_bytes(),
        );
        push_aad_field(&mut aad, "algorithm", self.algorithm.as_bytes());
        push_aad_field(&mut aad, "domain_id", self.domain_id.as_bytes());
        push_aad_field(
            &mut aad,
            "recipient_device_id",
            self.recipient_device_id.as_bytes(),
        );
        push_aad_field(
            &mut aad,
            "recipient_key_agreement_key_id",
            self.recipient_key_agreement_key_id.as_bytes(),
        );
        push_aad_field(&mut aad, "wrapping_key_id", self.wrapping_key_id.as_bytes());
        push_aad_field(&mut aad, "key_epoch", self.key_epoch.to_string().as_bytes());
        push_aad_field(&mut aad, "nonce", self.nonce.as_bytes());
        push_aad_field(&mut aad, "ephemeral_public_key", ephemeral_public_key);
        push_aad_field(
            &mut aad,
            "created_at_ms",
            self.created_at_ms.to_string().as_bytes(),
        );
        aad
    }
}

impl fmt::Debug for WrappedEpochMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WrappedEpochMaterial")
            .field("schema_version", &self.schema_version)
            .field("algorithm", &self.algorithm)
            .field("domain_id", &self.domain_id)
            .field("recipient_device_id", &self.recipient_device_id)
            .field(
                "recipient_key_agreement_key_id",
                &self.recipient_key_agreement_key_id,
            )
            .field("wrapping_key_id", &self.wrapping_key_id)
            .field("key_epoch", &self.key_epoch)
            .field("wrapped_key_len", &self.wrapped_key.len())
            .field("ciphertext_hash", &self.ciphertext_hash)
            .field("created_at_ms", &self.created_at_ms)
            .finish()
    }
}

fn validate_p256_public_key(public_key: &[u8]) -> Result<(), CryptoError> {
    if public_key.len() != P256_KEY_AGREEMENT_PUBLIC_KEY_LEN || public_key.first() != Some(&4) {
        return Err(invalid_public_key());
    }
    PublicKey::from_sec1_bytes(public_key)
        .map(|_| ())
        .map_err(|_| invalid_public_key())
}

fn invalid_public_key() -> CryptoError {
    CryptoError::invalid_field(
        "key_agreement_public_key",
        "value must be a valid 65-byte uncompressed P-256 public key",
    )
}

fn validate_object_key(object_key: &KeyDescriptor) -> Result<(), CryptoError> {
    object_key.validate()?;
    if object_key.role != KeyRole::ObjectKey {
        return Err(CryptoError::invalid_field(
            "key_role",
            "wrapped epoch material requires an object key descriptor",
        ));
    }
    if object_key.key_id.len() > MAX_KEY_ID_LEN {
        return Err(CryptoError::invalid_field("key_id", "value is too long"));
    }
    Ok(())
}

fn encode_plaintext(
    object_key: &KeyDescriptor,
    sync_master_key: &SyncMasterKeyMaterial,
) -> Result<Vec<u8>, CryptoError> {
    validate_object_key(object_key)?;
    let key_id_len = u16::try_from(object_key.key_id.len())
        .map_err(|_| CryptoError::invalid_field("key_id", "value is too long"))?;
    let mut plaintext = Vec::with_capacity(4 + object_key.key_id.len() + SYNC_MASTER_KEY_LEN);
    plaintext.extend_from_slice(&WRAPPED_EPOCH_SCHEMA_VERSION.to_be_bytes());
    plaintext.extend_from_slice(&key_id_len.to_be_bytes());
    plaintext.extend_from_slice(object_key.key_id.as_bytes());
    plaintext.extend_from_slice(sync_master_key.as_bytes());
    Ok(plaintext)
}

fn decode_plaintext(
    plaintext: &[u8],
    key_epoch: u64,
) -> Result<(KeyDescriptor, SyncMasterKeyMaterial), CryptoError> {
    if plaintext.len() < 4 + SYNC_MASTER_KEY_LEN {
        return Err(CryptoError::DecryptionFailed);
    }
    let version = u16::from_be_bytes([plaintext[0], plaintext[1]]);
    let key_id_len = usize::from(u16::from_be_bytes([plaintext[2], plaintext[3]]));
    let expected_len = 4usize
        .checked_add(key_id_len)
        .and_then(|len| len.checked_add(SYNC_MASTER_KEY_LEN))
        .ok_or(CryptoError::DecryptionFailed)?;
    if version != WRAPPED_EPOCH_SCHEMA_VERSION
        || key_id_len == 0
        || key_id_len > MAX_KEY_ID_LEN
        || plaintext.len() != expected_len
    {
        return Err(CryptoError::DecryptionFailed);
    }
    let key_id = std::str::from_utf8(&plaintext[4..4 + key_id_len])
        .map_err(|_| CryptoError::DecryptionFailed)?;
    let mut master_key = [0u8; SYNC_MASTER_KEY_LEN];
    master_key.copy_from_slice(&plaintext[4 + key_id_len..]);
    let descriptor = KeyDescriptor::new(key_id, KeyRole::ObjectKey, key_epoch)?;
    let material = SyncMasterKeyMaterial::new(master_key)?;
    master_key.zeroize();
    Ok((descriptor, material))
}

fn encode_envelope(ephemeral_public_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    validate_p256_public_key(ephemeral_public_key)?;
    let public_key_len =
        u16::try_from(ephemeral_public_key.len()).map_err(|_| CryptoError::EncryptionFailed)?;
    let mut envelope =
        Vec::with_capacity(ENVELOPE_HEADER_LEN + ephemeral_public_key.len() + ciphertext.len());
    envelope.extend_from_slice(ENVELOPE_MAGIC);
    envelope.extend_from_slice(&WRAPPED_EPOCH_SCHEMA_VERSION.to_be_bytes());
    envelope.extend_from_slice(&public_key_len.to_be_bytes());
    envelope.extend_from_slice(ephemeral_public_key);
    envelope.extend_from_slice(ciphertext);
    Ok(envelope)
}

fn decode_envelope(envelope: &[u8]) -> Result<(&[u8], &[u8]), CryptoError> {
    if envelope.len() < ENVELOPE_HEADER_LEN + P256_KEY_AGREEMENT_PUBLIC_KEY_LEN + AEAD_TAG_LEN
        || envelope.get(..4) != Some(ENVELOPE_MAGIC)
    {
        return Err(CryptoError::invalid_field(
            "wrapped_key",
            "invalid envelope",
        ));
    }
    let version = u16::from_be_bytes([envelope[4], envelope[5]]);
    let public_key_len = usize::from(u16::from_be_bytes([envelope[6], envelope[7]]));
    if version != WRAPPED_EPOCH_SCHEMA_VERSION
        || public_key_len != P256_KEY_AGREEMENT_PUBLIC_KEY_LEN
    {
        return Err(CryptoError::invalid_field(
            "wrapped_key",
            "unsupported envelope",
        ));
    }
    let split = ENVELOPE_HEADER_LEN + public_key_len;
    let public_key = &envelope[ENVELOPE_HEADER_LEN..split];
    validate_p256_public_key(public_key)?;
    Ok((public_key, &envelope[split..]))
}

fn derive_aead_key(shared_secret: &EcdhSharedSecret, aad: &[u8]) -> Result<[u8; 32], CryptoError> {
    let hkdf = Hkdf::<Sha256>::new(
        Some(b"radishlex-wrapped-epoch-ecdh-v1"),
        shared_secret.as_bytes(),
    );
    let mut key = [0u8; 32];
    hkdf.expand(aad, &mut key)
        .map_err(|_| CryptoError::KeyDerivationFailed)?;
    Ok(key)
}

fn encrypt(
    shared_secret: &EcdhSharedSecret,
    aad: &[u8],
    nonce: &Nonce,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut key = derive_aead_key(shared_secret, aad)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::EncryptionFailed)?;
    let result = cipher
        .encrypt(
            XNonce::from_slice(nonce.as_bytes()),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::EncryptionFailed);
    key.zeroize();
    result
}

fn decrypt(
    shared_secret: &EcdhSharedSecret,
    aad: &[u8],
    nonce: &Nonce,
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut key = derive_aead_key(shared_secret, aad)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::DecryptionFailed)?;
    let result = cipher
        .decrypt(
            XNonce::from_slice(nonce.as_bytes()),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CryptoError::DecryptionFailed);
    key.zeroize();
    result
}

fn ciphertext_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use p256::{ecdh::diffie_hellman, SecretKey};

    use super::*;

    fn recipient() -> (SecretKey, DeviceKeyAgreementPublicKey) {
        let secret = SecretKey::from_slice(&[7u8; 32]).expect("secret");
        let public = secret.public_key().to_encoded_point(false);
        let profile = DeviceKeyAgreementPublicKey::p256(
            "device-a",
            "agreement-a",
            public.as_bytes(),
            10,
            None,
        )
        .expect("profile");
        (secret, profile)
    }

    fn shared(secret: &SecretKey, record: &WrappedEpochMaterial) -> EcdhSharedSecret {
        let ephemeral =
            PublicKey::from_sec1_bytes(record.ephemeral_public_key().expect("ephemeral"))
                .expect("ephemeral public key");
        let shared = diffie_hellman(secret.to_nonzero_scalar(), ephemeral.as_affine());
        EcdhSharedSecret::new((*shared.raw_secret_bytes()).into()).expect("shared secret")
    }

    #[test]
    fn wrapped_epoch_round_trip_and_debug_redaction() {
        let (secret, recipient) = recipient();
        let descriptor = KeyDescriptor::new("object-key-1", KeyRole::ObjectKey, 1).expect("key");
        let master = SyncMasterKeyMaterial::new([9u8; 32]).expect("master");
        let record = WrappedEpochMaterial::seal_for_recipient(
            "domain-a",
            &recipient,
            "wrapping-a-1",
            &descriptor,
            &master,
            Nonce::new(vec![3u8; 24]).expect("nonce"),
            20,
        )
        .expect("record");
        let (opened_descriptor, opened_master) =
            record.unwrap(&shared(&secret, &record)).expect("open");
        assert_eq!(opened_descriptor, descriptor);
        assert_eq!(opened_master, master);
        let debug = format!("{record:?} {opened_master:?}");
        assert!(!debug.contains(&format!("{:?}", master.as_bytes())));
        assert!(!debug.contains(&format!("{:?}", record.wrapped_key)));
    }

    #[test]
    fn wrapped_epoch_rejects_metadata_and_ciphertext_tampering() {
        let (secret, recipient) = recipient();
        let descriptor = KeyDescriptor::new("object-key-2", KeyRole::ObjectKey, 2).expect("key");
        let master = SyncMasterKeyMaterial::new([8u8; 32]).expect("master");
        let record = WrappedEpochMaterial::seal_for_recipient(
            "domain-a",
            &recipient,
            "wrapping-a-2",
            &descriptor,
            &master,
            Nonce::new(vec![4u8; 24]).expect("nonce"),
            30,
        )
        .expect("record");
        for mutate in [
            |value: &mut WrappedEpochMaterial| value.domain_id.push('x'),
            |value: &mut WrappedEpochMaterial| value.recipient_device_id.push('x'),
            |value: &mut WrappedEpochMaterial| value.recipient_key_agreement_key_id.push('x'),
            |value: &mut WrappedEpochMaterial| value.wrapping_key_id.push('x'),
        ] {
            let mut tampered = record.clone();
            mutate(&mut tampered);
            assert_eq!(
                tampered.unwrap(&shared(&secret, &tampered)),
                Err(CryptoError::DecryptionFailed)
            );
        }
        let mut tampered = record.clone();
        let last = tampered.wrapped_key.len() - 1;
        tampered.wrapped_key[last] ^= 1;
        assert_eq!(
            tampered.unwrap(&shared(&secret, &record)),
            Err(CryptoError::CiphertextHashMismatch)
        );
    }

    #[test]
    fn wrong_device_private_key_cannot_unwrap() {
        let (_secret, recipient) = recipient();
        let wrong_secret = SecretKey::from_slice(&[6u8; 32]).expect("wrong secret");
        let descriptor = KeyDescriptor::new("object-key-1", KeyRole::ObjectKey, 1).expect("key");
        let master = SyncMasterKeyMaterial::new([5u8; 32]).expect("master");
        let record = WrappedEpochMaterial::seal_for_recipient(
            "domain-a",
            &recipient,
            "wrapping-a-1",
            &descriptor,
            &master,
            Nonce::new(vec![5u8; 24]).expect("nonce"),
            40,
        )
        .expect("record");
        assert_eq!(
            record.unwrap(&shared(&wrong_secret, &record)),
            Err(CryptoError::DecryptionFailed)
        );
    }
}
