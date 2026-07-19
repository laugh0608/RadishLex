use radishlex_ime_crypto::{
    CryptoError, DeviceKeyAgreementKeyHandle, DeviceKeyAgreementPublicKey, EcdhSharedSecret,
};
use radishlex_ime_sync::SyncDeviceKeyAgreementBackend;

const TEST_KEY_CREATED_AT_MS: i64 = 1_790_001_000_000;

pub fn agreement_public_key(scalar: u8) -> Vec<u8> {
    use p256::elliptic_curve::sec1::ToEncodedPoint;

    let mut secret = [0u8; 32];
    secret[31] = scalar;
    p256::SecretKey::from_slice(&secret)
        .expect("test agreement secret")
        .public_key()
        .to_encoded_point(false)
        .as_bytes()
        .to_vec()
}

pub struct TestP256AgreementBackend {
    device_id: String,
    key_id: String,
    secret: p256::SecretKey,
    public_key: Vec<u8>,
}

impl TestP256AgreementBackend {
    pub fn new(device_id: &str, key_id: &str, scalar: u8) -> Self {
        let mut secret = [0u8; 32];
        secret[31] = scalar;
        let secret = p256::SecretKey::from_slice(&secret).expect("test agreement secret");
        let public_key = {
            use p256::elliptic_curve::sec1::ToEncodedPoint;
            secret
                .public_key()
                .to_encoded_point(false)
                .as_bytes()
                .to_vec()
        };
        Self {
            device_id: device_id.to_owned(),
            key_id: key_id.to_owned(),
            secret,
            public_key,
        }
    }
}

impl SyncDeviceKeyAgreementBackend for TestP256AgreementBackend {
    fn key_handle(
        &self,
        device_id: &str,
        key_id: &str,
    ) -> Result<DeviceKeyAgreementKeyHandle, CryptoError> {
        if device_id != self.device_id || key_id != self.key_id {
            return Err(CryptoError::PrivateKeyUnavailable {
                key_id: key_id.to_owned(),
            });
        }
        DeviceKeyAgreementKeyHandle::p256(device_id, key_id, "test-p256-agreement-v1")
    }

    fn public_key(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
    ) -> Result<DeviceKeyAgreementPublicKey, CryptoError> {
        if handle.device_id != self.device_id || handle.key_id != self.key_id {
            return Err(CryptoError::PrivateKeyUnavailable {
                key_id: handle.key_id.clone(),
            });
        }
        DeviceKeyAgreementPublicKey::p256(
            &self.device_id,
            &self.key_id,
            self.public_key.clone(),
            TEST_KEY_CREATED_AT_MS,
            None,
        )
    }

    fn derive_shared_secret(
        &self,
        handle: &DeviceKeyAgreementKeyHandle,
        peer_public_key: &[u8],
    ) -> Result<EcdhSharedSecret, CryptoError> {
        if handle.device_id != self.device_id || handle.key_id != self.key_id {
            return Err(CryptoError::PrivateKeyUnavailable {
                key_id: handle.key_id.clone(),
            });
        }
        let peer = p256::PublicKey::from_sec1_bytes(peer_public_key)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        let shared = p256::ecdh::diffie_hellman(self.secret.to_nonzero_scalar(), peer.as_affine());
        EcdhSharedSecret::new((*shared.raw_secret_bytes()).into())
    }
}
