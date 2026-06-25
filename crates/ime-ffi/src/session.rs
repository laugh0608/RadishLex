use radishlex_ime_core::{InputSession, KeyEvent, SessionState};

use crate::demo_engine::FfiDemoEngine;

pub struct RadishLexSession {
    inner: InputSession<FfiDemoEngine>,
}

impl RadishLexSession {
    pub fn new() -> Self {
        Self {
            inner: InputSession::new(FfiDemoEngine::new()),
        }
    }

    pub fn inner_mut(&mut self) -> &mut InputSession<FfiDemoEngine> {
        &mut self.inner
    }

    pub fn push_char(&mut self, ch: char) -> radishlex_ime_core::CoreResult<()> {
        self.inner.push_key(KeyEvent::press_char(ch))?;
        Ok(())
    }

    pub fn snapshot(&self) -> radishlex_ime_core::CoreResult<String> {
        render_snapshot(&self.inner.state()?)
    }
}

impl Default for RadishLexSession {
    fn default() -> Self {
        Self::new()
    }
}

fn render_snapshot(state: &SessionState) -> radishlex_ime_core::CoreResult<String> {
    let mut output = String::new();
    output.push_str(&format!("schema: {}\n", state.schema().as_str()));
    output.push_str(&format!("composition: {}\n", state.composition().preedit()));
    output.push_str(&format!("cursor: {}\n", state.composition().cursor()));
    output.push_str("candidates:\n");

    if state.candidates().is_empty() {
        output.push_str("  <none>\n");
    } else {
        for (index, candidate) in state.candidates().iter().enumerate() {
            output.push_str(&format!("  {index}. {}", candidate.text()));
            if let Some(reading) = candidate.reading() {
                output.push_str(&format!(" [{reading}]"));
            }
            if let Some(annotation) = candidate.annotation() {
                output.push_str(&format!(" - {annotation}"));
            }
            output.push('\n');
        }
    }

    Ok(output)
}
