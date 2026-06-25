use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use radishlex_ime_core::{
    Candidate, CoreError, Engine, InputSession, Key, KeyEvent, NamedKey, SchemaId, SessionState,
};
#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::{RimeEngine, RimeEngineConfig};

use crate::DemoEngine;

const USAGE: &str = "\
Usage:
  radishlex-ime-cli demo <input-code> [candidate-index]
  radishlex-ime-cli rime --schema <schema> --shared-data <path> --user-data <path> [--key <name> ...] <input-code> [candidate-index]

Examples:
  radishlex-ime-cli demo luobo
  radishlex-ime-cli demo luobo 1
  radishlex-ime-cli rime --schema luna_pinyin --shared-data ./rime-data --user-data ./tmp/rime-user luobo
  radishlex-ime-cli rime --schema luna_pinyin --shared-data ./rime-data --user-data ./tmp/rime-user luobo --key page-down 0
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
        Some("rime") => run_rime(args),
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

    let selected_index = parse_optional_candidate_index(args.get(3))?;
    if args.len() > 4 {
        return Err(CliError::Usage("too many arguments for demo".to_owned()));
    }

    let mut session = InputSession::new(DemoEngine::new());
    session.set_schema(SchemaId::new("demo.pinyin")?)?;
    run_input_session(session, input_code, &[], selected_index)
}

#[cfg(not(feature = "native-rime"))]
fn run_rime(args: &[String]) -> Result<String, CliError> {
    let _ = parse_rime_args(args)?;
    Err(CliError::Usage(
        "rime command requires building radishlex-ime-cli with --features native-rime".to_owned(),
    ))
}

#[cfg(feature = "native-rime")]
fn run_rime(args: &[String]) -> Result<String, CliError> {
    let options = parse_rime_args(args)?;
    let schema = SchemaId::new(options.schema)?;
    let config = RimeEngineConfig::new(options.shared_data, options.user_data, schema)
        .map_err(rime_error_to_cli)?;
    let session = InputSession::new(RimeEngine::new(config).map_err(rime_error_to_cli)?);
    run_input_session(
        session,
        &options.input_code,
        &options.extra_keys,
        options.selected_index,
    )
}

fn run_input_session<E: Engine>(
    mut session: InputSession<E>,
    input_code: &str,
    extra_keys: &[NamedKey],
    selected_index: Option<usize>,
) -> Result<String, CliError> {
    for ch in input_code.chars() {
        session.push_key(KeyEvent::press_char(ch))?;
    }
    for key in extra_keys {
        session.push_key(KeyEvent::press(Key::Named(*key)))?;
    }

    let state = session.state()?;
    let commit_text = if let Some(index) = selected_index.or(default_index(&state)) {
        Some(session.commit_candidate(index)?.text().to_owned())
    } else {
        None
    };

    Ok(render_session(input_code, &state, commit_text.as_deref()))
}

#[cfg(feature = "native-rime")]
fn rime_error_to_cli(error: radishlex_ime_engine_rime::RimeEngineError) -> CliError {
    CliError::Core(CoreError::engine(error.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RimeCommandOptions {
    schema: String,
    shared_data: PathBuf,
    user_data: PathBuf,
    input_code: String,
    extra_keys: Vec<NamedKey>,
    selected_index: Option<usize>,
}

fn parse_rime_args(args: &[String]) -> Result<RimeCommandOptions, CliError> {
    let mut schema = None;
    let mut shared_data = None;
    let mut user_data = None;
    let mut extra_keys = Vec::new();
    let mut positional = Vec::new();
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--schema" => {
                index += 1;
                schema = Some(required_option_value(args, index, "--schema")?.to_owned());
            }
            "--shared-data" => {
                index += 1;
                shared_data = Some(PathBuf::from(required_option_value(
                    args,
                    index,
                    "--shared-data",
                )?));
            }
            "--user-data" => {
                index += 1;
                user_data = Some(PathBuf::from(required_option_value(
                    args,
                    index,
                    "--user-data",
                )?));
            }
            "--key" => {
                index += 1;
                let value = required_option_value(args, index, "--key")?;
                extra_keys.push(parse_named_key(value)?);
            }
            value if value.starts_with("--") => {
                return Err(CliError::Usage(format!("unknown rime option: {value}")));
            }
            value => positional.push(value.to_owned()),
        }
        index += 1;
    }

    let input_code = positional
        .first()
        .ok_or_else(|| CliError::Usage("missing input code for rime".to_owned()))?;
    validate_input_code(input_code)?;
    if positional.len() > 2 {
        return Err(CliError::Usage(
            "too many positional arguments for rime".to_owned(),
        ));
    }

    Ok(RimeCommandOptions {
        schema: schema.ok_or_else(|| CliError::Usage("missing --schema".to_owned()))?,
        shared_data: shared_data
            .ok_or_else(|| CliError::Usage("missing --shared-data".to_owned()))?,
        user_data: user_data.ok_or_else(|| CliError::Usage("missing --user-data".to_owned()))?,
        input_code: input_code.to_owned(),
        extra_keys,
        selected_index: parse_optional_candidate_index(positional.get(1))?,
    })
}

fn required_option_value<'a>(
    args: &'a [String],
    index: usize,
    option: &str,
) -> Result<&'a str, CliError> {
    let value = args
        .get(index)
        .ok_or_else(|| CliError::Usage(format!("missing value for {option}")))?;
    if value.starts_with("--") {
        return Err(CliError::Usage(format!("missing value for {option}")));
    }
    Ok(value)
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

fn parse_optional_candidate_index(value: Option<&String>) -> Result<Option<usize>, CliError> {
    value.map(|value| parse_candidate_index(value)).transpose()
}

fn parse_named_key(value: &str) -> Result<NamedKey, CliError> {
    match value {
        "space" => Ok(NamedKey::Space),
        "enter" => Ok(NamedKey::Enter),
        "backspace" => Ok(NamedKey::Backspace),
        "escape" => Ok(NamedKey::Escape),
        "tab" => Ok(NamedKey::Tab),
        "arrow-up" => Ok(NamedKey::ArrowUp),
        "arrow-down" => Ok(NamedKey::ArrowDown),
        "arrow-left" => Ok(NamedKey::ArrowLeft),
        "arrow-right" => Ok(NamedKey::ArrowRight),
        "page-up" => Ok(NamedKey::PageUp),
        "page-down" => Ok(NamedKey::PageDown),
        _ => Err(CliError::Usage(format!(
            "unknown key name: {value}; expected one of space, enter, backspace, escape, tab, arrow-up, arrow-down, arrow-left, arrow-right, page-up, page-down"
        ))),
    }
}

fn default_index(state: &SessionState) -> Option<usize> {
    if state.candidates().is_empty() {
        None
    } else {
        Some(0)
    }
}

fn render_session(input_code: &str, state: &SessionState, commit_text: Option<&str>) -> String {
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
    use radishlex_ime_core::NamedKey;

    use super::{parse_rime_args, run, CliError};

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

    #[test]
    fn rime_command_requires_native_feature_by_default() {
        let err = run(&args(&[
            "radishlex-ime-cli",
            "rime",
            "--schema",
            "luna_pinyin",
            "--shared-data",
            "shared",
            "--user-data",
            "user",
            "luobo",
        ]))
        .expect_err("default build cannot run native rime");

        assert!(err.to_string().contains("native-rime"));
    }

    #[test]
    fn rime_command_rejects_missing_schema() {
        let err = run(&args(&[
            "radishlex-ime-cli",
            "rime",
            "--shared-data",
            "shared",
            "--user-data",
            "user",
            "luobo",
        ]))
        .expect_err("schema is required");

        assert!(err.to_string().contains("missing --schema"));
    }

    #[test]
    fn rime_args_parse_extra_key_after_input_code() {
        let options = parse_rime_args(&args(&[
            "radishlex-ime-cli",
            "rime",
            "--schema",
            "luna_pinyin",
            "--shared-data",
            "shared",
            "--user-data",
            "user",
            "luobo",
            "--key",
            "page-down",
            "0",
        ]))
        .expect("rime args should parse");

        assert_eq!(options.input_code, "luobo");
        assert_eq!(options.extra_keys, vec![NamedKey::PageDown]);
        assert_eq!(options.selected_index, Some(0));
    }

    #[test]
    fn rime_args_parse_repeated_extra_keys_before_input_code() {
        let options = parse_rime_args(&args(&[
            "radishlex-ime-cli",
            "rime",
            "--schema",
            "luna_pinyin",
            "--shared-data",
            "shared",
            "--user-data",
            "user",
            "--key",
            "page-down",
            "--key",
            "page-up",
            "luobo",
        ]))
        .expect("rime args should parse");

        assert_eq!(
            options.extra_keys,
            vec![NamedKey::PageDown, NamedKey::PageUp]
        );
        assert_eq!(options.selected_index, None);
    }

    #[test]
    fn rime_command_rejects_unknown_extra_key() {
        let err = run(&args(&[
            "radishlex-ime-cli",
            "rime",
            "--schema",
            "luna_pinyin",
            "--shared-data",
            "shared",
            "--user-data",
            "user",
            "luobo",
            "--key",
            "home",
        ]))
        .expect_err("unknown key must fail");

        assert!(matches!(err, CliError::Usage(_)));
        assert!(err.to_string().contains("unknown key name: home"));
    }
}
