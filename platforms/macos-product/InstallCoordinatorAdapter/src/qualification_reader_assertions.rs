//! Semantic assertion and normal-program chain checks, without I/O or authority.
use super::super::model::*;
use super::super::ReadError;
use radishlex_ime_product_install::ProgramComponent;

pub(super) fn assertion_supported(
    assertion: &Assertion,
    row: &CoverageRow,
    manifest: &Manifest,
) -> bool {
    let observations: Vec<_> = row
        .observations
        .iter()
        .filter(|o| assertion.observation_sequences.contains(&o.sequence))
        .collect();
    let sample = manifest
        .samples
        .iter()
        .find(|s| s.sample_id == row.sample_id);
    let matching =
        |predicate: &dyn Fn(&Event) -> bool| observations.iter().any(|o| predicate(&o.event));
    let component_count = |component, predicate: &dyn Fn(&Event) -> bool| {
        observations
            .iter()
            .filter(|o| {
                o.process.as_ref().is_some_and(|p| p.component == component) && predicate(&o.event)
            })
            .count() as u64
    };
    match (&assertion.kind, &assertion.actual) {
        (AssertionKind::InitialState, AssertionValue::State(value)) => {
            sample.is_some_and(|s| s.initial_state == *value)
                && matching(&|e| matches!(e, Event::Storage { state, .. } if state == value))
        }
        (AssertionKind::MaintenanceEquivalent, AssertionValue::State(value)) => {
            sample.is_some_and(|s| s.initial_state == *value)
                && final_storage_preserves(assertion, row, &|state| state == value)
        }
        (AssertionKind::NoNewDataReceipt, AssertionValue::Count(count)) => {
            *count == 0
                && matching(
                    &|e| matches!(e, Event::ReceiptState { new_data_receipts, .. } if new_data_receipts == count),
                )
        }
        (AssertionKind::OuterTerminal, AssertionValue::OuterState(value)) => {
            *value == row.scenario.outer_state
                && matching(&|e| matches!(e, Event::ReceiptState { outer, .. } if outer == value))
        }
        (AssertionKind::HistoryPreserved, AssertionValue::Digest(value)) => matching(
            &|e| matches!(e, Event::ReceiptState { history_sha256, .. } if history_sha256 == value),
        ),
        (AssertionKind::ProtectionSnapshotsPreserved, AssertionValue::Digest(value)) => matching(
            &|e| matches!(e, Event::ReceiptState { protection_snapshots_sha256, .. } if protection_snapshots_sha256 == value),
        ),
        (AssertionKind::RepeatedCancellation, AssertionValue::Count(value)) => {
            *value >= 2
                && row.scenario.repeated_cancellation
                && matching(
                    &|e| matches!(e, Event::ReceiptState { cancellation_count, .. } if cancellation_count == value),
                )
        }
        (AssertionKind::ManagerTwoGates, AssertionValue::Count(value)) => {
            *value == 2
                && component_count(ProgramComponent::Manager, &|e| {
                    matches!(
                        e,
                        Event::Gate {
                            outcome: Outcome::Allowed,
                            ..
                        }
                    )
                }) == *value
        }
        (AssertionKind::InputMethodTwoGates, AssertionValue::Count(value)) => {
            *value == 2
                && component_count(ProgramComponent::InputMethod, &|e| {
                    matches!(
                        e,
                        Event::Gate {
                            outcome: Outcome::Allowed,
                            ..
                        }
                    )
                }) == *value
        }
        (AssertionKind::ManagerRuntimeOpened, AssertionValue::Count(value)) => {
            *value > 0
                && component_count(ProgramComponent::Manager, &|e| {
                    matches!(e, Event::RuntimeOpened { .. })
                }) == *value
        }
        (AssertionKind::InputMethodRuntimeOpened, AssertionValue::Count(value)) => {
            *value > 0
                && component_count(ProgramComponent::InputMethod, &|e| {
                    matches!(e, Event::RuntimeOpened { .. })
                }) == *value
        }
        (AssertionKind::ClientSelection, AssertionValue::Count(value)) => {
            *value > 0
                && observations
                    .iter()
                    .filter(|o| matches!(o.event, Event::Selection { .. }))
                    .count() as u64
                    == *value
        }
        (AssertionKind::LearningPersisted, AssertionValue::State(value)) => {
            sample.is_some_and(|s| {
                s.initial_state.words_sha256 != value.words_sha256
                    && s.initial_state.schema == value.schema
            }) && matching(&|e| matches!(e, Event::Learned { state } if state == value))
                && matching(&|e| matches!(e, Event::Reopened { state } if state == value))
        }
        (AssertionKind::ReopenedSemantics, AssertionValue::State(value)) => {
            matching(&|e| matches!(e, Event::Reopened { state } if state == value))
        }
        (AssertionKind::WalPreserved, AssertionValue::Digest(value)) => {
            sample.is_some_and(|s| &s.initial_state.wal_sha256 == value)
                && final_storage_preserves(assertion, row, &|state| &state.wal_sha256 == value)
        }
        (AssertionKind::WordLearned, AssertionValue::Digest(value)) => {
            sample.is_some_and(|s| &s.initial_state.words_sha256 != value)
                && matching(
                    &|e| matches!(e, Event::Learned { state } if &state.words_sha256 == value),
                )
        }
        (AssertionKind::TombstonePreserved, AssertionValue::Digest(value)) => {
            sample.is_some_and(|s| &s.initial_state.tombstones_sha256 == value)
                && final_storage_preserves(assertion, row, &|state| {
                    &state.tombstones_sha256 == value
                })
        }
        (AssertionKind::SettingsPreserved, AssertionValue::Digest(value)) => {
            sample.is_some_and(|s| &s.initial_state.settings_sha256 == value)
                && final_storage_preserves(assertion, row, &|state| &state.settings_sha256 == value)
        }
        (AssertionKind::OfflineProjectionPreserved, AssertionValue::Digest(value)) => {
            sample.is_some_and(|s| &s.initial_state.offline_projection_sha256 == value)
                && final_storage_preserves(assertion, row, &|state| {
                    &state.offline_projection_sha256 == value
                })
        }
        (AssertionKind::GateDeniedBeforeWrite, AssertionValue::Outcome(Outcome::Rejected)) => {
            matching(&|e| {
                matches!(
                    e,
                    Event::Gate {
                        outcome: Outcome::Rejected,
                        ..
                    }
                )
            }) && !row
                .observations
                .iter()
                .any(|o| matches!(o.event, Event::RuntimeOpened { .. } | Event::Learned { .. }))
        }
        (AssertionKind::ReaderRejected, AssertionValue::Outcome(Outcome::Rejected)) => {
            matching(&|e| {
                matches!(
                    e,
                    Event::ReaderResult {
                        outcome: Outcome::Rejected,
                        ..
                    }
                )
            })
        }
        (kind, AssertionValue::EndState(value)) => {
            let checkpoint = match kind {
                AssertionKind::NextPreparation => TransactionCheckpoint::NextPreparation,
                AssertionKind::NewV1 => TransactionCheckpoint::NewV1,
                AssertionKind::DataTerminalRelease => TransactionCheckpoint::DataTerminalRelease,
                AssertionKind::Reauthorization => TransactionCheckpoint::Reauthorized,
                AssertionKind::CheckpointRevalidation => TransactionCheckpoint::Released,
                _ => return false,
            };
            matching(
                &|e| matches!(e, Event::Transaction { checkpoint: actual, state } if *actual == checkpoint && state == value),
            )
        }
        _ => false,
    }
}

// A postcondition is a separate measured stage, not the initial Storage repeated.
// All later storage measurements must agree for the asserted semantics, and the
// assertion must explicitly cite the final one. Intermediate green observations
// cannot hide a later contradictory state, even when the latter is not cited.
fn final_storage_preserves(
    assertion: &Assertion,
    row: &CoverageRow,
    preserves: &dyn Fn(&LogicalState) -> bool,
) -> bool {
    let Some(boundary) = row.observations.iter().find(|o| {
        matches!(
            o.event,
            Event::Transaction {
                checkpoint: TransactionCheckpoint::Postcondition,
                ..
            }
        )
    }) else {
        return false;
    };
    let mut final_storage_sequence = None;
    let mut final_state_sequence = None;
    for observation in row
        .observations
        .iter()
        .filter(|o| o.sequence > boundary.sequence)
    {
        let state = match &observation.event {
            Event::Storage { state, .. } => {
                final_storage_sequence = Some(observation.sequence);
                state
            }
            Event::Learned { state } | Event::Reopened { state } => state,
            _ => continue,
        };
        if !preserves(state) {
            return false;
        }
        final_state_sequence = Some(observation.sequence);
    }
    final_storage_sequence.is_some_and(|sequence| {
        final_state_sequence == Some(sequence)
            && assertion.observation_sequences.contains(&sequence)
    })
}

fn one_observation<'a>(
    observations: &[&'a Observation],
    predicate: impl Fn(&Event) -> bool,
) -> Result<&'a Observation, ReadError> {
    let mut matches = observations.iter().copied().filter(|o| predicate(&o.event));
    let observation = matches.next().ok_or(ReadError::AssertionFailed)?;
    if matches.next().is_some() {
        return Err(ReadError::ContradictoryRun);
    }
    Ok(observation)
}

pub(super) fn validate_normal_programs(row: &CoverageRow) -> Result<(), ReadError> {
    let initial = match &row.observations[0].event {
        Event::Storage { state, .. } => state,
        _ => return Err(ReadError::ContradictoryRun),
    };
    for component in [ProgramComponent::Manager, ProgramComponent::InputMethod] {
        let relevant: Vec<_> = row
            .observations
            .iter()
            .filter(|o| o.process.as_ref().is_some_and(|p| p.component == component))
            .collect();
        let starts: Vec<_> = relevant
            .iter()
            .copied()
            .filter(|o| matches!(o.event, Event::Started { .. }))
            .collect();
        if starts.len() != 2 {
            return Err(ReadError::AssertionFailed);
        }
        let first = starts[0]
            .process
            .as_ref()
            .ok_or(ReadError::ContradictoryRun)?;
        let second = starts[1]
            .process
            .as_ref()
            .ok_or(ReadError::ContradictoryRun)?;
        if first.start_id == second.start_id {
            return Err(ReadError::ContradictoryRun);
        }
        let mut learned_state = None;
        for process in [first, second] {
            let events: Vec<_> = relevant
                .iter()
                .copied()
                .filter(|o| {
                    o.process
                        .as_ref()
                        .is_some_and(|p| p.start_id == process.start_id)
                })
                .collect();
            let outer_gate = one_observation(&events, |event| {
                matches!(
                    event,
                    Event::Gate {
                        gate: Gate::OuterInstall,
                        outcome: Outcome::Allowed
                    }
                )
            })?;
            let data_gate = one_observation(&events, |event| {
                matches!(
                    event,
                    Event::Gate {
                        gate: Gate::DataUpgrade,
                        outcome: Outcome::Allowed
                    }
                )
            })?;
            let opened = one_observation(&events, |event| {
                matches!(event, Event::RuntimeOpened { .. })
            })?;
            let exit = one_observation(&events, |event| {
                matches!(event, Event::Exited { exit_code: 0 })
            })?;
            if !(process.start_sequence < outer_gate.sequence
                && outer_gate.sequence < opened.sequence
                && process.start_sequence < data_gate.sequence
                && data_gate.sequence < opened.sequence)
            {
                return Err(ReadError::ContradictoryRun);
            }
            if process.start_id == first.start_id {
                if events
                    .iter()
                    .any(|o| matches!(o.event, Event::Reopened { .. }))
                {
                    return Err(ReadError::ContradictoryRun);
                }
                let input =
                    one_observation(&events, |event| matches!(event, Event::ClientInput { .. }))?;
                let selection =
                    one_observation(&events, |event| matches!(event, Event::Selection { .. }))?;
                let learned =
                    one_observation(&events, |event| matches!(event, Event::Learned { .. }))?;
                if !(opened.sequence < input.sequence
                    && input.sequence < selection.sequence
                    && selection.sequence < learned.sequence
                    && learned.sequence < exit.sequence
                    && exit.sequence < second.start_sequence)
                {
                    return Err(ReadError::ContradictoryRun);
                }
                if let Event::Learned { state } = &learned.event {
                    if state.schema != initial.schema || state.words_sha256 == initial.words_sha256
                    {
                        return Err(ReadError::AssertionFailed);
                    }
                    learned_state = Some(state);
                }
            } else {
                if events.iter().any(|o| {
                    matches!(
                        o.event,
                        Event::ClientInput { .. } | Event::Selection { .. } | Event::Learned { .. }
                    )
                }) {
                    return Err(ReadError::ContradictoryRun);
                }
                let reopened =
                    one_observation(&events, |event| matches!(event, Event::Reopened { .. }))?;
                if !(opened.sequence < reopened.sequence && reopened.sequence < exit.sequence) {
                    return Err(ReadError::ContradictoryRun);
                }
                if let Event::Reopened { state } = &reopened.event {
                    if learned_state != Some(state) {
                        return Err(ReadError::AssertionFailed);
                    }
                }
            }
        }
    }
    Ok(())
}
