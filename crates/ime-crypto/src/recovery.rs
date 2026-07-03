use std::fmt;

use argon2::{Algorithm, Argon2, Params, Version};

use crate::device::RecoveryMaterial;
use crate::model::{
    decrypt_xchacha20poly1305_raw, encrypt_xchacha20poly1305_raw, validate_key_material,
    AlgorithmId, CryptoError, Nonce, SyncMasterKeyMaterial, OBJECT_KEY_LEN,
};

pub const RECOVERY_CODE_PREFIX: &str = "RLX1";
pub const RECOVERY_CODE_SECRET_LEN: usize = 20;
pub const RECOVERY_SALT_LEN: usize = 16;
pub const RECOVERY_WRAPPING_KEY_LEN: usize = OBJECT_KEY_LEN;
pub const RECOVERY_KDF_ID_ARGON2ID_V1: &str = "argon2id-v1";
pub const RECOVERY_KDF_VERSION_ARGON2ID_V1: u16 = 1;
pub const ARGON2_VERSION_V0X13: u32 = 0x13;

const RECOVERY_CODE_GROUPS: usize = 8;
const RECOVERY_CODE_GROUP_LEN: usize = 4;
const RECOVERY_CODE_SECRET_CHARS: usize = RECOVERY_CODE_GROUPS * RECOVERY_CODE_GROUP_LEN;
const ARGON2ID_V1_MEMORY_KIB: u32 = 65_536;
const ARGON2ID_V1_ITERATIONS: u32 = 3;
const ARGON2ID_V1_PARALLELISM: u32 = 4;
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryCode {
    secret: [u8; RECOVERY_CODE_SECRET_LEN],
}

impl RecoveryCode {
    pub fn parse(input: &str) -> Result<Self, CryptoError> {
        let normalized = input.trim().to_ascii_uppercase();
        let segments: Vec<&str> = normalized.split('-').collect();
        if segments.len() != RECOVERY_CODE_GROUPS + 2 {
            return Err(CryptoError::invalid_field(
                "recovery_code",
                "value must use RLX1 plus eight secret groups and one checksum group",
            ));
        }
        if segments[0] != RECOVERY_CODE_PREFIX {
            return Err(CryptoError::invalid_field(
                "recovery_code",
                "value must start with RLX1",
            ));
        }

        let mut encoded_secret = String::with_capacity(RECOVERY_CODE_SECRET_CHARS);
        for segment in &segments[1..=RECOVERY_CODE_GROUPS] {
            if segment.len() != RECOVERY_CODE_GROUP_LEN {
                return Err(CryptoError::invalid_field(
                    "recovery_code",
                    "secret groups must contain four characters",
                ));
            }
            encoded_secret.push_str(segment);
        }

        let checksum_segment = segments[RECOVERY_CODE_GROUPS + 1];
        if checksum_segment.len() != 1 {
            return Err(CryptoError::invalid_field(
                "recovery_code",
                "checksum group must contain one character",
            ));
        }

        let secret = decode_secret(&encoded_secret)?;
        let expected = recovery_code_checksum(&secret);
        let actual = decode_crockford_char(checksum_segment.as_bytes()[0])?;
        if actual != expected {
            return Err(CryptoError::invalid_field(
                "recovery_code",
                "checksum mismatch",
            ));
        }

        Ok(Self { secret })
    }

    fn secret_bytes(&self) -> &[u8; RECOVERY_CODE_SECRET_LEN] {
        &self.secret
    }
}

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode([redacted])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryKdfProfile {
    pub kdf_id: String,
    pub kdf_version: u16,
    pub argon2_version: u32,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt_len: usize,
    pub output_len: usize,
}

impl RecoveryKdfProfile {
    pub fn argon2id_v1() -> Self {
        Self {
            kdf_id: RECOVERY_KDF_ID_ARGON2ID_V1.to_owned(),
            kdf_version: RECOVERY_KDF_VERSION_ARGON2ID_V1,
            argon2_version: ARGON2_VERSION_V0X13,
            memory_kib: ARGON2ID_V1_MEMORY_KIB,
            iterations: ARGON2ID_V1_ITERATIONS,
            parallelism: ARGON2ID_V1_PARALLELISM,
            salt_len: RECOVERY_SALT_LEN,
            output_len: RECOVERY_WRAPPING_KEY_LEN,
        }
    }

    pub fn from_recovery_material(material: &RecoveryMaterial) -> Result<Self, CryptoError> {
        let profile = Self {
            kdf_id: material.kdf_id.clone(),
            kdf_version: material.kdf_version,
            argon2_version: ARGON2_VERSION_V0X13,
            memory_kib: material.memory_kib,
            iterations: material.iterations,
            parallelism: material.parallelism,
            salt_len: material.salt.len(),
            output_len: material.output_len,
        };
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), CryptoError> {
        if self.kdf_id != RECOVERY_KDF_ID_ARGON2ID_V1 {
            return Err(CryptoError::invalid_field(
                "kdf_id",
                format!("unsupported recovery KDF {}", self.kdf_id),
            ));
        }
        if self.kdf_version != RECOVERY_KDF_VERSION_ARGON2ID_V1 {
            return Err(CryptoError::invalid_field(
                "kdf_version",
                format!("value must be {RECOVERY_KDF_VERSION_ARGON2ID_V1}"),
            ));
        }
        if self.argon2_version != ARGON2_VERSION_V0X13 {
            return Err(CryptoError::invalid_field(
                "argon2_version",
                format!("value must be {ARGON2_VERSION_V0X13:#x}"),
            ));
        }
        if self.memory_kib < ARGON2ID_V1_MEMORY_KIB {
            return Err(CryptoError::invalid_field(
                "memory_kib",
                format!("value must be at least {ARGON2ID_V1_MEMORY_KIB}"),
            ));
        }
        if self.iterations < ARGON2ID_V1_ITERATIONS {
            return Err(CryptoError::invalid_field(
                "iterations",
                format!("value must be at least {ARGON2ID_V1_ITERATIONS}"),
            ));
        }
        if self.parallelism < ARGON2ID_V1_PARALLELISM {
            return Err(CryptoError::invalid_field(
                "parallelism",
                format!("value must be at least {ARGON2ID_V1_PARALLELISM}"),
            ));
        }
        if self.salt_len < RECOVERY_SALT_LEN {
            return Err(CryptoError::invalid_field(
                "salt_len",
                format!("value must be at least {RECOVERY_SALT_LEN}"),
            ));
        }
        if self.output_len != RECOVERY_WRAPPING_KEY_LEN {
            return Err(CryptoError::invalid_field(
                "output_len",
                format!("value must be {RECOVERY_WRAPPING_KEY_LEN}"),
            ));
        }
        Ok(())
    }

    pub fn derive_wrapping_key(
        &self,
        code: &RecoveryCode,
        salt: &[u8],
    ) -> Result<RecoveryWrappingKeyMaterial, CryptoError> {
        self.validate()?;
        if salt.len() < self.salt_len {
            return Err(CryptoError::invalid_field(
                "salt",
                format!("value must be at least {} bytes", self.salt_len),
            ));
        }

        let params = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(self.output_len),
        )
        .map_err(|_| CryptoError::KeyDerivationFailed)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut output = [0u8; RECOVERY_WRAPPING_KEY_LEN];
        argon2
            .hash_password_into(code.secret_bytes(), salt, &mut output)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        RecoveryWrappingKeyMaterial::new(output)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryWrappingKeyMaterial([u8; RECOVERY_WRAPPING_KEY_LEN]);

impl RecoveryWrappingKeyMaterial {
    pub fn new(bytes: [u8; RECOVERY_WRAPPING_KEY_LEN]) -> Result<Self, CryptoError> {
        validate_key_material("recovery_wrapping_key", &bytes)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; RECOVERY_WRAPPING_KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for RecoveryWrappingKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryWrappingKeyMaterial([redacted])")
    }
}

impl RecoveryMaterial {
    #[allow(clippy::too_many_arguments)]
    pub fn encrypt_sync_master_key(
        recovery_id: impl Into<String>,
        domain_id: impl Into<String>,
        key_epoch: u64,
        profile: &RecoveryKdfProfile,
        code: &RecoveryCode,
        salt: impl Into<Vec<u8>>,
        sync_master_key: &SyncMasterKeyMaterial,
        timestamp_ms: i64,
        envelope_nonce: Nonce,
    ) -> Result<Self, CryptoError> {
        let salt = salt.into();
        let wrapping_key = profile.derive_wrapping_key(code, &salt)?;
        let material = Self::new(
            recovery_id,
            domain_id,
            key_epoch,
            profile.kdf_id.clone(),
            profile.kdf_version,
            salt,
            profile.memory_kib,
            profile.iterations,
            profile.parallelism,
            profile.output_len,
            AlgorithmId::xchacha20poly1305_hkdf_sha256(),
            envelope_nonce,
            b"pending",
            timestamp_ms,
            timestamp_ms,
        )?;
        let associated_data = material.associated_data().to_bytes();
        let encrypted_recovery_key = encrypt_xchacha20poly1305_raw(
            wrapping_key.as_bytes(),
            &material.envelope_nonce,
            &associated_data,
            sync_master_key.as_bytes(),
        )?;

        Self::new(
            material.recovery_id,
            material.domain_id,
            material.key_epoch,
            material.kdf_id,
            material.kdf_version,
            material.salt,
            material.memory_kib,
            material.iterations,
            material.parallelism,
            material.output_len,
            material.envelope_algorithm,
            material.envelope_nonce,
            encrypted_recovery_key,
            material.created_at_ms,
            material.updated_at_ms,
        )
    }

    pub fn decrypt_sync_master_key(
        &self,
        code: &RecoveryCode,
    ) -> Result<SyncMasterKeyMaterial, CryptoError> {
        self.validate()?;
        let profile = RecoveryKdfProfile::from_recovery_material(self)?;
        let wrapping_key = profile.derive_wrapping_key(code, &self.salt)?;
        let plaintext = decrypt_xchacha20poly1305_raw(
            wrapping_key.as_bytes(),
            &self.envelope_nonce,
            &self.associated_data().to_bytes(),
            &self.encrypted_recovery_key,
        )?;
        let key: [u8; OBJECT_KEY_LEN] = plaintext.try_into().map_err(|_| {
            CryptoError::invalid_field("recovery_key", "decrypted material must be 32 bytes")
        })?;
        SyncMasterKeyMaterial::new(key)
    }
}

fn decode_secret(encoded: &str) -> Result<[u8; RECOVERY_CODE_SECRET_LEN], CryptoError> {
    if encoded.len() != RECOVERY_CODE_SECRET_CHARS {
        return Err(CryptoError::invalid_field(
            "recovery_code",
            format!("secret must contain {RECOVERY_CODE_SECRET_CHARS} characters"),
        ));
    }

    let mut output = [0u8; RECOVERY_CODE_SECRET_LEN];
    let mut accumulator: u16 = 0;
    let mut bits = 0u8;
    let mut offset = 0usize;

    for byte in encoded.bytes() {
        accumulator = (accumulator << 5) | u16::from(decode_crockford_char(byte)?);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            if offset >= RECOVERY_CODE_SECRET_LEN {
                return Err(CryptoError::invalid_field(
                    "recovery_code",
                    "secret is longer than expected",
                ));
            }
            output[offset] = ((accumulator >> bits) & 0xff) as u8;
            offset += 1;
        }
        accumulator &= low_bits_mask(bits);
    }

    if offset != RECOVERY_CODE_SECRET_LEN || bits != 0 {
        return Err(CryptoError::invalid_field(
            "recovery_code",
            "secret bit length is invalid",
        ));
    }
    Ok(output)
}

fn decode_crockford_char(byte: u8) -> Result<u8, CryptoError> {
    match byte {
        b'0' | b'O' | b'o' => Ok(0),
        b'1' | b'I' | b'i' | b'L' | b'l' => Ok(1),
        b'2'..=b'9' => Ok(byte - b'0'),
        b'A' | b'a' => Ok(10),
        b'B' | b'b' => Ok(11),
        b'C' | b'c' => Ok(12),
        b'D' | b'd' => Ok(13),
        b'E' | b'e' => Ok(14),
        b'F' | b'f' => Ok(15),
        b'G' | b'g' => Ok(16),
        b'H' | b'h' => Ok(17),
        b'J' | b'j' => Ok(18),
        b'K' | b'k' => Ok(19),
        b'M' | b'm' => Ok(20),
        b'N' | b'n' => Ok(21),
        b'P' | b'p' => Ok(22),
        b'Q' | b'q' => Ok(23),
        b'R' | b'r' => Ok(24),
        b'S' | b's' => Ok(25),
        b'T' | b't' => Ok(26),
        b'V' | b'v' => Ok(27),
        b'W' | b'w' => Ok(28),
        b'X' | b'x' => Ok(29),
        b'Y' | b'y' => Ok(30),
        b'Z' | b'z' => Ok(31),
        _ => Err(CryptoError::invalid_field(
            "recovery_code",
            "value contains a non-Crockford Base32 character",
        )),
    }
}

fn recovery_code_checksum(secret: &[u8; RECOVERY_CODE_SECRET_LEN]) -> u8 {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"radishlex-recovery-code-checksum-v1");
    hasher.update(secret);
    (hasher.finalize()[0] & 0x1f) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::XCHACHA20POLY1305_NONCE_LEN;

    const CROCKFORD_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

    #[test]
    fn recovery_code_parses_crockford_secret_and_redacts_debug() {
        let code = RecoveryCode::parse(&format_recovery_code([7u8; RECOVERY_CODE_SECRET_LEN]))
            .expect("recovery code parses");

        assert_eq!(code.secret_bytes(), &[7u8; RECOVERY_CODE_SECRET_LEN]);
        let debug = format!("{code:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("RLX1"));
    }

    #[test]
    fn recovery_code_rejects_bad_format_and_checksum() {
        let error = RecoveryCode::parse("RLX1-short").expect_err("short code fails");
        assert!(error.to_string().contains("recovery_code"));

        let mut code = format_recovery_code([9u8; RECOVERY_CODE_SECRET_LEN]);
        code.pop();
        code.push('0');
        let error = RecoveryCode::parse(&code).expect_err("checksum mismatch fails");
        assert!(error.to_string().contains("checksum"));
    }

    #[test]
    fn kdf_profile_validates_current_security_floor() {
        let profile = RecoveryKdfProfile::argon2id_v1();
        profile.validate().expect("default profile validates");

        let mut weak = profile.clone();
        weak.memory_kib = 19 * 1024;
        let error = weak.validate().expect_err("weak memory fails");
        assert!(error.to_string().contains("memory_kib"));

        let mut unknown = profile;
        unknown.kdf_id = "pbkdf2".to_owned();
        let error = unknown.validate().expect_err("unknown KDF fails");
        assert!(error.to_string().contains("kdf_id"));
    }

    #[test]
    fn kdf_derives_stable_key_and_changes_with_salt() {
        let code = RecoveryCode::parse(&format_recovery_code([3u8; RECOVERY_CODE_SECRET_LEN]))
            .expect("code");
        let profile = low_cost_test_profile();
        let first = profile
            .derive_wrapping_key(&code, b"0123456789abcdef")
            .expect("first key");
        let second = profile
            .derive_wrapping_key(&code, b"0123456789abcdef")
            .expect("second key");
        let changed = profile
            .derive_wrapping_key(&code, b"fedcba9876543210")
            .expect("changed salt");

        assert_eq!(first, second);
        assert_ne!(first, changed);
        assert!(format!("{first:?}").contains("[redacted]"));
    }

    #[test]
    fn recovery_material_encrypts_and_decrypts_sync_master_key() {
        let profile = low_cost_test_profile();
        let code = RecoveryCode::parse(&format_recovery_code([4u8; RECOVERY_CODE_SECRET_LEN]))
            .expect("code");
        let master_key = SyncMasterKeyMaterial::new([42u8; OBJECT_KEY_LEN]).expect("master");
        let material = RecoveryMaterial::encrypt_sync_master_key(
            "recovery-a",
            "domain-a",
            3,
            &profile,
            &code,
            b"0123456789abcdef".to_vec(),
            &master_key,
            100,
            nonce(7),
        )
        .expect("material");

        assert_ne!(material.encrypted_recovery_key, master_key.as_bytes());
        let debug = format!("{material:?}");
        assert!(debug.contains("salt_len"));
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("encrypted_recovery_key: [42"));

        let decrypted = material
            .decrypt_sync_master_key(&code)
            .expect("decrypts with same code");
        assert_eq!(decrypted, master_key);
    }

    #[test]
    fn recovery_material_rejects_wrong_code_and_aad_mutation() {
        let profile = low_cost_test_profile();
        let code = RecoveryCode::parse(&format_recovery_code([5u8; RECOVERY_CODE_SECRET_LEN]))
            .expect("code");
        let wrong_code =
            RecoveryCode::parse(&format_recovery_code([6u8; RECOVERY_CODE_SECRET_LEN]))
                .expect("wrong code");
        let master_key = SyncMasterKeyMaterial::new([42u8; OBJECT_KEY_LEN]).expect("master");
        let material = RecoveryMaterial::encrypt_sync_master_key(
            "recovery-a",
            "domain-a",
            3,
            &profile,
            &code,
            b"0123456789abcdef".to_vec(),
            &master_key,
            100,
            nonce(7),
        )
        .expect("material");

        let error = material
            .decrypt_sync_master_key(&wrong_code)
            .expect_err("wrong recovery code cannot decrypt");
        assert_eq!(error, CryptoError::DecryptionFailed);

        let mut tampered = material;
        tampered.domain_id = "domain-b".to_owned();
        let error = tampered
            .decrypt_sync_master_key(&code)
            .expect_err("AAD mutation fails");
        assert_eq!(error, CryptoError::DecryptionFailed);
    }

    fn low_cost_test_profile() -> RecoveryKdfProfile {
        RecoveryKdfProfile {
            kdf_id: RECOVERY_KDF_ID_ARGON2ID_V1.to_owned(),
            kdf_version: RECOVERY_KDF_VERSION_ARGON2ID_V1,
            argon2_version: ARGON2_VERSION_V0X13,
            memory_kib: ARGON2ID_V1_MEMORY_KIB,
            iterations: ARGON2ID_V1_ITERATIONS,
            parallelism: ARGON2ID_V1_PARALLELISM,
            salt_len: RECOVERY_SALT_LEN,
            output_len: RECOVERY_WRAPPING_KEY_LEN,
        }
    }

    fn format_recovery_code(secret: [u8; RECOVERY_CODE_SECRET_LEN]) -> String {
        let mut encoded = String::with_capacity(RECOVERY_CODE_SECRET_CHARS);
        let mut accumulator = 0u16;
        let mut bits = 0u8;

        for byte in secret {
            accumulator = (accumulator << 8) | u16::from(byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                encoded.push(encode_crockford_char(((accumulator >> bits) & 0x1f) as u8));
            }
            accumulator &= low_bits_mask(bits);
        }
        assert_eq!(bits, 0);

        let mut groups = Vec::with_capacity(RECOVERY_CODE_GROUPS);
        for index in 0..RECOVERY_CODE_GROUPS {
            let start = index * RECOVERY_CODE_GROUP_LEN;
            let end = start + RECOVERY_CODE_GROUP_LEN;
            groups.push(&encoded[start..end]);
        }

        format!(
            "{RECOVERY_CODE_PREFIX}-{}-{}",
            groups.join("-"),
            encode_crockford_char(recovery_code_checksum(&secret))
        )
    }

    fn encode_crockford_char(value: u8) -> char {
        CROCKFORD_ALPHABET[value as usize] as char
    }

    fn nonce(seed: u8) -> Nonce {
        Nonce::new(vec![seed; XCHACHA20POLY1305_NONCE_LEN]).expect("nonce")
    }
}

fn low_bits_mask(bits: u8) -> u16 {
    if bits == 0 {
        0
    } else {
        (1u16 << bits) - 1
    }
}
