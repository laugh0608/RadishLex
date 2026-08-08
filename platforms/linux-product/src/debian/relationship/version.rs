use std::cmp::Ordering;

use crate::model::{ArtifactVersionRelation, LinuxOperationKind};

use super::{
    is_canonical_positive_decimal, DebianRelationshipError, DebianRelationshipErrorCode,
    VerifiedArtifactRelationship,
};

pub fn validate_operation_relation(
    operation: LinuxOperationKind,
    source: Option<&VerifiedArtifactRelationship>,
    target: Option<&VerifiedArtifactRelationship>,
) -> Result<ArtifactVersionRelation, DebianRelationshipError> {
    match (operation, source, target) {
        (LinuxOperationKind::Install, None, Some(_))
        | (LinuxOperationKind::Remove, Some(_), None) => Ok(ArtifactVersionRelation::NotApplicable),
        (LinuxOperationKind::Repair, Some(source), Some(target)) if source == target => {
            Ok(ArtifactVersionRelation::SameRelease)
        }
        (LinuxOperationKind::Upgrade, Some(source), Some(target)) => {
            require_same_product(source, target)?;
            if compare_debian_versions(target.package_version(), source.package_version())?
                == Ordering::Greater
            {
                Ok(ArtifactVersionRelation::TargetNewer)
            } else {
                Err(version_relation_error())
            }
        }
        (LinuxOperationKind::Rollback, Some(source), Some(target)) => {
            require_same_product(source, target)?;
            if compare_debian_versions(target.package_version(), source.package_version())?
                == Ordering::Less
            {
                Ok(ArtifactVersionRelation::TargetOlder)
            } else {
                Err(version_relation_error())
            }
        }
        _ => Err(version_relation_error()),
    }
}

fn require_same_product(
    source: &VerifiedArtifactRelationship,
    target: &VerifiedArtifactRelationship,
) -> Result<(), DebianRelationshipError> {
    if source.package_name() != target.package_name()
        || source.architecture() != target.architecture()
        || source.data_contract() != target.data_contract()
    {
        return Err(version_relation_error());
    }
    Ok(())
}

fn version_relation_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::VersionRelationInvalid,
        "source and target versions do not match the requested operation",
    )
}

pub fn compare_debian_versions(
    left: &str,
    right: &str,
) -> Result<Ordering, DebianRelationshipError> {
    let left = parse_debian_version(left)?;
    let right = parse_debian_version(right)?;
    let epoch = compare_decimal(left.epoch, right.epoch);
    if epoch != Ordering::Equal {
        return Ok(epoch);
    }
    let upstream = compare_version_part(left.upstream, right.upstream);
    if upstream != Ordering::Equal {
        return Ok(upstream);
    }
    Ok(compare_version_part(left.revision, right.revision))
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ParsedDebianVersion<'a> {
    epoch: &'a str,
    upstream: &'a str,
    revision: &'a str,
}

pub(super) fn parse_debian_version(
    value: &str,
) -> Result<ParsedDebianVersion<'_>, DebianRelationshipError> {
    if value.is_empty()
        || value.len() > 256
        || !value.is_ascii()
        || value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b':' | b'~' | b'-'))
        })
    {
        return Err(version_error());
    }
    let (epoch, remainder) = match value.split_once(':') {
        Some((epoch, remainder))
            if !epoch.is_empty()
                && epoch.bytes().all(|byte| byte.is_ascii_digit())
                && !remainder.is_empty() =>
        {
            (epoch, remainder)
        }
        Some(_) => return Err(version_error()),
        None => ("0", value),
    };
    if remainder.contains(':') || !remainder.as_bytes()[0].is_ascii_digit() {
        return Err(version_error());
    }
    let (upstream, revision) = match remainder.rsplit_once('-') {
        Some((upstream, revision)) if !upstream.is_empty() && !revision.is_empty() => {
            (upstream, revision)
        }
        Some(_) => return Err(version_error()),
        None => (remainder, "0"),
    };
    Ok(ParsedDebianVersion {
        epoch,
        upstream,
        revision,
    })
}

pub(super) fn validate_radishlex_package_version(
    value: &str,
) -> Result<(), DebianRelationshipError> {
    parse_debian_version(value)?;
    let Some((upstream, revision)) = value.rsplit_once('-') else {
        return Err(version_error());
    };
    let Some((product_version, build)) = upstream.rsplit_once('+') else {
        return Err(version_error());
    };
    let parts: Vec<_> = product_version.split('.').collect();
    if parts.len() != 3
        || parts[0].len() != 2
        || !parts[0].bytes().all(|byte| byte.is_ascii_digit())
        || !is_canonical_positive_decimal(parts[1])
        || parts[1].parse::<u8>().map_or(true, |month| month > 12)
        || !is_canonical_positive_decimal(parts[2])
        || !is_canonical_positive_decimal(build)
        || !is_canonical_positive_decimal(revision)
    {
        return Err(version_error());
    }
    Ok(())
}

fn version_error() -> DebianRelationshipError {
    DebianRelationshipError::new(
        DebianRelationshipErrorCode::VersionInvalid,
        "Debian version is invalid",
    )
}

fn compare_decimal(left: &str, right: &str) -> Ordering {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    let left = if left.is_empty() { "0" } else { left };
    let right = if right.is_empty() { "0" } else { right };
    left.len()
        .cmp(&right.len())
        .then_with(|| left.as_bytes().cmp(right.as_bytes()))
}

fn compare_version_part(left: &str, right: &str) -> Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() || right_index < right.len() {
        while left
            .get(left_index)
            .is_some_and(|byte| !byte.is_ascii_digit())
            || right
                .get(right_index)
                .is_some_and(|byte| !byte.is_ascii_digit())
        {
            let left_order = version_character_order(left.get(left_index).copied());
            let right_order = version_character_order(right.get(right_index).copied());
            if left_order != right_order {
                return left_order.cmp(&right_order);
            }
            if left_index < left.len() {
                left_index += 1;
            }
            if right_index < right.len() {
                right_index += 1;
            }
        }

        while left.get(left_index) == Some(&b'0') {
            left_index += 1;
        }
        while right.get(right_index) == Some(&b'0') {
            right_index += 1;
        }
        let left_end = digit_run_end(left, left_index);
        let right_end = digit_run_end(right, right_index);
        let length_order = (left_end - left_index).cmp(&(right_end - right_index));
        if length_order != Ordering::Equal {
            return length_order;
        }
        let digit_order = left[left_index..left_end].cmp(&right[right_index..right_end]);
        if digit_order != Ordering::Equal {
            return digit_order;
        }
        left_index = left_end;
        right_index = right_end;
    }
    Ordering::Equal
}

fn digit_run_end(value: &[u8], mut index: usize) -> usize {
    while value.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    index
}

fn version_character_order(value: Option<u8>) -> i32 {
    match value {
        Some(b'~') => -1,
        None => 0,
        Some(byte) if byte.is_ascii_digit() => 0,
        Some(byte) if byte.is_ascii_alphabetic() => i32::from(byte),
        Some(byte) => i32::from(byte) + 256,
    }
}
