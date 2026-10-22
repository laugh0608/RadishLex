//! Fixed budgets for the synthetic-only observer; these are not OS quotas.

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

pub(super) const MIB: u64 = 1024 * 1024;
pub(super) const MAX_READ: u64 = 3 * 1024 * MIB;
pub(super) const MAX_WRITE: u64 = 256 * MIB;
pub(super) const MAX_SCRATCH: u64 = 272 * MIB;
pub(super) const MAX_BATCH: u64 = 544 * MIB;
pub(super) const MIN_FREE: u64 = 1024 * MIB;
pub(super) const BLOCK_SIZE: usize = 64 * 1024;
static NEXT_CALL: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BudgetError {
    LimitExceeded,
    Cancelled,
    Deadline,
    EntryTooShort,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Capture,
    Sqlite,
    Verify,
    Return,
    Cleanup,
}

/// A process-local call identity. Clones share revocation, never renew the call.
#[derive(Clone)]
pub(super) struct ObservationControl {
    call_id: u64,
    revoked: Arc<AtomicBool>,
    work_end: Instant,
    end: Instant,
}

impl ObservationControl {
    pub(super) fn enter(task_end: Instant) -> Result<Self, BudgetError> {
        Self::enter_at(Instant::now(), task_end)
    }

    fn enter_at(now: Instant, task_end: Instant) -> Result<Self, BudgetError> {
        let remaining = task_end
            .checked_duration_since(now)
            .ok_or(BudgetError::EntryTooShort)?;
        if remaining < Duration::from_secs(15) {
            return Err(BudgetError::EntryTooShort);
        }
        let end = task_end.min(
            now.checked_add(Duration::from_secs(120))
                .ok_or(BudgetError::Overflow)?,
        );
        let call_id = NEXT_CALL
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| BudgetError::Overflow)?;
        Ok(Self {
            call_id,
            revoked: Arc::new(AtomicBool::new(false)),
            work_end: end - Duration::from_secs(10),
            end,
        })
    }

    pub(super) fn call_id(&self) -> u64 {
        self.call_id
    }

    pub(super) fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }

    pub(super) fn check_work(&self) -> Result<(), BudgetError> {
        self.check_at(Instant::now(), false)
    }

    fn check_at(&self, now: Instant, cleanup: bool) -> Result<(), BudgetError> {
        if cleanup {
            return if now < self.end {
                Ok(())
            } else {
                Err(BudgetError::Deadline)
            };
        }
        if self.revoked.load(Ordering::Acquire) {
            return Err(BudgetError::Cancelled);
        }
        if now >= self.work_end {
            return Err(BudgetError::Deadline);
        }
        Ok(())
    }
}

pub(super) struct ObserverBudget {
    control: ObservationControl,
    phase: Phase,
    read: u64,
    written: u64,
}

impl ObserverBudget {
    pub(super) fn new(control: ObservationControl) -> Self {
        Self {
            control,
            phase: Phase::Capture,
            read: 0,
            written: 0,
        }
    }

    pub(super) fn control(&self) -> ObservationControl {
        self.control.clone()
    }

    pub(super) fn counters(&self) -> (u64, u64) {
        (self.read, self.written)
    }

    pub(super) fn check_io(&self) -> Result<(), BudgetError> {
        self.control
            .check_at(Instant::now(), self.phase == Phase::Cleanup)
    }

    pub(super) fn checkpoint(&mut self, phase: Phase) -> Result<(), BudgetError> {
        self.control
            .check_at(Instant::now(), phase == Phase::Cleanup)?;
        self.phase = phase;
        Ok(())
    }

    pub(super) fn begin_cleanup(&mut self) -> Result<(), BudgetError> {
        self.checkpoint(Phase::Cleanup)
    }

    pub(super) fn charge_read(&mut self, bytes: u64) -> Result<(), BudgetError> {
        self.control
            .check_at(Instant::now(), self.phase == Phase::Cleanup)?;
        charge(&mut self.read, bytes, MAX_READ)
    }

    pub(super) fn charge_write(&mut self, bytes: u64) -> Result<(), BudgetError> {
        self.control.check_work()?;
        if self.phase == Phase::Cleanup {
            return Err(BudgetError::Cancelled);
        }
        charge(&mut self.written, bytes, MAX_WRITE)
    }

    pub(super) fn admit_family(&self, lengths: [Option<u64>; 4]) -> Result<(), BudgetError> {
        self.check_io()?;
        family_lengths(lengths)
    }
}

fn charge(total: &mut u64, bytes: u64, maximum: u64) -> Result<(), BudgetError> {
    let next = total.checked_add(bytes).ok_or(BudgetError::Overflow)?;
    if next > maximum {
        return Err(BudgetError::LimitExceeded);
    }
    *total = next;
    Ok(())
}

fn family_lengths(lengths: [Option<u64>; 4]) -> Result<(), BudgetError> {
    if lengths[0].is_none() || lengths[3].is_some() {
        return Err(BudgetError::LimitExceeded);
    }
    let mut total = 0u64;
    for (size, limit) in lengths[..3].iter().zip([256 * MIB, 128 * MIB, 8 * MIB]) {
        let bytes = size.unwrap_or(0);
        if bytes > limit {
            return Err(BudgetError::LimitExceeded);
        }
        total = total.checked_add(bytes).ok_or(BudgetError::Overflow)?;
    }
    if total > 256 * MIB {
        return Err(BudgetError::LimitExceeded);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_family_limits_and_combined_limit_are_inclusive() {
        for (index, limit) in [256 * MIB, 128 * MIB, 8 * MIB].into_iter().enumerate() {
            let mut sizes = [Some(0), None, None, None];
            sizes[index] = Some(limit);
            assert_eq!(family_lengths(sizes), Ok(()));
            sizes[index] = Some(limit + 1);
            assert_eq!(family_lengths(sizes), Err(BudgetError::LimitExceeded));
        }
        assert_eq!(
            family_lengths([Some(128 * MIB), Some(128 * MIB), None, None]),
            Ok(())
        );
        assert_eq!(
            family_lengths([Some(128 * MIB), Some(128 * MIB), Some(1), None]),
            Err(BudgetError::LimitExceeded)
        );
        assert!(family_lengths([None, None, None, None]).is_err());
        assert!(family_lengths([Some(1), None, None, Some(0)]).is_err());
        assert!(family_lengths([Some(u64::MAX), None, None, None]).is_err());
    }

    #[test]
    fn cumulative_counters_never_wrap_or_advance_on_rejection() {
        for limit in [MAX_READ, MAX_WRITE] {
            let mut total = 0;
            assert_eq!(charge(&mut total, limit, limit), Ok(()));
            assert_eq!(
                charge(&mut total, 1, limit),
                Err(BudgetError::LimitExceeded)
            );
            assert_eq!(total, limit);
        }
        let mut total = u64::MAX;
        assert_eq!(charge(&mut total, 1, u64::MAX), Err(BudgetError::Overflow));
        assert_eq!(total, u64::MAX);
    }

    #[test]
    fn deadlines_and_revocation_preserve_only_the_disposal_window() {
        let now = Instant::now();
        assert!(matches!(
            ObservationControl::enter_at(now, now + Duration::from_millis(14999)),
            Err(BudgetError::EntryTooShort)
        ));
        let control = ObservationControl::enter_at(now, now + Duration::from_secs(300)).unwrap();
        assert_eq!(control.work_end, now + Duration::from_secs(110));
        assert_eq!(control.end, now + Duration::from_secs(120));
        assert_eq!(
            control.check_at(control.work_end, false),
            Err(BudgetError::Deadline)
        );
        assert_eq!(control.check_at(control.work_end, true), Ok(()));
        assert_eq!(
            control.check_at(control.end, true),
            Err(BudgetError::Deadline)
        );
        control.revoke();
        assert_eq!(control.check_at(now, false), Err(BudgetError::Cancelled));
        assert_eq!(control.check_at(now, true), Ok(()));
        let minimum = ObservationControl::enter_at(now, now + Duration::from_secs(15)).unwrap();
        assert_ne!(control.call_id(), minimum.call_id());
    }
}
