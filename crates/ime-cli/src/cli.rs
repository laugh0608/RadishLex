use std::fmt;
use std::process::ExitCode;

use radishlex_ime_core::{Candidate, CoreError, InputSession, KeyEvent, SchemaId, SessionState};

use crate::DemoEngine;

const USAGE: &str = "\
Usage:
  radishlex-ime-cli demo <input-code> [candidate-index]

Examples:
  radishlex-ime-cli demo luobo
  radishlex-ime-cli demo luobo 1
";

#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Core(CoreError),
}

impl CliError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) => ExitCode::from(2),
            Self::Core(_) => ExitCode::from(1),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => write!(f, "{message}\n\n{USAGE}"),
            Self::Core(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<CoreError> for CliError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

pub fn run(args: &[String]) -> Result<String, CliError> {
    let command = args.get(1).map(String::as_str);
    match command {
        Some("demo") => run_demo(args),
        Some("-h" | "--help") => Ok(USAGE.to_owned()),
        Some(other) => Err(CliError::Usage(format!("unknown command: {other}"))),
        None => Err(CliError::Usage("missing command".to_owned())),
    }
}

fn run_demo(args: &[String]) -> Result<String, CliError> {
    let input_code = args
        .get(2)
        .ok_or_else(|| CliError::Usage("missing input code".to_owned()))?;
    validate_input_code(input_code)?;

    let selected_index = match args.get(3) {
        Some(value) => Some(parse_candidate_index(value)?),
        None => None,
    };
    if args.len() > 4 {
        return Err(CliError::Usage("too many arguments for demo".to_owned()));
    }

    let mut session = InputSession::new(DemoEngine::new());
    session.set_schema(SchemaId::new("demo.pinyin")?)?;
    for ch in input_code.chars() {
        session.push_key(KeyEvent::press_char(ch))?;
    }

    let state = session.state()?;
    let commit_text = if let Some(index) = selected_index.or(default_index(&state)) {
        Some(session.commit_candidate(index)?.text().to_owned())
    } else {
        None
    };

    Ok(render_demo(input_code, &state, commit_text.as_deref()))
}

fn validate_input_code(input_code: &str) -> Result<(), CliError> {
    if input_code.is_empty() {
        return Err(CliError::Usage("input code cannot be empty".to_owned()));
    }
    if !input_code
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '\'')
    {
        return Err(CliError::Usage(
            "input code must contain only ASCII letters, digits, or apostrophes".to_owned(),
        ));
    }
    Ok(())
}

fn parse_candidate_index(value: &str) -> Result<usize, CliError> {
    value.parse::<usize>().map_err(|_| {
        CliError::Usage(format!(
            "candidate index must be a non-negative integer: {value}"
        ))
    })
}

fn default_index(state: &SessionState) -> Option<usize> {
    if state.candidates().is_empty() {
        None
    } else {
        Some(0)
    }
}

fn render_demo(input_code: &str, state: &SessionState, commit_text: Option<&str>) -> String {
    let mut output = String::new();
    output.push_str(&format!("schema: {}\n", state.schema().as_str()));
    output.push_str(&format!("input: {input_code}\n"));
    output.push_str(&format!("composition: {}\n", state.composition().preedit()));
    output.push_str("candidates:\n");

    if state.candidates().is_empty() {
        output.push_str("  <none>\n");
    } else {
        for (index, candidate) in state.candidates().iter().enumerate() {
            output.push_str(&format_candidate(index, candidate));
        }
    }

    match commit_text {
        Some(text) => output.push_str(&format!("commit: {text}\n")),
        None => output.push_str("commit: <none>\n"),
    }

    output
}

fn format_candidate(index: usize, candidate: &Candidate) -> String {
    let reading = candidate
        .reading()
        .map(|value| format!(" [{value}]"))
        .unwrap_or_default();
    let annotation = candidate
        .annotation()
        .map(|value| format!(" - {value}"))
        .unwrap_or_default();

    format!("  {index}. {}{reading}{annotation}\n", candidate.text())
}

#[cfg(test)]
mod tests {
    use super::{run, CliError};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn demo_command_commits_default_candidate() {
        let output =
            run(&args(&["radishlex-ime-cli", "demo", "luobo"])).expect("demo command succeeds");

        assert!(output.contains("schema: demo.pinyin"));
        assert!(output.contains("composition: luobo"));
        assert!(output.contains("0. 萝卜 [luobo]"));
        assert!(output.contains("commit: 萝卜"));
    }

    #[test]
    fn demo_command_can_select_candidate_by_index() {
        let output = run(&args(&["radishlex-ime-cli", "demo", "luobo", "1"]))
            .expect("demo command succeeds");

        assert!(output.contains("1. 萝卜词核 [luobo]"));
        assert!(output.contains("commit: 萝卜词核"));
    }

    #[test]
    fn demo_command_shows_no_commit_for_unknown_code() {
        let output =
            run(&args(&["radishlex-ime-cli", "demo", "unknown"])).expect("demo command succeeds");

        assert!(output.contains("candidates:\n  <none>"));
        assert!(output.contains("commit: <none>"));
    }

    #[test]
    fn demo_command_rejects_invalid_candidate_index() {
        let err = run(&args(&["radishlex-ime-cli", "demo", "luobo", "abc"]))
            .expect_err("invalid index must fail");

        assert!(matches!(err, CliError::Usage(_)));
    }
}
