use std::cmp::Ordering;

use radishlex_ime_core::Candidate;
use radishlex_ime_userdb::{RankerWeight, TermStatus, UserTerm};

use crate::model::{CandidateExplanation, RankRequest, RankedCandidate, RankerConfig};

#[derive(Debug, Clone, PartialEq)]
pub struct Ranker {
    config: RankerConfig,
}

impl Ranker {
    pub fn new(config: RankerConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(RankerConfig::default())
    }

    pub fn rank(&self, request: RankRequest) -> Vec<RankedCandidate> {
        let mut ranked = request
            .candidates
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, candidate)| {
                let explanation = self.explain_candidate(&request, index, &candidate);
                RankedCandidate {
                    candidate,
                    original_index: index,
                    final_score: explanation.final_score(),
                    explanation,
                }
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|left, right| {
            right
                .final_score
                .partial_cmp(&left.final_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.original_index.cmp(&right.original_index))
        });

        ranked
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

        let user_term_boost = if deleted {
            0.0
        } else {
            matching_terms
                .iter()
                .filter(|term| term.status == TermStatus::Active)
                .map(|term| term.weight.max(0.0) * self.config.user_term_weight)
                .sum()
        };

        let frequency_boost = if deleted {
            0.0
        } else {
            matching_weights
                .iter()
                .map(|weight| weight.frequency.max(0) as f64 * self.config.frequency_weight)
                .sum()
        };

        let recency_boost = if deleted {
            0.0
        } else {
            matching_weights
                .iter()
                .map(|weight| weight.recency_score.clamp(0.0, 1.0) * self.config.recency_weight)
                .sum()
        };

        let context_boost = if deleted {
            0.0
        } else if matching_weights
            .iter()
            .any(|weight| weight.context_kind == request.context_kind)
        {
            self.config.context_weight
        } else {
            0.0
        };

        let negative_feedback_penalty = matching_weights
            .iter()
            .map(|weight| weight.negative_score.max(0.0) * self.config.negative_feedback_weight)
            .sum();

        let suppressed_penalty = if matching_terms
            .iter()
            .any(|term| term.status == TermStatus::Suppressed)
        {
            self.config.suppressed_penalty
        } else {
            0.0
        };

        CandidateExplanation {
            engine_order_factor: -(original_index as f64) * self.config.engine_order_step,
            user_term_boost,
            frequency_boost,
            recency_boost,
            context_boost,
            negative_feedback_penalty,
            suppressed_penalty,
            deleted_penalty: if deleted {
                self.config.deleted_penalty
            } else {
                0.0
            },
        }
    }
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
        }
    }

    fn weight(text: &str, frequency: i64, recency_score: f64, negative_score: f64) -> RankerWeight {
        RankerWeight {
            input_code: "luobo".to_owned(),
            text: text.to_owned(),
            reading: None,
            frequency,
            recency_score,
            negative_score,
            context_kind: "chat".to_owned(),
        }
    }

    #[test]
    fn active_user_term_can_promote_later_candidate() {
        let request = RankRequest::new("luobo", vec![candidate("落泊"), candidate("萝卜")])
            .with_user_terms(vec![term("萝卜", 2.0, TermStatus::Active)]);

        let ranked = Ranker::default().rank(request);

        assert_eq!(ranked[0].candidate.text(), "萝卜");
        assert_eq!(ranked[0].original_index, 1);
        assert!(ranked[0].explanation.user_term_boost > 0.0);
    }

    #[test]
    fn frequency_recency_and_context_are_explained() {
        let request = RankRequest::new("luobo", vec![candidate("落泊"), candidate("萝卜")])
            .with_context_kind("chat")
            .with_ranker_weights(vec![weight("萝卜", 5, 0.8, 0.0)]);

        let ranked = Ranker::default().rank(request);

        assert_eq!(ranked[0].candidate.text(), "萝卜");
        assert!(ranked[0].explanation.frequency_boost > 0.0);
        assert!(ranked[0].explanation.recency_boost > 0.0);
        assert!(ranked[0].explanation.context_boost > 0.0);
    }

    #[test]
    fn negative_feedback_lowers_candidate() {
        let request = RankRequest::new("luobo", vec![candidate("萝卜"), candidate("落泊")])
            .with_ranker_weights(vec![weight("萝卜", 0, 0.0, 3.0)]);

        let ranked = Ranker::default().rank(request);

        assert_eq!(ranked[0].candidate.text(), "落泊");
        let penalized = ranked
            .iter()
            .find(|candidate| candidate.candidate.text() == "萝卜")
            .expect("penalized candidate");
        assert!(penalized.explanation.negative_feedback_penalty > 0.0);
    }

    #[test]
    fn suppressed_term_gets_penalty() {
        let request = RankRequest::new("luobo", vec![candidate("萝卜"), candidate("落泊")])
            .with_user_terms(vec![term("萝卜", 10.0, TermStatus::Suppressed)]);

        let ranked = Ranker::default().rank(request);

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
        let request = RankRequest::new("luobo", vec![candidate("落泊"), candidate("萝卜")])
            .with_user_terms(vec![term("萝卜", 20.0, TermStatus::Active)])
            .with_ranker_weights(vec![weight("萝卜", 20, 1.0, 0.0)])
            .with_deleted_terms(vec![DeletedTermSummary::new(
                "luobo",
                "萝卜",
                None::<String>,
            )]);

        let ranked = Ranker::default().rank(request);

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
        let request = RankRequest::new("luobo", vec![candidate("a"), candidate("b")]);

        let ranked = Ranker::default().rank(request);

        assert_eq!(ranked[0].candidate.text(), "a");
        assert_eq!(ranked[1].candidate.text(), "b");
    }
}
