use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use radishlex_ime_core::{
    Candidate, CoreError, Engine, InputSession, Key, KeyEvent, NamedKey, SchemaId, SessionState,
};
#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::{RimeEngine, RimeEngineConfig};
use radishlex_ime_ranker::{RankRequest, RankedCandidate, Ranker};
use radishlex_ime_userdb::{
    NegativeFeedbackDraft, NegativeFeedbackReason, SelectionEventDraft, TermSource, UserDb,
    UserDbError,
};

use crate::DemoEngine;

const USAGE: &str = "\
Usage:
  radishlex-ime-cli demo <input-code> [candidate-index]
  radishlex-ime-cli rime --schema <schema> --shared-data <path> --user-data <path> [--key <name> ...] <input-code> [candidate-index]
  radishlex-ime-cli dict list --db <path>
  radishlex-ime-cli dict add --db <path> --input <code> --text <text> [--reading <reading>]
  radishlex-ime-cli dict delete --db <path> --input <code> --text <text> [--reading <reading>]
  radishlex-ime-cli learn select --db <path> --input <code> --text <text> [--reading <reading>] [--index <n>] [--count <n>] [--session <id>] [--context <kind>]
  radishlex-ime-cli learn suppress --db <path> --input <code> --text <text> [--reading <reading>] [--reason <reason>] [--context <kind>]
  radishlex-ime-cli rank explain --db <path> --input <code> --candidate <text> [--reading <reading>] [--context <kind>]

Examples:
  radishlex-ime-cli demo luobo
  radishlex-ime-cli demo luobo 1
  radishlex-ime-cli rime --schema luna_pinyin --shared-data ./rime-data --user-data ./tmp/rime-user luobo
  radishlex-ime-cli rime --schema luna_pinyin --shared-data ./rime-data --user-data ./tmp/rime-user luobo --key page-down 0
  radishlex-ime-cli dict add --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
  radishlex-ime-cli learn select --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
  radishlex-ime-cli rank explain --db /tmp/radishlex-userdb.sqlite --input luobo --candidate 萝卜
";

#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Core(CoreError),
    Data(String),
}

impl CliError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) => ExitCode::from(2),
            Self::Core(_) | Self::Data(_) => ExitCode::from(1),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => write!(f, "{message}\n\n{USAGE}"),
            Self::Core(error) => write!(f, "{error}"),
            Self::Data(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<CoreError> for CliError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

impl From<UserDbError> for CliError {
    fn from(error: UserDbError) -> Self {
        Self::Data(error.to_string())
    }
}

pub fn run(args: &[String]) -> Result<String, CliError> {
    let command = args.get(1).map(String::as_str);
    match command {
        Some("demo") => run_demo(args),
        Some("rime") => run_rime(args),
        Some("dict") => run_dict(args),
        Some("learn") => run_learn(args),
        Some("rank") => run_rank(args),
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

fn run_dict(args: &[String]) -> Result<String, CliError> {
    match args.get(2).map(String::as_str) {
        Some("list") => run_dict_list(args),
        Some("add") => run_dict_add(args),
        Some("delete") => run_dict_delete(args),
        Some(other) => Err(CliError::Usage(format!("unknown dict command: {other}"))),
        None => Err(CliError::Usage("missing dict command".to_owned())),
    }
}

fn run_dict_list(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(args, 3, &["db"], "dict list")?;
    let db_path = required_named_option(&options, "db")?;
    let db = UserDb::open(db_path)?;
    let terms = db.list_active_terms()?;

    let mut output = String::from("terms:\n");
    if terms.is_empty() {
        output.push_str("  <none>\n");
    } else {
        for term in terms {
            output.push_str(&format!(
                "  {} -> {}{} source={} status={} weight={:.3}\n",
                term.input_code,
                term.text,
                term.reading
                    .as_deref()
                    .map(|reading| format!(" [{reading}]"))
                    .unwrap_or_default(),
                term.source,
                term.status,
                term.weight
            ));
        }
    }
    Ok(output)
}

fn run_dict_add(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(args, 3, &["db", "input", "text", "reading"], "dict add")?;
    let db_path = required_named_option(&options, "db")?;
    let input_code = required_named_option(&options, "input")?;
    validate_input_code(input_code)?;
    let text = required_named_option(&options, "text")?;
    let reading = options.get("reading").map(String::as_str);

    let mut db = UserDb::open(db_path)?;
    let term = db.add_term(input_code, text, reading, TermSource::ManualAdd)?;

    Ok(format!(
        "added: {}\ninput: {}\nstatus: {}\nweight: {:.3}\n",
        term.text, term.input_code, term.status, term.weight
    ))
}

fn run_dict_delete(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(args, 3, &["db", "input", "text", "reading"], "dict delete")?;
    let db_path = required_named_option(&options, "db")?;
    let input_code = required_named_option(&options, "input")?;
    validate_input_code(input_code)?;
    let text = required_named_option(&options, "text")?;
    let reading = options.get("reading").map(String::as_str);

    let mut db = UserDb::open(db_path)?;
    db.delete_term(input_code, text, reading)?;

    Ok(format!("deleted: {text}\ninput: {input_code}\n"))
}

fn run_learn(args: &[String]) -> Result<String, CliError> {
    match args.get(2).map(String::as_str) {
        Some("select") => run_learn_select(args),
        Some("suppress") => run_learn_suppress(args),
        Some(other) => Err(CliError::Usage(format!("unknown learn command: {other}"))),
        None => Err(CliError::Usage("missing learn command".to_owned())),
    }
}

fn run_learn_select(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(
        args,
        3,
        &[
            "db", "input", "text", "reading", "index", "count", "session", "context",
        ],
        "learn select",
    )?;
    let db_path = required_named_option(&options, "db")?;
    let input_code = required_named_option(&options, "input")?;
    validate_input_code(input_code)?;
    let text = required_named_option(&options, "text")?;
    let reading = options.get("reading").map(String::as_str);
    let index = parse_optional_usize(options.get("index"), "index")?.unwrap_or(0);
    let count = parse_optional_usize(options.get("count"), "count")?.unwrap_or(index + 1);
    let session_id = options.get("session").map_or("cli", String::as_str);
    let context_kind = options.get("context").map_or("general", String::as_str);

    let mut event = SelectionEventDraft::new(session_id, input_code, text, index, count)
        .with_context_kind(context_kind);
    if let Some(reading) = reading {
        event = event.with_reading(reading);
    }

    let mut db = UserDb::open(db_path)?;
    let event_id = db.record_selection(event)?.ok_or_else(|| {
        CliError::Data("selection event was skipped by privacy policy".to_owned())
    })?;

    Ok(format!(
        "selection_event: {event_id}\ninput: {input_code}\ntext: {text}\n"
    ))
}

fn run_learn_suppress(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(
        args,
        3,
        &["db", "input", "text", "reading", "reason", "context"],
        "learn suppress",
    )?;
    let db_path = required_named_option(&options, "db")?;
    let input_code = required_named_option(&options, "input")?;
    validate_input_code(input_code)?;
    let text = required_named_option(&options, "text")?;
    let reading = options.get("reading").map(String::as_str);
    let context_kind = options.get("context").map_or("general", String::as_str);
    let reason = options
        .get("reason")
        .map(|value| NegativeFeedbackReason::from_str(value))
        .transpose()?
        .unwrap_or(NegativeFeedbackReason::ManualSuppress);

    let mut feedback =
        NegativeFeedbackDraft::new(input_code, text, reason).with_context_kind(context_kind);
    if let Some(reading) = reading {
        feedback = feedback.with_reading(reading);
    }

    let mut db = UserDb::open(db_path)?;
    let feedback_id = db.record_negative_feedback(feedback)?.ok_or_else(|| {
        CliError::Data("negative feedback was skipped by privacy policy".to_owned())
    })?;

    Ok(format!(
        "negative_feedback: {feedback_id}\ninput: {input_code}\ntext: {text}\nreason: {reason}\n"
    ))
}

fn run_rank(args: &[String]) -> Result<String, CliError> {
    match args.get(2).map(String::as_str) {
        Some("explain") => run_rank_explain(args),
        Some(other) => Err(CliError::Usage(format!("unknown rank command: {other}"))),
        None => Err(CliError::Usage("missing rank command".to_owned())),
    }
}

fn run_rank_explain(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(
        args,
        3,
        &["db", "input", "candidate", "reading", "context"],
        "rank explain",
    )?;
    let db_path = required_named_option(&options, "db")?;
    let input_code = required_named_option(&options, "input")?;
    validate_input_code(input_code)?;
    let candidate_text = required_named_option(&options, "candidate")?;
    let reading = options.get("reading").map(String::as_str);
    let context_kind = options.get("context").map_or("general", String::as_str);

    let db = UserDb::open(db_path)?;
    let stored_reading = reading.unwrap_or_default();
    let mut user_terms = Vec::new();
    if let Some(term) = db.fetch_term(input_code, candidate_text, stored_reading)? {
        user_terms.push(term);
    }

    let mut ranker_weights = Vec::new();
    if let Some(weight) = db.ranker_weight(input_code, candidate_text, reading, context_kind)? {
        ranker_weights.push(weight);
    }

    let mut candidate = Candidate::new(candidate_text);
    if let Some(reading) = reading {
        candidate = candidate.with_reading(reading);
    }

    let request = RankRequest::new(input_code, vec![candidate])
        .with_context_kind(context_kind)
        .with_user_terms(user_terms)
        .with_ranker_weights(ranker_weights);
    let ranked = Ranker::default().rank(request);
    let candidate = ranked
        .first()
        .ok_or_else(|| CliError::Data("ranker returned no candidates".to_owned()))?;

    Ok(render_rank_explanation(input_code, context_kind, candidate))
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

fn parse_named_options(
    args: &[String],
    start: usize,
    allowed: &[&str],
    command_name: &str,
) -> Result<BTreeMap<String, String>, CliError> {
    let mut options = BTreeMap::new();
    let mut index = start;

    while index < args.len() {
        let option = args[index].as_str();
        if !option.starts_with("--") {
            return Err(CliError::Usage(format!(
                "unexpected positional argument for {command_name}: {option}"
            )));
        }

        let name = option.trim_start_matches("--");
        if !allowed.contains(&name) {
            return Err(CliError::Usage(format!(
                "unknown {command_name} option: {option}"
            )));
        }
        if options.contains_key(name) {
            return Err(CliError::Usage(format!(
                "duplicate {command_name} option: {option}"
            )));
        }

        index += 1;
        let value = required_option_value(args, index, option)?;
        options.insert(name.to_owned(), value.to_owned());
        index += 1;
    }

    Ok(options)
}

fn required_named_option<'a>(
    options: &'a BTreeMap<String, String>,
    name: &'static str,
) -> Result<&'a str, CliError> {
    options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| CliError::Usage(format!("missing --{name}")))
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

fn parse_optional_usize(
    value: Option<&String>,
    field: &'static str,
) -> Result<Option<usize>, CliError> {
    value
        .map(|value| {
            value.parse::<usize>().map_err(|_| {
                CliError::Usage(format!("{field} must be a non-negative integer: {value}"))
            })
        })
        .transpose()
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

fn render_rank_explanation(
    input_code: &str,
    context_kind: &str,
    ranked: &RankedCandidate,
) -> String {
    let explanation = &ranked.explanation;
    let mut output = String::new();
    output.push_str(&format!("input: {input_code}\n"));
    output.push_str(&format!("candidate: {}\n", ranked.candidate.text()));
    output.push_str(&format!("context: {context_kind}\n"));
    output.push_str(&format!("original_index: {}\n", ranked.original_index));
    output.push_str(&format!("final_score: {:.3}\n", ranked.final_score));
    output.push_str("explain:\n");
    output.push_str(&format!(
        "  engine_order_factor: {:.3}\n",
        explanation.engine_order_factor
    ));
    output.push_str(&format!(
        "  user_term_boost: {:.3}\n",
        explanation.user_term_boost
    ));
    output.push_str(&format!(
        "  frequency_boost: {:.3}\n",
        explanation.frequency_boost
    ));
    output.push_str(&format!(
        "  recency_boost: {:.3}\n",
        explanation.recency_boost
    ));
    output.push_str(&format!(
        "  context_boost: {:.3}\n",
        explanation.context_boost
    ));
    output.push_str(&format!(
        "  negative_feedback_penalty: {:.3}\n",
        explanation.negative_feedback_penalty
    ));
    output.push_str(&format!(
        "  suppressed_penalty: {:.3}\n",
        explanation.suppressed_penalty
    ));
    output.push_str(&format!(
        "  deleted_penalty: {:.3}\n",
        explanation.deleted_penalty
    ));
    output
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use radishlex_ime_core::NamedKey;

    use super::{parse_rime_args, run, CliError};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn temp_db_path(test_name: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "radishlex-cli-{test_name}-{}-{timestamp}.sqlite",
            std::process::id()
        ));
        path.to_string_lossy().into_owned()
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

    #[test]
    fn dict_commands_add_list_and_delete_terms() {
        let db = temp_db_path("dict");

        let empty =
            run(&args(&["radishlex-ime-cli", "dict", "list", "--db", &db])).expect("list succeeds");
        assert!(empty.contains("terms:\n  <none>"));

        let added = run(&args(&[
            "radishlex-ime-cli",
            "dict",
            "add",
            "--db",
            &db,
            "--input",
            "luobo",
            "--text",
            "萝卜",
            "--reading",
            "luo bo",
        ]))
        .expect("add succeeds");
        assert!(added.contains("added: 萝卜"));
        assert!(added.contains("status: active"));

        let listed =
            run(&args(&["radishlex-ime-cli", "dict", "list", "--db", &db])).expect("list succeeds");
        assert!(listed.contains("luobo -> 萝卜 [luo bo]"));

        let deleted = run(&args(&[
            "radishlex-ime-cli",
            "dict",
            "delete",
            "--db",
            &db,
            "--input",
            "luobo",
            "--text",
            "萝卜",
            "--reading",
            "luo bo",
        ]))
        .expect("delete succeeds");
        assert!(deleted.contains("deleted: 萝卜"));

        let listed =
            run(&args(&["radishlex-ime-cli", "dict", "list", "--db", &db])).expect("list succeeds");
        assert!(listed.contains("terms:\n  <none>"));

        let _ = fs::remove_file(db);
    }

    #[test]
    fn learn_commands_feed_rank_explain() {
        let db = temp_db_path("learn");

        let event = run(&args(&[
            "radishlex-ime-cli",
            "learn",
            "select",
            "--db",
            &db,
            "--input",
            "luobo",
            "--text",
            "萝卜",
            "--index",
            "1",
            "--count",
            "2",
            "--context",
            "chat",
        ]))
        .expect("selection succeeds");
        assert!(event.contains("selection_event:"));

        let explain = run(&args(&[
            "radishlex-ime-cli",
            "rank",
            "explain",
            "--db",
            &db,
            "--input",
            "luobo",
            "--candidate",
            "萝卜",
            "--context",
            "chat",
        ]))
        .expect("rank explain succeeds");
        assert!(explain.contains("candidate: 萝卜"));
        assert!(explain.contains("user_term_boost: 1.000"));
        assert!(explain.contains("frequency_boost: 0.350"));
        assert!(explain.contains("context_boost: 0.300"));

        let feedback = run(&args(&[
            "radishlex-ime-cli",
            "learn",
            "suppress",
            "--db",
            &db,
            "--input",
            "luobo",
            "--text",
            "萝卜",
            "--reason",
            "manual_suppress",
        ]))
        .expect("feedback succeeds");
        assert!(feedback.contains("negative_feedback:"));

        let explain = run(&args(&[
            "radishlex-ime-cli",
            "rank",
            "explain",
            "--db",
            &db,
            "--input",
            "luobo",
            "--candidate",
            "萝卜",
        ]))
        .expect("rank explain succeeds");
        assert!(explain.contains("negative_feedback_penalty: 1.200"));
        assert!(explain.contains("suppressed_penalty: 2.000"));

        let _ = fs::remove_file(db);
    }

    #[test]
    fn learning_commands_require_explicit_database_path() {
        let err =
            run(&args(&["radishlex-ime-cli", "dict", "list"])).expect_err("db path is required");

        assert!(matches!(err, CliError::Usage(_)));
        assert!(err.to_string().contains("missing --db"));
    }
}
