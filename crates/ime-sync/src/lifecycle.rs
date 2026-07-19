use std::collections::BTreeMap;
use std::fmt;

use radishlex_ime_crypto::{
    canonical_signature_bytes, DeviceKeyAgreementPublicKey, DeviceSignature,
    DeviceSigningPublicKey, SignatureAlgorithmId, SignatureField,
};
use sha2::{Digest, Sha256};

use crate::remote::{
    OpaqueSyncCursor, RemoteDeviceAuthorization, RemoteDeviceRevocation, RemoteLifecycleDevice,
    RemoteLifecycleEvent, RemoteLifecycleEventKind, RemoteLifecycleSnapshot,
};
use crate::{
    SignedDeviceAuthorization, SignedDeviceRevocation, SyncDevice, SyncDeviceStatus,
    SyncTrustedDeviceProfile, SyncTrustedDomainState,
};

const JOIN_PROFILE_CHALLENGE_PREFIX: &str = "profile-sha256-v1:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncLifecycleError {
    InvalidSnapshot,
    InvalidTrustAnchor,
    InvalidAuthorization,
    InvalidRevocation,
    InvalidSequence,
}

impl fmt::Display for SyncLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSnapshot => "lifecycle snapshot is invalid",
            Self::InvalidTrustAnchor => "lifecycle trust anchor does not match",
            Self::InvalidAuthorization => "device authorization is invalid",
            Self::InvalidRevocation => "device revocation is invalid",
            Self::InvalidSequence => "lifecycle sequence or key epoch is invalid",
        };
        f.write_str(message)
    }
}

impl std::error::Error for SyncLifecycleError {}

#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedSyncLifecycle {
    trusted_domain: SyncTrustedDomainState,
    events: Vec<RemoteLifecycleEvent>,
    cursor: OpaqueSyncCursor,
}

impl VerifiedSyncLifecycle {
    pub fn trusted_domain(&self) -> &SyncTrustedDomainState {
        &self.trusted_domain
    }

    pub fn events(&self) -> &[RemoteLifecycleEvent] {
        &self.events
    }

    pub fn cursor(&self) -> &OpaqueSyncCursor {
        &self.cursor
    }

    pub fn into_parts(
        self,
    ) -> (
        SyncTrustedDomainState,
        Vec<RemoteLifecycleEvent>,
        OpaqueSyncCursor,
    ) {
        (self.trusted_domain, self.events, self.cursor)
    }
}

impl fmt::Debug for VerifiedSyncLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedSyncLifecycle")
            .field("trusted_domain", &self.trusted_domain)
            .field("event_count", &self.events.len())
            .field("cursor", &self.cursor)
            .finish()
    }
}

#[derive(Clone)]
struct TrustedDeviceRecord {
    device: SyncDevice,
    public_key: DeviceSigningPublicKey,
    key_agreement_public_key: DeviceKeyAgreementPublicKey,
    reject_from_change_sequence: Option<u64>,
}

pub fn verify_lifecycle_snapshot(
    snapshot: RemoteLifecycleSnapshot,
    trust_anchor: &DeviceSigningPublicKey,
) -> Result<VerifiedSyncLifecycle, SyncLifecycleError> {
    snapshot
        .domain
        .validate()
        .map_err(|_| SyncLifecycleError::InvalidSnapshot)?;
    trust_anchor
        .validate()
        .map_err(|_| SyncLifecycleError::InvalidTrustAnchor)?;
    if snapshot.entries.is_empty() {
        return Err(SyncLifecycleError::InvalidSnapshot);
    }

    let mut devices = BTreeMap::<String, TrustedDeviceRecord>::new();
    let mut current_epoch = 0;
    for (index, event) in snapshot.entries.iter().enumerate() {
        let expected_sequence = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(SyncLifecycleError::InvalidSequence)?;
        if event.lifecycle_sequence != expected_sequence
            || event.domain_id != snapshot.domain.domain_id
        {
            return Err(SyncLifecycleError::InvalidSequence);
        }
        match event.event_type {
            RemoteLifecycleEventKind::InitialDevice => {
                if index != 0 || current_epoch != 0 {
                    return Err(SyncLifecycleError::InvalidSequence);
                }
                apply_initial_device(event, trust_anchor, &mut devices)?;
                current_epoch = event.key_epoch;
            }
            RemoteLifecycleEventKind::DeviceAuthorized => {
                if event.key_epoch != current_epoch {
                    return Err(SyncLifecycleError::InvalidSequence);
                }
                apply_authorization(event, &mut devices)?;
            }
            RemoteLifecycleEventKind::DeviceRevoked => {
                let revocation = event
                    .revocation
                    .as_ref()
                    .ok_or(SyncLifecycleError::InvalidRevocation)?;
                if revocation.previous_key_epoch != current_epoch
                    || revocation.new_key_epoch != event.key_epoch
                {
                    return Err(SyncLifecycleError::InvalidSequence);
                }
                apply_revocation(event, &mut devices)?;
                current_epoch = event.key_epoch;
            }
        }
    }
    if current_epoch != snapshot.domain.current_key_epoch {
        return Err(SyncLifecycleError::InvalidSequence);
    }

    let profiles = devices
        .into_values()
        .map(|record| match record.reject_from_change_sequence {
            Some(sequence) => {
                SyncTrustedDeviceProfile::revoked_with_key_agreement_from_change_sequence(
                    record.device,
                    record.public_key,
                    record.key_agreement_public_key,
                    sequence,
                )
            }
            None => SyncTrustedDeviceProfile::active_with_key_agreement(
                record.device,
                record.public_key,
                record.key_agreement_public_key,
            ),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SyncLifecycleError::InvalidSnapshot)?;
    let trusted_domain = SyncTrustedDomainState::new(snapshot.domain, profiles)
        .map_err(|_| SyncLifecycleError::InvalidSnapshot)?;
    Ok(VerifiedSyncLifecycle {
        trusted_domain,
        events: snapshot.entries,
        cursor: snapshot.next_cursor,
    })
}

fn apply_initial_device(
    event: &RemoteLifecycleEvent,
    trust_anchor: &DeviceSigningPublicKey,
    devices: &mut BTreeMap<String, TrustedDeviceRecord>,
) -> Result<(), SyncLifecycleError> {
    let profile = &event.device;
    if event.record_id != profile.device_id
        || profile.status != SyncDeviceStatus::Active
        || profile.revoked_at_ms.is_some()
        || profile.authorized_at_ms != Some(event.created_at_ms)
        || profile.device_id != trust_anchor.device_id
        || profile.signing_public_key_id != trust_anchor.signing_key_id
        || profile.signing_algorithm != trust_anchor.signature_algorithm.as_str()
        || profile.signing_public_key != trust_anchor.public_key
        || trust_anchor.created_at_ms > event.created_at_ms
        || trust_anchor.revoked_at_ms.is_some()
    {
        return Err(SyncLifecycleError::InvalidTrustAnchor);
    }
    let device = sync_device_at_authorization(profile, event.created_at_ms)?;
    let key_agreement_public_key =
        key_agreement_public_key_from_profile(profile, event.created_at_ms, None)?;
    devices.insert(
        profile.device_id.clone(),
        TrustedDeviceRecord {
            device,
            public_key: trust_anchor.clone(),
            key_agreement_public_key,
            reject_from_change_sequence: None,
        },
    );
    Ok(())
}

fn apply_authorization(
    event: &RemoteLifecycleEvent,
    devices: &mut BTreeMap<String, TrustedDeviceRecord>,
) -> Result<(), SyncLifecycleError> {
    let authorization = event
        .authorization
        .as_ref()
        .ok_or(SyncLifecycleError::InvalidAuthorization)?;
    let profile = &event.device;
    if authorization.domain_id != event.domain_id
        || authorization.join_request_id != event.record_id
        || authorization.recipient_device_id != profile.device_id
        || authorization.recipient_signing_public_key_id != profile.signing_public_key_id
        || authorization.recipient_key_agreement_key_id != profile.key_agreement_public_key_id
        || authorization.key_epoch != event.key_epoch
        || authorization.created_at_ms != event.created_at_ms
        || profile.status != SyncDeviceStatus::Active
        || profile.authorized_at_ms != Some(event.created_at_ms)
        || profile.revoked_at_ms.is_some()
        || devices.contains_key(&profile.device_id)
    {
        return Err(SyncLifecycleError::InvalidAuthorization);
    }
    let expected_challenge = device_join_profile_challenge(
        &event.domain_id,
        &authorization.join_request_id,
        profile,
        authorization.join_created_at_ms,
        authorization.join_expires_at_ms,
    );
    if authorization.join_challenge != expected_challenge {
        return Err(SyncLifecycleError::InvalidAuthorization);
    }

    let authorizer = devices
        .get(&authorization.authorizer_device_id)
        .filter(|record| record.device.status == SyncDeviceStatus::Active)
        .ok_or(SyncLifecycleError::InvalidAuthorization)?;
    let recipient = sync_device_at_authorization(profile, event.created_at_ms)?;
    let signature = device_signature_from_authorization(authorization)?;
    let signed = SignedDeviceAuthorization {
        signature,
        authorizer_device_id: authorization.authorizer_device_id.clone(),
        recipient_device_id: authorization.recipient_device_id.clone(),
        recipient_public_key_id: authorization.recipient_signing_public_key_id.clone(),
        join_challenge: authorization.join_challenge.clone(),
        join_short_code: authorization.join_short_code.clone(),
        key_epoch: authorization.key_epoch,
        wrapping_key_id: authorization.wrapping_key_id.clone(),
        encrypted_key_len: authorization.encrypted_key_len,
        created_at_ms: authorization.created_at_ms,
    };
    signed
        .verify_with_devices(&authorizer.public_key, &authorizer.device, &recipient)
        .map_err(|_| SyncLifecycleError::InvalidAuthorization)?;
    let public_key = public_key_from_profile(profile, authorization.join_created_at_ms, None)?;
    let key_agreement_public_key =
        key_agreement_public_key_from_profile(profile, authorization.join_created_at_ms, None)?;
    devices.insert(
        profile.device_id.clone(),
        TrustedDeviceRecord {
            device: recipient,
            public_key,
            key_agreement_public_key,
            reject_from_change_sequence: None,
        },
    );
    Ok(())
}

fn apply_revocation(
    event: &RemoteLifecycleEvent,
    devices: &mut BTreeMap<String, TrustedDeviceRecord>,
) -> Result<(), SyncLifecycleError> {
    let revocation = event
        .revocation
        .as_ref()
        .ok_or(SyncLifecycleError::InvalidRevocation)?;
    let reject_from = event
        .reject_from_object_change_sequence
        .filter(|sequence| *sequence > 0)
        .ok_or(SyncLifecycleError::InvalidRevocation)?;
    if revocation.domain_id != event.domain_id
        || revocation.revoked_device_id != event.device.device_id
        || revocation.created_at_ms != event.created_at_ms
        || revocation.new_key_epoch != event.key_epoch
        || event.device.authorized_at_ms.is_none()
        || event.device.revoked_at_ms != Some(event.created_at_ms)
        || !matches!(
            event.device.status,
            SyncDeviceStatus::Revoked | SyncDeviceStatus::Lost
        )
    {
        return Err(SyncLifecycleError::InvalidRevocation);
    }
    let revoker = devices
        .get(&revocation.revoker_device_id)
        .filter(|record| record.device.status == SyncDeviceStatus::Active)
        .ok_or(SyncLifecycleError::InvalidRevocation)?
        .clone();
    let signed = signed_revocation(revocation)?;
    signed
        .verify_with_revoker(&revoker.public_key, &revoker.device)
        .map_err(|_| SyncLifecycleError::InvalidRevocation)?;

    let target = devices
        .get_mut(&revocation.revoked_device_id)
        .filter(|record| record.device.status == SyncDeviceStatus::Active)
        .ok_or(SyncLifecycleError::InvalidRevocation)?;
    if target.device.public_key_id != event.device.signing_public_key_id
        || target.public_key.public_key != event.device.signing_public_key
        || target.public_key.signature_algorithm.as_str() != event.device.signing_algorithm
        || target.key_agreement_public_key.key_id != event.device.key_agreement_public_key_id
        || target.key_agreement_public_key.public_key != event.device.key_agreement_public_key
    {
        return Err(SyncLifecycleError::InvalidRevocation);
    }
    let lost = revocation.reason == "device_lost";
    target.device = target
        .device
        .revoke(event.created_at_ms, lost)
        .map_err(|_| SyncLifecycleError::InvalidRevocation)?;
    target.public_key.revoked_at_ms = Some(event.created_at_ms);
    target.key_agreement_public_key.revoked_at_ms = Some(event.created_at_ms);
    target.reject_from_change_sequence = Some(reject_from);
    Ok(())
}

fn key_agreement_public_key_from_profile(
    profile: &RemoteLifecycleDevice,
    created_at_ms: i64,
    revoked_at_ms: Option<i64>,
) -> Result<DeviceKeyAgreementPublicKey, SyncLifecycleError> {
    DeviceKeyAgreementPublicKey::p256(
        profile.device_id.clone(),
        profile.key_agreement_public_key_id.clone(),
        profile.key_agreement_public_key.clone(),
        created_at_ms,
        revoked_at_ms,
    )
    .map_err(|_| SyncLifecycleError::InvalidSnapshot)
}

fn sync_device_at_authorization(
    profile: &RemoteLifecycleDevice,
    authorized_at_ms: i64,
) -> Result<SyncDevice, SyncLifecycleError> {
    SyncDevice::new(
        profile.device_id.clone(),
        profile.signing_public_key_id.clone(),
        SyncDeviceStatus::Active,
        Some(authorized_at_ms),
        None,
        profile.last_seen_at_ms,
    )
    .map_err(|_| SyncLifecycleError::InvalidSnapshot)
}

fn public_key_from_profile(
    profile: &RemoteLifecycleDevice,
    created_at_ms: i64,
    revoked_at_ms: Option<i64>,
) -> Result<DeviceSigningPublicKey, SyncLifecycleError> {
    let algorithm = SignatureAlgorithmId::new(profile.signing_algorithm.clone())
        .map_err(|_| SyncLifecycleError::InvalidSnapshot)?;
    DeviceSigningPublicKey::new(
        profile.device_id.clone(),
        profile.signing_public_key_id.clone(),
        algorithm,
        profile.signing_public_key.clone(),
        created_at_ms,
        revoked_at_ms,
    )
    .map_err(|_| SyncLifecycleError::InvalidSnapshot)
}

fn device_signature_from_authorization(
    authorization: &RemoteDeviceAuthorization,
) -> Result<DeviceSignature, SyncLifecycleError> {
    let algorithm = SignatureAlgorithmId::new(authorization.signature_algorithm.clone())
        .map_err(|_| SyncLifecycleError::InvalidAuthorization)?;
    let signature = DeviceSignature {
        signature_schema_version: authorization.signature_schema_version,
        signature_algorithm: algorithm,
        signature_key_id: authorization.signature_key_id.clone(),
        signer_device_id: authorization.authorizer_device_id.clone(),
        signature: authorization.signature.clone(),
    };
    signature
        .validate()
        .map_err(|_| SyncLifecycleError::InvalidAuthorization)?;
    Ok(signature)
}

fn signed_revocation(
    revocation: &RemoteDeviceRevocation,
) -> Result<SignedDeviceRevocation, SyncLifecycleError> {
    let algorithm = SignatureAlgorithmId::new(revocation.signature_algorithm.clone())
        .map_err(|_| SyncLifecycleError::InvalidRevocation)?;
    let signature = DeviceSignature {
        signature_schema_version: revocation.signature_schema_version,
        signature_algorithm: algorithm,
        signature_key_id: revocation.signature_key_id.clone(),
        signer_device_id: revocation.revoker_device_id.clone(),
        signature: revocation.signature.clone(),
    };
    let signed = SignedDeviceRevocation {
        signature,
        revoked_by_device_id: revocation.revoker_device_id.clone(),
        revoked_device_id: revocation.revoked_device_id.clone(),
        previous_key_epoch: revocation.previous_key_epoch,
        new_key_epoch: revocation.new_key_epoch,
        reason: revocation.reason.clone(),
        revoked_at_ms: revocation.created_at_ms,
    };
    signed
        .validate()
        .map_err(|_| SyncLifecycleError::InvalidRevocation)?;
    Ok(signed)
}

pub fn device_join_profile_challenge(
    domain_id: &str,
    join_request_id: &str,
    device: &RemoteLifecycleDevice,
    created_at_ms: i64,
    expires_at_ms: i64,
) -> String {
    let canonical = canonical_signature_bytes(
        "device_join_profile",
        &[
            SignatureField::text("domain_id", domain_id),
            SignatureField::text("join_request_id", join_request_id),
            SignatureField::text("device_id", &device.device_id),
            SignatureField::text("signing_algorithm", &device.signing_algorithm),
            SignatureField::text("signing_public_key_id", &device.signing_public_key_id),
            SignatureField::bytes("signing_public_key", &device.signing_public_key),
            SignatureField::text(
                "key_agreement_public_key_id",
                &device.key_agreement_public_key_id,
            ),
            SignatureField::bytes("key_agreement_public_key", &device.key_agreement_public_key),
            SignatureField::i64("created_at_ms", created_at_ms),
            SignatureField::i64("expires_at_ms", expires_at_ms),
        ],
    );
    let digest = Sha256::digest(canonical);
    let mut challenge =
        String::with_capacity(JOIN_PROFILE_CHALLENGE_PREFIX.len() + digest.len() * 2);
    challenge.push_str(JOIN_PROFILE_CHALLENGE_PREFIX);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(challenge, "{byte:02x}");
    }
    challenge
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RemoteDeviceAuthorization, RemoteDeviceRevocation, SyncDomain};
    use radishlex_ime_crypto::{
        DeviceSigningKeyHandle, TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN,
    };

    #[test]
    fn signed_lifecycle_builds_active_and_revoked_profiles() {
        let (snapshot, anchor) = lifecycle_fixture();
        let verified = verify_lifecycle_snapshot(snapshot, &anchor).expect("verified lifecycle");
        assert_eq!(verified.trusted_domain().domain().current_key_epoch, 2);
        assert_eq!(
            verified
                .trusted_domain()
                .device_profile("device-b")
                .expect("device b")
                .reject_from_change_sequence(),
            Some(7)
        );
    }

    #[test]
    fn lifecycle_rejects_public_key_substitution_and_sequence_gap() {
        let (mut snapshot, anchor) = lifecycle_fixture();
        snapshot.entries[1].device.signing_public_key[0] ^= 0x01;
        assert_eq!(
            verify_lifecycle_snapshot(snapshot, &anchor),
            Err(SyncLifecycleError::InvalidAuthorization)
        );

        let (mut snapshot, anchor) = lifecycle_fixture();
        snapshot.entries[2].lifecycle_sequence = 4;
        assert_eq!(
            verify_lifecycle_snapshot(snapshot, &anchor),
            Err(SyncLifecycleError::InvalidSequence)
        );
    }

    fn lifecycle_fixture() -> (RemoteLifecycleSnapshot, DeviceSigningPublicKey) {
        let mut store = TestMemoryDeviceKeyStore::new();
        let anchor = store
            .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
            .expect("anchor");
        let recipient_key = store
            .insert_signing_key("device-b", "signing-key-b", [8u8; 32], 20)
            .expect("recipient key");
        let handle = store
            .handle("device-a", "signing-key-a")
            .expect("signing handle");
        let initial_device = remote_device(
            "device-a",
            "signing-key-a",
            anchor.public_key.clone(),
            SyncDeviceStatus::Active,
            10,
            None,
        );
        let recipient_device = remote_device(
            "device-b",
            "signing-key-b",
            recipient_key.public_key.clone(),
            SyncDeviceStatus::Active,
            30,
            None,
        );
        let challenge =
            device_join_profile_challenge("domain-a", "join-b", &recipient_device, 20, 50);
        let authorization = signed_authorization(&store, &handle, &challenge);
        let revoked_device = remote_device(
            "device-b",
            "signing-key-b",
            recipient_key.public_key,
            SyncDeviceStatus::Lost,
            30,
            Some(40),
        );
        let revocation = signed_revocation_fixture(&store, &handle);
        (
            RemoteLifecycleSnapshot {
                domain: SyncDomain::new("domain-a", 2, "sync-key-2", 10, 40).expect("domain"),
                entries: vec![
                    RemoteLifecycleEvent {
                        domain_id: "domain-a".to_owned(),
                        lifecycle_sequence: 1,
                        event_type: RemoteLifecycleEventKind::InitialDevice,
                        record_id: "device-a".to_owned(),
                        key_epoch: 1,
                        reject_from_object_change_sequence: None,
                        created_at_ms: 10,
                        device: initial_device,
                        authorization: None,
                        revocation: None,
                    },
                    RemoteLifecycleEvent {
                        domain_id: "domain-a".to_owned(),
                        lifecycle_sequence: 2,
                        event_type: RemoteLifecycleEventKind::DeviceAuthorized,
                        record_id: "join-b".to_owned(),
                        key_epoch: 1,
                        reject_from_object_change_sequence: None,
                        created_at_ms: 30,
                        device: recipient_device,
                        authorization: Some(authorization),
                        revocation: None,
                    },
                    RemoteLifecycleEvent {
                        domain_id: "domain-a".to_owned(),
                        lifecycle_sequence: 3,
                        event_type: RemoteLifecycleEventKind::DeviceRevoked,
                        record_id: "device-b:2".to_owned(),
                        key_epoch: 2,
                        reject_from_object_change_sequence: Some(7),
                        created_at_ms: 40,
                        device: revoked_device,
                        authorization: None,
                        revocation: Some(revocation),
                    },
                ],
                next_cursor: OpaqueSyncCursor::new("lifecycle-cursor-3").expect("cursor"),
            },
            anchor,
        )
    }

    fn remote_device(
        device_id: &str,
        key_id: &str,
        public_key: Vec<u8>,
        status: SyncDeviceStatus,
        authorized_at_ms: i64,
        revoked_at_ms: Option<i64>,
    ) -> RemoteLifecycleDevice {
        RemoteLifecycleDevice {
            domain_id: "domain-a".to_owned(),
            device_id: device_id.to_owned(),
            signing_algorithm: "ed25519-v1".to_owned(),
            signing_public_key_id: key_id.to_owned(),
            signing_public_key: public_key,
            key_agreement_public_key_id: format!("agreement-{device_id}"),
            key_agreement_public_key: agreement_public_key(1),
            status,
            authorized_at_ms: Some(authorized_at_ms),
            revoked_at_ms,
            last_seen_at_ms: None,
        }
    }

    fn agreement_public_key(scalar: u8) -> Vec<u8> {
        match scalar {
            1 => vec![
                0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
                0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
                0x45, 0xd8, 0x98, 0xc2, 0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e,
                0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e,
                0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
            ],
            _ => unreachable!("test scalar"),
        }
    }

    fn signed_authorization(
        store: &TestMemoryDeviceKeyStore,
        handle: &DeviceSigningKeyHandle,
        challenge: &str,
    ) -> RemoteDeviceAuthorization {
        let mut signed = SignedDeviceAuthorization {
            signature: placeholder_signature(),
            authorizer_device_id: "device-a".to_owned(),
            recipient_device_id: "device-b".to_owned(),
            recipient_public_key_id: "signing-key-b".to_owned(),
            join_challenge: challenge.to_owned(),
            join_short_code: "123456".to_owned(),
            key_epoch: 1,
            wrapping_key_id: "wrapping-key-b".to_owned(),
            encrypted_key_len: 32,
            created_at_ms: 30,
        };
        signed.signature = store
            .sign(handle, &signed.canonical_bytes())
            .expect("authorization signature");
        RemoteDeviceAuthorization {
            domain_id: "domain-a".to_owned(),
            join_request_id: "join-b".to_owned(),
            authorizer_device_id: signed.authorizer_device_id,
            recipient_device_id: signed.recipient_device_id,
            recipient_signing_public_key_id: signed.recipient_public_key_id,
            recipient_key_agreement_key_id: "agreement-device-b".to_owned(),
            join_short_code: signed.join_short_code,
            join_challenge: signed.join_challenge,
            join_created_at_ms: 20,
            join_expires_at_ms: 50,
            key_epoch: signed.key_epoch,
            wrapping_key_id: signed.wrapping_key_id,
            encrypted_key_len: signed.encrypted_key_len,
            created_at_ms: signed.created_at_ms,
            signature_schema_version: signed.signature.signature_schema_version,
            signature_algorithm: signed.signature.signature_algorithm.as_str().to_owned(),
            signature_key_id: signed.signature.signature_key_id,
            signature: signed.signature.signature,
        }
    }

    fn signed_revocation_fixture(
        store: &TestMemoryDeviceKeyStore,
        handle: &DeviceSigningKeyHandle,
    ) -> RemoteDeviceRevocation {
        let mut signed = SignedDeviceRevocation {
            signature: placeholder_signature(),
            revoked_by_device_id: "device-a".to_owned(),
            revoked_device_id: "device-b".to_owned(),
            previous_key_epoch: 1,
            new_key_epoch: 2,
            reason: "device_lost".to_owned(),
            revoked_at_ms: 40,
        };
        signed.signature = store
            .sign(handle, &signed.canonical_bytes())
            .expect("revocation signature");
        RemoteDeviceRevocation {
            domain_id: "domain-a".to_owned(),
            revoked_device_id: signed.revoked_device_id,
            revoker_device_id: signed.revoked_by_device_id,
            previous_key_epoch: signed.previous_key_epoch,
            new_key_epoch: signed.new_key_epoch,
            reason: signed.reason,
            created_at_ms: signed.revoked_at_ms,
            signature_schema_version: signed.signature.signature_schema_version,
            signature_algorithm: signed.signature.signature_algorithm.as_str().to_owned(),
            signature_key_id: signed.signature.signature_key_id,
            signature: signed.signature.signature,
        }
    }

    fn placeholder_signature() -> DeviceSignature {
        DeviceSignature::new("signing-key-a", "device-a", vec![1; ED25519_SIGNATURE_LEN])
            .expect("placeholder")
    }
}
