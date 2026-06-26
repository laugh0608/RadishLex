use radishlex_ime_core::{Candidate, CandidateSource, Composition};

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

#[cfg_attr(not(feature = "native-rime"), allow(dead_code))]
pub fn composition_from_parts(preedit: &str, cursor_pos: i32) -> RimeEngineResult<Composition> {
    let cursor = normalized_cursor(preedit, cursor_pos)?;
    Ok(Composition::new(preedit, cursor)?)
}

#[cfg_attr(not(feature = "native-rime"), allow(dead_code))]
fn normalized_cursor(preedit: &str, cursor_pos: i32) -> RimeEngineResult<usize> {
    if cursor_pos < 0 {
        return Ok(0);
    }

    let cursor = cursor_pos as usize;
    if cursor <= preedit.len() && preedit.is_char_boundary(cursor) {
        return Ok(cursor);
    }

    if cursor <= preedit.chars().count() {
        return Ok(preedit
            .char_indices()
            .nth(cursor)
            .map(|(index, _)| index)
            .unwrap_or(preedit.len()));
    }

    Ok(preedit.len())
}

#[cfg(test)]
mod tests {
    use radishlex_ime_core::CandidateSource;

    use super::{candidate_from_view, composition_from_parts, RimeCandidateView};
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

    #[test]
    fn converts_ascii_cursor_as_byte_offset() {
        let composition =
            composition_from_parts("luobo", 3).expect("composition conversion succeeds");

        assert_eq!(composition.preedit(), "luobo");
        assert_eq!(composition.cursor(), 3);
    }

    #[test]
    fn converts_character_cursor_to_utf8_boundary() {
        let composition =
            composition_from_parts("萝卜abc", 2).expect("composition conversion succeeds");

        assert_eq!(composition.cursor(), "萝卜".len());
    }

    #[test]
    fn clamps_negative_cursor_to_start() {
        let composition =
            composition_from_parts("luobo", -1).expect("composition conversion succeeds");

        assert_eq!(composition.cursor(), 0);
    }

    #[test]
    fn clamps_large_cursor_to_end() {
        let composition =
            composition_from_parts("萝卜", 99).expect("composition conversion succeeds");

        assert_eq!(composition.cursor(), "萝卜".len());
    }
}
