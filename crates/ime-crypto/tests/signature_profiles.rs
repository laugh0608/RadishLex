use radishlex_ime_crypto::{
    CryptoError, DeviceSignature, DeviceSigningPublicKey, SignatureAlgorithmId,
};

const VECTORS: &str = include_str!("../../../tests/fixtures/device-signature-profiles-v1.txt");

#[test]
fn device_signature_profiles_match_shared_cross_language_vectors() {
    for line in VECTORS.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let vector = SignatureProfileVector::parse(line);
        let actual = verify_vector(&vector)
            .map(|()| "ok")
            .unwrap_or_else(signature_error_detail);
        assert_eq!(actual, vector.expected_detail, "case_id={}", vector.case_id);
    }
}

fn verify_vector(vector: &SignatureProfileVector) -> Result<(), CryptoError> {
    let signature_algorithm = SignatureAlgorithmId::new(vector.signature_algorithm.clone())?;
    let key_algorithm = SignatureAlgorithmId::new(vector.key_algorithm.clone())?;
    let public_key = DeviceSigningPublicKey::new(
        "device-vector",
        "signing-key-vector",
        key_algorithm,
        vector.public_key.clone(),
        vector.created_at_ms,
        vector.revoked_at_ms,
    )?;
    let signature = DeviceSignature::new_for_algorithm(
        signature_algorithm,
        "signing-key-vector",
        "device-vector",
        vector.signature.clone(),
    )?;
    signature.verify_at(&public_key, &vector.canonical, vector.signed_at_ms)
}

fn signature_error_detail(error: CryptoError) -> &'static str {
    match error {
        CryptoError::UnsupportedSignatureAlgorithm { .. } => "unsupported_signature_algorithm",
        CryptoError::SignatureAlgorithmMismatch => "signature_algorithm_mismatch",
        CryptoError::InvalidSigningPublicKey { .. } => "invalid_signing_public_key",
        CryptoError::InvalidSignatureEncoding { .. } => "invalid_signature_encoding",
        CryptoError::SignatureVerificationFailed => "signature_verification_failed",
        CryptoError::SignatureKeyNotActive { .. } => "signature_key_not_active",
        other => panic!("unexpected signature profile error: {other}"),
    }
}

struct SignatureProfileVector {
    case_id: String,
    signature_algorithm: String,
    key_algorithm: String,
    public_key: Vec<u8>,
    signature: Vec<u8>,
    canonical: Vec<u8>,
    created_at_ms: i64,
    revoked_at_ms: Option<i64>,
    signed_at_ms: i64,
    expected_detail: String,
}

impl SignatureProfileVector {
    fn parse(line: &str) -> Self {
        let fields: Vec<_> = line.split('|').collect();
        assert_eq!(fields.len(), 10, "invalid vector line: {line}");
        Self {
            case_id: fields[0].to_owned(),
            signature_algorithm: fields[1].to_owned(),
            key_algorithm: fields[2].to_owned(),
            public_key: decode_hex(fields[3]),
            signature: decode_hex(fields[4]),
            canonical: decode_hex(fields[5]),
            created_at_ms: fields[6].parse().expect("created_at_ms"),
            revoked_at_ms: (!fields[7].is_empty())
                .then(|| fields[7].parse().expect("revoked_at_ms")),
            signed_at_ms: fields[8].parse().expect("signed_at_ms"),
            expected_detail: fields[9].to_owned(),
        }
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0, "hex value must have even length");
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex utf8");
            u8::from_str_radix(pair, 16).expect("hex byte")
        })
        .collect()
}
