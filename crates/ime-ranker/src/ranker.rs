use std::cmp::Ordering;

use radishlex_ime_core::Candidate;
use radishlex_ime_userdb::{RankerWeight, TermStatus, UserTerm};

use crate::error::RankerError;
use crate::model::{CandidateExplanation, RankRequest, RankedCandidate, RankerConfig};

#[derive(Debug, Clone, PartialEq)]
pub struct Ranker {
    config: RankerConfig,
}

impl Ranker {
    pub fn new(config: RankerConfig) -> Result<Self, RankerError> {
        validate_config(&config)?;
        Ok(Self { config })
    }

    pub fn with_default_config() -> Self {
        Self::new(RankerConfig::default()).expect("default ranker configuration must be valid")
    }

    pub fn rank(&self, request: RankRequest) -> Result<Vec<RankedCandidate>, RankerError> {
        validate_request(&request)?;
        let mut ranked = request
            .candidates
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, candidate)| {
                let explanation = self.explain_candidate(&request, index, &candidate);
                Ok(RankedCandidate {
                    candidate,
                    original_index: index,
                    final_score: explanation.final_score(),
                    explanation,
                })
            })
            .collect::<Result<Vec<_>, RankerError>>()?;

        ranked.sort_by(|left, right| {
            right
                .final_score
                .partial_cmp(&left.final_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.original_index.cmp(&right.original_index))
        });

        Ok(ranked)
    }

    fn explain_candidate(
        &self,
        request: &RankRequest,
        original_index: usize,
        candidate: &Candidate,
    ) -> CandidateExplanation {
        let deleted = is_deleted(request, candidate);
        let matching_terms = request
            .user_terms
            .iter()
            .filter(|term| term_matches_candidate(&request.input_code, term, candidate))
            .collect::<Vec<_>>();
        let matching_weights = request
            .ranker_weights
            .iter()
            .filter(|weight| weight_matches_candidate(&request.input_code, weight, candidate))
            .collect::<Vec<_>>();

        let suppressed = matching_terms
            .iter()
            .any(|term| term.status == TermStatus::Suppressed);
        let engine_order_factor = -(original_index as f64) * self.config.engine_order_step;

        if deleted {
            return CandidateExplanation {
                engine_order_factor,
                user_term_boost: 0.0,
                frequency_boost: 0.0,
                recency_boost: 0.0,
                context_boost: 0.0,
                negative_feedback_penalty: 0.0,
                suppressed_penalty: 0.0,
                deleted_penalty: self.config.deleted_penalty,
            };
        }

        let user_term_boost = if suppressed {
            0.0
        } else {
            matching_terms
                .iter()
                .filter(|term| term.status == TermStatus::Active)
                .map(|term| term.weight.min(self.config.max_user_term_weight))
                .fold(0.0, f64::max)
                * self.config.user_term_weight
        };

        let frequency_boost = if suppressed {
            0.0
        } else {
            let frequency = matching_weights
                .iter()
                .map(|weight| weight.frequency)
                .max()
                .unwrap_or(0) as f64;
            (frequency.ln_1p() * self.config.frequency_weight).min(self.config.max_frequency_boost)
        };

        let recency_boost = if suppressed {
            0.0
        } else {
            matching_weights
                .iter()
                .filter_map(|weight| weight.last_used_at_ms)
                .max()
                .map(|last_used_at_ms| {
                    recency_factor(
                        request.evaluated_at_ms,
                        last_used_at_ms,
                        self.config.recency_half_life_ms,
                    ) * self.config.recency_weight
                })
                .unwrap_or(0.0)
        };

        let context_boost = if !suppressed
            && matching_weights
                .iter()
                .any(|weight| weight.context_kind == request.context_kind && weight.frequency > 0)
        {
            self.config.context_weight
        } else {
            0.0
        };

        let negative_feedback_count = matching_weights
            .iter()
            .map(|weight| weight.negative_score)
            .fold(0.0, f64::max);
        let negative_feedback_penalty = (negative_feedback_count.ln_1p()
            * self.config.negative_feedback_weight)
            .min(self.config.max_negative_feedback_penalty);

        let suppressed_penalty = if suppressed {
            self.config.suppressed_penalty
        } else {
            0.0
        };

        CandidateExplanation {
            engine_order_factor,
            user_term_boost,
            frequency_boost,
            recency_boost,
            context_boost,
            negative_feedback_penalty,
            suppressed_penalty,
            deleted_penalty: 0.0,
        }
    }
}

fn validate_config(config: &RankerConfig) -> Result<(), RankerError> {
    let finite_non_negative = [
        ("engine_order_step", config.engine_order_step),
        ("user_term_weight", config.user_term_weight),
        ("max_user_term_weight", config.max_user_term_weight),
        ("frequency_weight", config.frequency_weight),
        ("max_frequency_boost", config.max_frequency_boost),
        ("recency_weight", config.recency_weight),
        ("context_weight", config.context_weight),
        ("negative_feedback_weight", config.negative_feedback_weight),
        (
            "max_negative_feedback_penalty",
            config.max_negative_feedback_penalty,
        ),
        ("suppressed_penalty", config.suppressed_penalty),
        ("deleted_penalty", config.deleted_penalty),
    ];
    for (field, value) in finite_non_negative {
        if !value.is_finite() || value < 0.0 {
            return Err(RankerError::invalid(
                field,
                "value must be finite and non-negative",
            ));
        }
    }
    if !config.recency_half_life_ms.is_finite() || config.recency_half_life_ms <= 0.0 {
        return Err(RankerError::invalid(
            "recency_half_life_ms",
            "value must be finite and greater than zero",
        ));
    }
    Ok(())
}

fn validate_request(request: &RankRequest) -> Result<(), RankerError> {
    if request.evaluated_at_ms < 0 {
        return Err(RankerError::invalid(
            "evaluated_at_ms",
            "value must be non-negative",
        ));
    }
    for term in &request.user_terms {
        if !term.weight.is_finite() || term.weight < 0.0 {
            return Err(RankerError::invalid(
                "user_term.weight",
                "value must be finite and non-negative",
            ));
        }
        if term.last_used_at_ms.is_some_and(|value| value < 0)
            || term.restored_at_ms.is_some_and(|value| value < 0)
        {
            return Err(RankerError::invalid(
                "user_term.timestamp",
                "value must be non-negative",
            ));
        }
    }
    for weight in &request.ranker_weights {
        if weight.frequency < 0 {
            return Err(RankerError::invalid(
                "ranker_weight.frequency",
                "value must be non-negative",
            ));
        }
        if weight.last_used_at_ms.is_some_and(|value| value < 0) {
            return Err(RankerError::invalid(
                "ranker_weight.last_used_at_ms",
                "value must be non-negative",
            ));
        }
        if !weight.negative_score.is_finite() || weight.negative_score < 0.0 {
            return Err(RankerError::invalid(
                "ranker_weight.negative_score",
                "value must be finite and non-negative",
            ));
        }
    }
    Ok(())
}

fn recency_factor(evaluated_at_ms: i64, last_used_at_ms: i64, half_life_ms: f64) -> f64 {
    let age_ms = evaluated_at_ms.saturating_sub(last_used_at_ms).max(0) as f64;
    2.0_f64.powf(-age_ms / half_life_ms)
}

impl Default for Ranker {
    fn default() -> Self {
        Self::with_default_config()
    }
}

fn is_deleted(request: &RankRequest, candidate: &Candidate) -> bool {
    request.deleted_terms.iter().any(|deleted| {
        deleted.input_code == request.input_code
            && deleted.text == candidate.text()
            && reading_matches(deleted.reading.as_deref(), candidate.reading())
    }) || request.user_terms.iter().any(|term| {
        term.status == TermStatus::Deleted
            && term_matches_candidate(&request.input_code, term, candidate)
    })
}

fn term_matches_candidate(input_code: &str, term: &UserTerm, candidate: &Candidate) -> bool {
    term.input_code == input_code
        && term.text == candidate.text()
        && reading_matches(term.reading.as_deref(), candidate.reading())
}

fn weight_matches_candidate(
    input_code: &str,
    weight: &RankerWeight,
    candidate: &Candidate,
) -> bool {
    weight.input_code == input_code
        && weight.text == candidate.text()
        && reading_matches(weight.reading.as_deref(), candidate.reading())
}

fn reading_matches(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (Some(""), None) | (None, Some("")) | (None, None) => true,
        (Some(_), None) | (None, Some(_)) => true,
    }
}

#[cfg(test)]
mod tests {
    use radishlex_ime_core::Candidate;
    use radishlex_ime_userdb::{RankerWeight, TermSource, TermStatus, UserTerm};

    use crate::{DeletedTermSummary, RankRequest, Ranker};

    fn candidate(text: &str) -> Candidate {
        Candidate::new(text)
    }

    fn term(text: &str, weight: f64, status: TermStatus) -> UserTerm {
        UserTerm {
            id: 1,
            text: text.to_owned(),
            reading: None,
            input_code: "luobo".to_owned(),
            source: TermSource::EngineSelection,
            weight,
            status,
            created_at_ms: 1,
            updated_at_ms: 1,
            last_used_at_ms: Some(1),
            restored_at_ms: None,
            import_batch_id: None,
        }
    }

    fn weight(
        text: &str,
        frequency: i64,
        last_used_at_ms: Option<i64>,
        negative_score: f64,
    ) -> RankerWeight {
        RankerWeight {
            input_code: "luobo".to_owned(),
            text: text.to_owned(),
            reading: None,
            frequency,
            last_used_at_ms,
            negative_score,
            context_kind: "chat".to_owned(),
        }
    }

    #[test]
    fn active_user_term_can_promote_later_candidate() {
        let request = RankRequest::new("luobo", vec![candidate("落泊"), candidate("萝卜")], 1_000)
            .with_user_terms(vec![term("萝卜", 2.0, TermStatus::Active)]);

        let ranked = Ranker::default().rank(request).expect("rank succeeds");

        assert_eq!(ranked[0].candidate.text(), "萝卜");
        assert_eq!(ranked[0].original_index, 1);
        assert!(ranked[0].explanation.user_term_boost > 0.0);
    }

    #[test]
    fn frequency_recency_and_context_are_explained() {
        let request = RankRequest::new("luobo", vec![candidate("落泊"), candidate("萝卜")], 1_000)
            .with_context_kind("chat")
            .with_ranker_weights(vec![weight("萝卜", 5, Some(800), 0.0)]);

        let ranked = Ranker::default().rank(request).expect("rank succeeds");

        assert_eq!(ranked[0].candidate.text(), "萝卜");
        assert!(ranked[0].explanation.frequency_boost > 0.0);
        assert!(ranked[0].explanation.recency_boost > 0.0);
        assert!(ranked[0].explanation.context_boost > 0.0);
    }

    #[test]
    fn negative_feedback_lowers_candidate() {
        let request = RankRequest::new("luobo", vec![candidate("萝卜"), candidate("落泊")], 1_000)
            .with_ranker_weights(vec![weight("萝卜", 0, None, 3.0)]);

        let ranked = Ranker::default().rank(request).expect("rank succeeds");

        assert_eq!(ranked[0].candidate.text(), "落泊");
        let penalized = ranked
            .iter()
            .find(|candidate| candidate.candidate.text() == "萝卜")
            .expect("penalized candidate");
        assert!(penalized.explanation.negative_feedback_penalty > 0.0);
    }

    #[test]
    fn suppressed_term_gets_penalty() {
        let request = RankRequest::new("luobo", vec![candidate("萝卜"), candidate("落泊")], 1_000)
            .with_user_terms(vec![term("萝卜", 10.0, TermStatus::Suppressed)]);

        let ranked = Ranker::default().rank(request).expect("rank succeeds");

        assert_eq!(ranked[0].candidate.text(), "落泊");
        let suppressed = ranked
            .iter()
            .find(|candidate| candidate.candidate.text() == "萝卜")
            .expect("suppressed candidate");
        assert!(suppressed.explanation.suppressed_penalty > 0.0);
        assert_eq!(suppressed.explanation.user_term_boost, 0.0);
    }

    #[test]
    fn deleted_tombstone_blocks_old_boosts() {
        let request = RankRequest::new("luobo", vec![candidate("落泊"), candidate("萝卜")], 1_000)
            .with_user_terms(vec![term("萝卜", 20.0, TermStatus::Active)])
            .with_ranker_weights(vec![weight("萝卜", 20, Some(1_000), 0.0)])
            .with_deleted_terms(vec![DeletedTermSummary::new(
                "luobo",
                "萝卜",
                None::<String>,
            )]);

        let ranked = Ranker::default().rank(request).expect("rank succeeds");

        assert_eq!(ranked[0].candidate.text(), "落泊");
        let deleted = ranked
            .iter()
            .find(|candidate| candidate.candidate.text() == "萝卜")
            .expect("deleted candidate");
        assert!(deleted.explanation.deleted_penalty > 0.0);
        assert_eq!(deleted.explanation.user_term_boost, 0.0);
        assert_eq!(deleted.explanation.frequency_boost, 0.0);
    }

    #[test]
    fn equal_scores_keep_engine_order() {
        let request = RankRequest::new("luobo", vec![candidate("a"), candidate("b")], 1_000);

        let ranked = Ranker::default().rank(request).expect("rank succeeds");

        assert_eq!(ranked[0].candidate.text(), "a");
        assert_eq!(ranked[1].candidate.text(), "b");
    }

    #[test]
    fn recency_uses_explicit_evaluation_time_and_decays_monotonically() {
        let half_life_ms = crate::RankerConfig::default().recency_half_life_ms as i64;
        let last_used_at_ms = 10_000;
        let boosts = [0, half_life_ms, half_life_ms * 2].map(|age_ms| {
            let request =
                RankRequest::new("luobo", vec![candidate("萝卜")], last_used_at_ms + age_ms)
                    .with_ranker_weights(vec![weight("萝卜", 1, Some(last_used_at_ms), 0.0)]);
            Ranker::default().rank(request).expect("rank succeeds")[0]
                .explanation
                .recency_boost
        });

        assert!(boosts[0] > boosts[1]);
        assert!(boosts[1] > boosts[2]);
        assert_approx_eq(boosts[1], boosts[0] / 2.0);
        assert_approx_eq(boosts[2], boosts[1] / 2.0);

        let future = RankRequest::new("luobo", vec![candidate("萝卜")], 5_000)
            .with_ranker_weights(vec![weight("萝卜", 1, Some(10_000), 0.0)]);
        let future_boost = Ranker::default()
            .rank(future)
            .expect("future timestamp ranks")[0]
            .explanation
            .recency_boost;
        assert_approx_eq(future_boost, boosts[0]);
    }

    #[test]
    fn frequency_and_negative_contributions_are_monotonic_bounded_and_finite() {
        let ranker = Ranker::default();
        let frequency_boosts = [1, 10, i64::MAX].map(|frequency| {
            ranker
                .rank(
                    RankRequest::new("luobo", vec![candidate("萝卜")], 1_000)
                        .with_ranker_weights(vec![weight("萝卜", frequency, None, 0.0)]),
                )
                .expect("frequency ranks")[0]
                .explanation
                .frequency_boost
        });
        assert!(frequency_boosts[0] < frequency_boosts[1]);
        assert!(frequency_boosts[1] <= frequency_boosts[2]);
        assert_eq!(
            frequency_boosts[2],
            crate::RankerConfig::default().max_frequency_boost
        );

        let ranked = ranker
            .rank(
                RankRequest::new("luobo", vec![candidate("萝卜")], 1_000)
                    .with_user_terms(vec![term("萝卜", f64::MAX, TermStatus::Active)])
                    .with_ranker_weights(vec![weight("萝卜", i64::MAX, Some(1_000), f64::MAX)]),
            )
            .expect("bounded extreme signals rank");
        let explanation = &ranked[0].explanation;
        assert!(ranked[0].final_score.is_finite());
        assert!(explanation.user_term_boost.is_finite());
        assert!(explanation.frequency_boost.is_finite());
        assert!(explanation.negative_feedback_penalty.is_finite());
        assert_eq!(
            explanation.negative_feedback_penalty,
            crate::RankerConfig::default().max_negative_feedback_penalty
        );
    }

    #[test]
    fn deleted_and_suppressed_states_disable_all_stale_positive_factors() {
        let stale_weight = weight("萝卜", 100, Some(1_000), 4.0);
        let deleted = Ranker::default()
            .rank(
                RankRequest::new("luobo", vec![candidate("萝卜")], 1_000)
                    .with_context_kind("chat")
                    .with_user_terms(vec![term("萝卜", 20.0, TermStatus::Suppressed)])
                    .with_ranker_weights(vec![stale_weight.clone()])
                    .with_deleted_terms(vec![DeletedTermSummary::new(
                        "luobo",
                        "萝卜",
                        None::<String>,
                    )]),
            )
            .expect("deleted state ranks");
        let deleted = &deleted[0].explanation;
        assert_eq!(deleted.user_term_boost, 0.0);
        assert_eq!(deleted.frequency_boost, 0.0);
        assert_eq!(deleted.recency_boost, 0.0);
        assert_eq!(deleted.context_boost, 0.0);
        assert_eq!(deleted.negative_feedback_penalty, 0.0);
        assert_eq!(deleted.suppressed_penalty, 0.0);
        assert!(deleted.deleted_penalty > 0.0);

        let suppressed = Ranker::default()
            .rank(
                RankRequest::new("luobo", vec![candidate("萝卜")], 1_000)
                    .with_context_kind("chat")
                    .with_user_terms(vec![term("萝卜", 20.0, TermStatus::Suppressed)])
                    .with_ranker_weights(vec![stale_weight]),
            )
            .expect("suppressed state ranks");
        let suppressed = &suppressed[0].explanation;
        assert_eq!(suppressed.user_term_boost, 0.0);
        assert_eq!(suppressed.frequency_boost, 0.0);
        assert_eq!(suppressed.recency_boost, 0.0);
        assert_eq!(suppressed.context_boost, 0.0);
        assert!(suppressed.negative_feedback_penalty > 0.0);
        assert!(suppressed.suppressed_penalty > 0.0);
        assert_eq!(suppressed.deleted_penalty, 0.0);
    }

    #[test]
    fn explanation_exactly_reconstructs_final_score() {
        let ranked = Ranker::default()
            .rank(
                RankRequest::new("luobo", vec![candidate("落泊"), candidate("萝卜")], 1_000)
                    .with_context_kind("chat")
                    .with_user_terms(vec![term("萝卜", 2.0, TermStatus::Active)])
                    .with_ranker_weights(vec![weight("萝卜", 7, Some(900), 1.0)]),
            )
            .expect("rank succeeds");

        for candidate in ranked {
            assert_approx_eq(candidate.final_score, candidate.explanation.final_score());
            assert!(candidate.final_score.is_finite());
        }
    }

    #[test]
    fn invalid_config_and_non_finite_request_signals_are_rejected() {
        let config = crate::RankerConfig {
            frequency_weight: f64::NAN,
            ..crate::RankerConfig::default()
        };
        let error = Ranker::new(config).expect_err("invalid config fails");
        assert_eq!(error.field(), "frequency_weight");

        let request = RankRequest::new("luobo", vec![candidate("萝卜")], 1_000)
            .with_user_terms(vec![term("萝卜", f64::INFINITY, TermStatus::Active)]);
        let error = Ranker::default()
            .rank(request)
            .expect_err("non-finite request fails");
        assert_eq!(error.field(), "user_term.weight");
    }

    fn assert_approx_eq(left: f64, right: f64) {
        assert!((left - right).abs() <= 1e-12, "left={left}, right={right}");
    }
}
