use radishlex_ime_core::{Candidate, CandidateSource};

use crate::error::{RimeEngineError, RimeEngineResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RimeCandidateView<'a> {
    pub text: &'a str,
    pub reading: Option<&'a str>,
    pub annotation: Option<&'a str>,
}

pub fn candidate_from_view(view: RimeCandidateView<'_>) -> RimeEngineResult<Candidate> {
    if view.text.is_empty() {
        return Err(RimeEngineError::EmptyCandidateText);
    }

    let mut candidate = Candidate::new(view.text).with_source(CandidateSource::Engine);
    if let Some(reading) = view.reading.filter(|value| !value.is_empty()) {
        candidate = candidate.with_reading(reading);
    }
    if let Some(annotation) = view.annotation.filter(|value| !value.is_empty()) {
        candidate = candidate.with_annotation(annotation);
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use radishlex_ime_core::CandidateSource;

    use super::{candidate_from_view, RimeCandidateView};
    use crate::RimeEngineError;

    #[test]
    fn converts_candidate_view_to_core_candidate() {
        let candidate = candidate_from_view(RimeCandidateView {
            text: "萝卜",
            reading: Some("luobo"),
            annotation: Some("demo"),
        })
        .expect("candidate conversion succeeds");

        assert_eq!(candidate.text(), "萝卜");
        assert_eq!(candidate.reading(), Some("luobo"));
        assert_eq!(candidate.annotation(), Some("demo"));
        assert_eq!(candidate.source(), CandidateSource::Engine);
    }

    #[test]
    fn rejects_empty_candidate_text() {
        let err = candidate_from_view(RimeCandidateView {
            text: "",
            reading: Some("empty"),
            annotation: None,
        })
        .expect_err("empty text must fail");

        assert_eq!(err, RimeEngineError::EmptyCandidateText);
    }
}
