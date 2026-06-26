use radishlex_ime_core::Candidate;
use radishlex_ime_userdb::{RankerWeight, UserTerm};

#[derive(Debug, Clone, PartialEq)]
pub struct RankerConfig {
    pub engine_order_step: f64,
    pub user_term_weight: f64,
    pub frequency_weight: f64,
    pub recency_weight: f64,
    pub context_weight: f64,
    pub negative_feedback_weight: f64,
    pub suppressed_penalty: f64,
    pub deleted_penalty: f64,
}

impl Default for RankerConfig {
    fn default() -> Self {
        Self {
            engine_order_step: 0.01,
            user_term_weight: 1.0,
            frequency_weight: 0.35,
            recency_weight: 0.25,
            context_weight: 0.3,
            negative_feedback_weight: 1.2,
            suppressed_penalty: 2.0,
            deleted_penalty: 10.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeletedTermSummary {
    pub input_code: String,
    pub text: String,
    pub reading: Option<String>,
}

impl DeletedTermSummary {
    pub fn new(
        input_code: impl Into<String>,
        text: impl Into<String>,
        reading: Option<impl Into<String>>,
    ) -> Self {
        Self {
            input_code: input_code.into(),
            text: text.into(),
            reading: reading.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankRequest {
    pub input_code: String,
    pub candidates: Vec<Candidate>,
    pub context_kind: String,
    pub user_terms: Vec<UserTerm>,
    pub ranker_weights: Vec<RankerWeight>,
    pub deleted_terms: Vec<DeletedTermSummary>,
}

impl RankRequest {
    pub fn new(input_code: impl Into<String>, candidates: Vec<Candidate>) -> Self {
        Self {
            input_code: input_code.into(),
            candidates,
            context_kind: "general".to_owned(),
            user_terms: Vec::new(),
            ranker_weights: Vec::new(),
            deleted_terms: Vec::new(),
        }
    }

    pub fn with_context_kind(mut self, context_kind: impl Into<String>) -> Self {
        self.context_kind = context_kind.into();
        self
    }

    pub fn with_user_terms(mut self, user_terms: Vec<UserTerm>) -> Self {
        self.user_terms = user_terms;
        self
    }

    pub fn with_ranker_weights(mut self, ranker_weights: Vec<RankerWeight>) -> Self {
        self.ranker_weights = ranker_weights;
        self
    }

    pub fn with_deleted_terms(mut self, deleted_terms: Vec<DeletedTermSummary>) -> Self {
        self.deleted_terms = deleted_terms;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateExplanation {
    pub engine_order_factor: f64,
    pub user_term_boost: f64,
    pub frequency_boost: f64,
    pub recency_boost: f64,
    pub context_boost: f64,
    pub negative_feedback_penalty: f64,
    pub suppressed_penalty: f64,
    pub deleted_penalty: f64,
}

impl CandidateExplanation {
    pub fn final_score(&self) -> f64 {
        self.engine_order_factor
            + self.user_term_boost
            + self.frequency_boost
            + self.recency_boost
            + self.context_boost
            - self.negative_feedback_penalty
            - self.suppressed_penalty
            - self.deleted_penalty
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub candidate: Candidate,
    pub original_index: usize,
    pub final_score: f64,
    pub explanation: CandidateExplanation,
}
