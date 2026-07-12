use radishlex_ime_core::Candidate;
use radishlex_ime_ranker::{RankRequest, RankedCandidate, Ranker};
use radishlex_ime_userdb::UserDb;

use crate::error::FfiError;
use crate::snapshot::RadishLexStringView;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadishLexRankExplainView {
    pub input_code: RadishLexStringView,
    pub candidate_text: RadishLexStringView,
    pub reading: RadishLexStringView,
    pub reading_present: u8,
    pub context_kind: RadishLexStringView,
    pub original_index: usize,
    pub final_score: f64,
    pub engine_order_factor: f64,
    pub user_term_boost: f64,
    pub frequency_boost: f64,
    pub recency_boost: f64,
    pub context_boost: f64,
    pub negative_feedback_penalty: f64,
    pub suppressed_penalty: f64,
    pub deleted_penalty: f64,
}

impl RadishLexRankExplainView {
    pub const fn empty() -> Self {
        Self {
            input_code: RadishLexStringView::empty(),
            candidate_text: RadishLexStringView::empty(),
            reading: RadishLexStringView::empty(),
            reading_present: 0,
            context_kind: RadishLexStringView::empty(),
            original_index: 0,
            final_score: 0.0,
            engine_order_factor: 0.0,
            user_term_boost: 0.0,
            frequency_boost: 0.0,
            recency_boost: 0.0,
            context_boost: 0.0,
            negative_feedback_penalty: 0.0,
            suppressed_penalty: 0.0,
            deleted_penalty: 0.0,
        }
    }
}

pub struct RadishLexRankExplain {
    input_code: String,
    reading: Option<String>,
    context_kind: String,
    ranked: RankedCandidate,
}

impl RadishLexRankExplain {
    pub fn view(&self) -> RadishLexRankExplainView {
        let explanation = &self.ranked.explanation;
        RadishLexRankExplainView {
            input_code: RadishLexStringView::from_str(&self.input_code),
            candidate_text: RadishLexStringView::from_str(self.ranked.candidate.text()),
            reading: optional_view(self.reading.as_deref()),
            reading_present: u8::from(self.reading.is_some()),
            context_kind: RadishLexStringView::from_str(&self.context_kind),
            original_index: self.ranked.original_index,
            final_score: self.ranked.final_score,
            engine_order_factor: explanation.engine_order_factor,
            user_term_boost: explanation.user_term_boost,
            frequency_boost: explanation.frequency_boost,
            recency_boost: explanation.recency_boost,
            context_boost: explanation.context_boost,
            negative_feedback_penalty: explanation.negative_feedback_penalty,
            suppressed_penalty: explanation.suppressed_penalty,
            deleted_penalty: explanation.deleted_penalty,
        }
    }

    /// # Safety
    /// `explain` must be null or a live `RadishLexRankExplain` pointer released exactly once.
    pub unsafe fn free(explain: *mut Self) {
        if explain.is_null() {
            return;
        }

        let _ = Box::from_raw(explain);
    }
}

pub fn rank_explain_for_path(
    db_path: &str,
    input_code: &str,
    candidate_text: &str,
    reading: Option<&str>,
    context_kind: Option<&str>,
) -> Result<RadishLexRankExplain, FfiError> {
    let input_code = required_nonempty("input_code", input_code)?;
    let candidate_text = required_nonempty("candidate_text", candidate_text)?;
    let reading = normalized_optional(reading);
    let context_kind = normalized_context_kind(context_kind);

    let db = UserDb::open(db_path)?;
    let mut user_terms = Vec::new();
    if let Some(term) = db.fetch_term(
        &input_code,
        &candidate_text,
        reading.as_deref().unwrap_or(""),
    )? {
        user_terms.push(term);
    }

    let mut ranker_weights = Vec::new();
    if let Some(weight) = db.ranker_weight(
        &input_code,
        &candidate_text,
        reading.as_deref(),
        &context_kind,
    )? {
        ranker_weights.push(weight);
    }

    let mut candidate = Candidate::new(candidate_text.clone());
    if let Some(reading) = &reading {
        candidate = candidate.with_reading(reading.clone());
    }

    let ranked = Ranker::default()
        .rank(
            RankRequest::new(input_code.clone(), vec![candidate])
                .with_context_kind(context_kind.clone())
                .with_user_terms(user_terms)
                .with_ranker_weights(ranker_weights),
        )
        .into_iter()
        .next()
        .ok_or_else(|| FfiError::internal("ranker returned no candidates"))?;

    Ok(RadishLexRankExplain {
        input_code,
        reading,
        context_kind,
        ranked,
    })
}

fn optional_view(value: Option<&str>) -> RadishLexStringView {
    value.map_or_else(RadishLexStringView::empty, RadishLexStringView::from_str)
}

fn required_nonempty(field: &'static str, value: &str) -> Result<String, FfiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(FfiError::invalid_argument(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(value.to_owned())
}

fn normalized_optional(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_owned())
        }
    })
}

fn normalized_context_kind(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("general")
        .to_owned()
}
