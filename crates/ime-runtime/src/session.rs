use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_core::{
    Candidate, Composition, Engine, InputSession, KeyEvent, KeyOutcome, SchemaId,
};
use radishlex_ime_ranker::{
    CandidateExplanation, DeletedTermSummary, RankRequest, RankedCandidate, Ranker,
};
use radishlex_ime_userdb::{PrivacyLevel, RankingCandidateIdentity, SelectionEventDraft, UserDb};

use crate::{LearningContext, PersonalizationPolicy, RuntimeError, RuntimeResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalizationStatus {
    Ready,
    PolicyBlocked,
    StorageUnavailable,
    ReadFailed,
    RankFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningDisposition {
    NotApplicable,
    Recorded,
    Deferred,
    SkippedByPolicy,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeCandidate {
    candidate: Candidate,
    display_index: usize,
    engine_index: usize,
    final_score: Option<f64>,
    explanation: Option<CandidateExplanation>,
}

impl RuntimeCandidate {
    pub fn candidate(&self) -> &Candidate {
        &self.candidate
    }

    pub fn display_index(&self) -> usize {
        self.display_index
    }

    pub fn engine_index(&self) -> usize {
        self.engine_index
    }

    pub fn final_score(&self) -> Option<f64> {
        self.final_score
    }

    pub fn explanation(&self) -> Option<&CandidateExplanation> {
        self.explanation.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSnapshot {
    composition: Composition,
    candidates: Vec<RuntimeCandidate>,
    schema: SchemaId,
    personalization_status: PersonalizationStatus,
}

impl RuntimeSnapshot {
    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    pub fn candidates(&self) -> &[RuntimeCandidate] {
        &self.candidates
    }

    pub fn schema(&self) -> &SchemaId {
        &self.schema
    }

    pub fn personalization_status(&self) -> PersonalizationStatus {
        self.personalization_status
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeEventResult {
    outcome: KeyOutcome,
    snapshot: RuntimeSnapshot,
    learning_disposition: LearningDisposition,
}

impl RuntimeEventResult {
    pub fn outcome(&self) -> &KeyOutcome {
        &self.outcome
    }

    pub fn snapshot(&self) -> &RuntimeSnapshot {
        &self.snapshot
    }

    pub fn learning_disposition(&self) -> LearningDisposition {
        self.learning_disposition
    }
}

#[derive(Debug, Clone)]
struct SelectionIntent {
    input_code: String,
    candidate: Candidate,
    display_index: usize,
    candidate_count: usize,
    context_kind: String,
}

pub struct PersonalizedInputSession<E> {
    input: InputSession<E>,
    userdb: Option<UserDb>,
    ranker: Ranker,
    context: LearningContext,
    session_id: String,
    display_candidates: Vec<RuntimeCandidate>,
    pending_selection: Option<SelectionIntent>,
    composition_active: bool,
    composition_policy: PersonalizationPolicy,
}

impl<E: Engine> PersonalizedInputSession<E> {
    pub fn open(
        engine: E,
        userdb_path: impl AsRef<Path>,
        session_id: impl Into<String>,
    ) -> RuntimeResult<Self> {
        let session_id = validated_session_id(session_id.into())?;
        let userdb = UserDb::open(userdb_path).ok();
        Self::from_parts(engine, userdb, session_id)
    }

    pub fn with_userdb(
        engine: E,
        userdb: UserDb,
        session_id: impl Into<String>,
    ) -> RuntimeResult<Self> {
        let session_id = validated_session_id(session_id.into())?;
        Self::from_parts(engine, Some(userdb), session_id)
    }

    fn from_parts(engine: E, userdb: Option<UserDb>, session_id: String) -> RuntimeResult<Self> {
        // A precomposed engine has no context provenance in this runtime.
        // Preserve its input, but never learn it as a fresh normal composition.
        let composition_active =
            !engine.composition()?.is_empty() || !engine.input_code()?.is_empty();
        Ok(Self {
            input: InputSession::new(engine),
            userdb,
            ranker: Ranker::with_default_config(),
            context: LearningContext::default(),
            session_id,
            display_candidates: Vec::new(),
            pending_selection: None,
            composition_active,
            composition_policy: if composition_active {
                PersonalizationPolicy::EngineOnly
            } else {
                PersonalizationPolicy::ReadWrite
            },
        })
    }

    pub fn set_learning_context(&mut self, context: LearningContext) {
        if self.context != context {
            if self.composition_active {
                self.composition_policy = self.composition_policy.restricted_by(context.policy());
            }
            self.pending_selection = None;
            self.display_candidates.clear();
            self.context = context;
        }
    }

    pub fn learning_context(&self) -> &LearningContext {
        &self.context
    }

    pub fn reset(&mut self) -> RuntimeResult<()> {
        self.pending_selection = None;
        self.display_candidates.clear();
        self.input.reset()?;
        self.refresh_composition_policy()?;
        Ok(())
    }

    pub fn set_schema(&mut self, schema: SchemaId) -> RuntimeResult<()> {
        self.pending_selection = None;
        self.display_candidates.clear();
        self.input.set_schema(schema)?;
        self.refresh_composition_policy()?;
        Ok(())
    }

    pub fn snapshot(&mut self) -> RuntimeResult<RuntimeSnapshot> {
        let state = self.input.state()?;
        let input_code = self.input.input_code()?;
        self.observe_composition(!state.composition().is_empty() || !input_code.is_empty());
        let policy = self.effective_policy();

        if policy == PersonalizationPolicy::EngineOnly {
            return Ok(self.engine_snapshot(
                state.composition().clone(),
                state.candidates().to_vec(),
                state.schema().clone(),
                PersonalizationStatus::PolicyBlocked,
            ));
        }

        if state.candidates().is_empty() || input_code.trim().is_empty() {
            let status = if self.userdb.is_some() {
                PersonalizationStatus::Ready
            } else {
                PersonalizationStatus::StorageUnavailable
            };
            return Ok(self.engine_snapshot(
                state.composition().clone(),
                state.candidates().to_vec(),
                state.schema().clone(),
                status,
            ));
        }

        let candidate_identities = state
            .candidates()
            .iter()
            .map(|candidate| RankingCandidateIdentity::new(candidate.text(), candidate.reading()))
            .collect::<Vec<_>>();
        let Some(userdb) = self.userdb.as_mut() else {
            return Ok(self.engine_snapshot(
                state.composition().clone(),
                state.candidates().to_vec(),
                state.schema().clone(),
                PersonalizationStatus::StorageUnavailable,
            ));
        };
        let signals = match userdb.ranking_signals(&input_code, &candidate_identities) {
            Ok(signals) => signals,
            Err(_) => {
                return Ok(self.engine_snapshot(
                    state.composition().clone(),
                    state.candidates().to_vec(),
                    state.schema().clone(),
                    PersonalizationStatus::ReadFailed,
                ));
            }
        };
        let deleted_terms = signals
            .deleted_terms
            .into_iter()
            .map(|term| DeletedTermSummary::new(term.input_code, term.text, term.reading))
            .collect();
        let request = RankRequest::new(input_code, state.candidates().to_vec(), now_ms()?)
            .with_context_kind(self.context.context_kind())
            .with_user_terms(signals.user_terms)
            .with_ranker_weights(signals.ranker_weights)
            .with_deleted_terms(deleted_terms);
        let ranked = match self.ranker.rank(request) {
            Ok(ranked) => ranked,
            Err(_) => {
                return Ok(self.engine_snapshot(
                    state.composition().clone(),
                    state.candidates().to_vec(),
                    state.schema().clone(),
                    PersonalizationStatus::RankFailed,
                ));
            }
        };

        Ok(self.ranked_snapshot(state.composition().clone(), ranked, state.schema().clone()))
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> RuntimeResult<RuntimeEventResult> {
        self.display_candidates.clear();
        self.restrict_composition_before_input();
        let outcome = self.input.push_key(key)?;
        let learning_disposition = if let Some(commit) = outcome.commit() {
            let intent = self.pending_selection.take();
            if self.effective_policy() != PersonalizationPolicy::ReadWrite {
                LearningDisposition::SkippedByPolicy
            } else {
                match intent {
                    Some(intent) if commit.text() == intent.candidate.text() => {
                        self.record_selection(&intent)
                    }
                    Some(_) | None => LearningDisposition::NotApplicable,
                }
            }
        } else {
            LearningDisposition::NotApplicable
        };
        let snapshot = self.snapshot()?;
        if outcome.commit().is_none() && snapshot.composition().is_empty() {
            self.pending_selection = None;
        }
        Ok(RuntimeEventResult {
            outcome,
            snapshot,
            learning_disposition,
        })
    }

    pub fn select_candidate(&mut self, display_index: usize) -> RuntimeResult<RuntimeEventResult> {
        if self.display_candidates.is_empty() {
            self.snapshot()?;
        }
        let candidate_count = self.display_candidates.len();
        let selected =
            self.display_candidates
                .get(display_index)
                .cloned()
                .ok_or(RuntimeError::Core(
                    radishlex_ime_core::CoreError::InvalidCandidateIndex {
                        index: display_index,
                        len: candidate_count,
                    },
                ))?;
        let intent = SelectionIntent {
            input_code: self.input.input_code()?,
            candidate: selected.candidate.clone(),
            display_index,
            candidate_count,
            context_kind: self.context.context_kind().to_owned(),
        };

        self.display_candidates.clear();
        self.restrict_composition_before_input();
        let outcome = self.input.select_candidate(selected.engine_index)?;
        let learning_disposition = if let Some(commit) = outcome.commit() {
            self.pending_selection = None;
            if self.effective_policy() != PersonalizationPolicy::ReadWrite {
                LearningDisposition::SkippedByPolicy
            } else if commit.text() == intent.candidate.text() {
                self.record_selection(&intent)
            } else {
                LearningDisposition::NotApplicable
            }
        } else if self.effective_policy() == PersonalizationPolicy::ReadWrite {
            self.pending_selection = Some(intent);
            LearningDisposition::Deferred
        } else {
            self.pending_selection = None;
            LearningDisposition::SkippedByPolicy
        };
        let snapshot = self.snapshot()?;
        Ok(RuntimeEventResult {
            outcome,
            snapshot,
            learning_disposition,
        })
    }

    pub fn engine(&self) -> &E {
        self.input.engine()
    }

    pub fn engine_mut(&mut self) -> &mut E {
        // Direct engine mutation cannot supply an input's privacy provenance.
        // Keep it blocked until an observed empty composition or explicit reset.
        self.composition_active = true;
        self.composition_policy = PersonalizationPolicy::EngineOnly;
        self.pending_selection = None;
        self.display_candidates.clear();
        self.input.engine_mut()
    }

    fn effective_policy(&self) -> PersonalizationPolicy {
        self.context.policy().restricted_by(self.composition_policy)
    }

    fn restrict_composition_before_input(&mut self) {
        // Mark before invoking the engine: failed calls may retain partial input.
        self.composition_active = true;
        self.composition_policy = self.composition_policy.restricted_by(self.context.policy());
    }

    fn observe_composition(&mut self, active: bool) {
        self.composition_active = active;
        if active {
            self.composition_policy = self.composition_policy.restricted_by(self.context.policy());
        } else {
            self.composition_policy = PersonalizationPolicy::ReadWrite;
            self.pending_selection = None;
        }
    }

    fn refresh_composition_policy(&mut self) -> RuntimeResult<()> {
        let active =
            !self.input.engine().composition()?.is_empty() || !self.input.input_code()?.is_empty();
        self.observe_composition(active);
        Ok(())
    }

    fn record_selection(&mut self, intent: &SelectionIntent) -> LearningDisposition {
        if self.effective_policy() != PersonalizationPolicy::ReadWrite {
            return LearningDisposition::SkippedByPolicy;
        }
        let Some(userdb) = self.userdb.as_mut() else {
            return LearningDisposition::Failed;
        };
        let mut event = SelectionEventDraft::new(
            &self.session_id,
            &intent.input_code,
            intent.candidate.text(),
            intent.display_index,
            intent.candidate_count,
        )
        .with_context_kind(&intent.context_kind)
        .with_privacy(PrivacyLevel::P1LocalOnly);
        if let Some(reading) = intent.candidate.reading() {
            event = event.with_reading(reading);
        }
        match userdb.record_selection(event) {
            Ok(Some(_)) => LearningDisposition::Recorded,
            Ok(None) => LearningDisposition::SkippedByPolicy,
            Err(_) => LearningDisposition::Failed,
        }
    }

    fn engine_snapshot(
        &mut self,
        composition: Composition,
        candidates: Vec<Candidate>,
        schema: SchemaId,
        status: PersonalizationStatus,
    ) -> RuntimeSnapshot {
        self.display_candidates = candidates
            .into_iter()
            .enumerate()
            .map(|(index, candidate)| RuntimeCandidate {
                candidate,
                display_index: index,
                engine_index: index,
                final_score: None,
                explanation: None,
            })
            .collect();
        RuntimeSnapshot {
            composition,
            candidates: self.display_candidates.clone(),
            schema,
            personalization_status: status,
        }
    }

    fn ranked_snapshot(
        &mut self,
        composition: Composition,
        ranked: Vec<RankedCandidate>,
        schema: SchemaId,
    ) -> RuntimeSnapshot {
        self.display_candidates = ranked
            .into_iter()
            .enumerate()
            .map(|(display_index, ranked)| RuntimeCandidate {
                candidate: ranked.candidate,
                display_index,
                engine_index: ranked.original_index,
                final_score: Some(ranked.final_score),
                explanation: Some(ranked.explanation),
            })
            .collect();
        RuntimeSnapshot {
            composition,
            candidates: self.display_candidates.clone(),
            schema,
            personalization_status: PersonalizationStatus::Ready,
        }
    }
}

fn validated_session_id(value: String) -> RuntimeResult<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        Err(RuntimeError::invalid_input(
            "session_id",
            "value cannot be empty",
        ))
    } else if value.len() > 128 {
        Err(RuntimeError::invalid_input(
            "session_id",
            "value exceeds 128 bytes",
        ))
    } else {
        Ok(value)
    }
}

fn now_ms() -> RuntimeResult<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RuntimeError::ClockFailure)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| RuntimeError::ClockFailure)
}
