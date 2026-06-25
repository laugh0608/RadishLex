use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use radishlex_ime_core::{
    Candidate, CoreError, Engine, InputSession, Key, KeyEvent, NamedKey, SchemaId, SessionState,
};
#[cfg(feature = "native-rime")]
use radishlex_ime_engine_rime::{RimeEngine, RimeEngineConfig};
use radishlex_ime_ranker::{RankRequest, RankedCandidate, Ranker};
use radishlex_ime_userdb::{
    decode_dictionary_terms_tsv, encode_dictionary_terms_tsv, DictionaryTermRecord,
    NegativeFeedbackDraft, NegativeFeedbackReason, RankerWeight, SelectionEventDraft, TermSource,
    UserDb, UserDbError, UserTerm,
};

use crate::DemoEngine;

const USAGE: &str = "\
Usage:
  radishlex-ime-cli demo <input-code> [candidate-index]
  radishlex-ime-cli rime --schema <schema> --shared-data <path> --user-data <path> [--key <name> ...] [--rank-db <path>] [--context <kind>] <input-code> [candidate-index]
  radishlex-ime-cli dict list --db <path>
  radishlex-ime-cli dict add --db <path> --input <code> --text <text> [--reading <reading>]
  radishlex-ime-cli dict delete --db <path> --input <code> --text <text> [--reading <reading>]
  radishlex-ime-cli dict export --db <path> --file <path>
  radishlex-ime-cli dict import --db <path> --file <path> [--source <name>] [--dry-run]
  radishlex-ime-cli dict import-batches --db <path>
  radishlex-ime-cli learn select --db <path> --input <code> --text <text> [--reading <reading>] [--index <n>] [--count <n>] [--session <id>] [--context <kind>]
  radishlex-ime-cli learn suppress --db <path> --input <code> --text <text> [--reading <reading>] [--reason <reason>] [--context <kind>]
  radishlex-ime-cli rank explain --db <path> --input <code> --candidate <text> [--reading <reading>] [--context <kind>]

Examples:
  radishlex-ime-cli demo luobo
  radishlex-ime-cli demo luobo 1
  radishlex-ime-cli rime --schema luna_pinyin --shared-data ./rime-data --user-data ./tmp/rime-user luobo
  radishlex-ime-cli rime --schema luna_pinyin --shared-data ./rime-data --user-data ./tmp/rime-user luobo --key page-down 0
  radishlex-ime-cli rime --schema luna_pinyin --shared-data ./rime-data --user-data ./tmp/rime-user --rank-db /tmp/radishlex-userdb.sqlite luobo
  radishlex-ime-cli dict add --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
  radishlex-ime-cli dict export --db /tmp/radishlex-userdb.sqlite --file /tmp/radishlex-terms.tsv
  radishlex-ime-cli dict import --db /tmp/radishlex-userdb.sqlite --file /tmp/radishlex-terms.tsv --dry-run
  radishlex-ime-cli dict import --db /tmp/radishlex-userdb.sqlite --file /tmp/radishlex-terms.tsv --source smoke
  radishlex-ime-cli dict import-batches --db /tmp/radishlex-userdb.sqlite
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
    run_input_session(session, input_code, &[], selected_index, None)
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
        options.rank_smoke.as_ref(),
    )
}

fn run_dict(args: &[String]) -> Result<String, CliError> {
    match args.get(2).map(String::as_str) {
        Some("list") => run_dict_list(args),
        Some("add") => run_dict_add(args),
        Some("delete") => run_dict_delete(args),
        Some("export") => run_dict_export(args),
        Some("import") => run_dict_import(args),
        Some("import-batches") => run_dict_import_batches(args),
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

fn run_dict_export(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(args, 3, &["db", "file", "output"], "dict export")?;
    let db_path = required_named_option(&options, "db")?;
    let file_path = required_one_of_options(&options, "file", "output")?;

    let db = UserDb::open(db_path)?;
    let records = db.export_dictionary_records()?;
    let encoded = encode_dictionary_terms_tsv(&records);
    fs::write(&file_path, encoded).map_err(|error| {
        CliError::Data(format!("failed to write {}: {error}", file_path.display()))
    })?;

    Ok(format!(
        "exported: {}\nfile: {}\nformat: radishlex-user-terms-v1\n",
        records.len(),
        file_path.display()
    ))
}

fn run_dict_import(args: &[String]) -> Result<String, CliError> {
    let parsed = parse_named_options_with_flags(
        args,
        3,
        &["db", "file", "input", "source"],
        &["dry-run"],
        "dict import",
    )?;
    let options = parsed.options;
    let dry_run = parsed.flags.contains("dry-run");
    let db_path = required_named_option(&options, "db")?;
    let file_path = required_one_of_options(&options, "file", "input")?;
    let source_name = options.get("source").map_or("cli", String::as_str);

    let encoded = fs::read_to_string(&file_path).map_err(|error| {
        CliError::Data(format!("failed to read {}: {error}", file_path.display()))
    })?;
    let records = decode_dictionary_terms_tsv(&encoded)?;
    validate_import_input_codes(&records)?;

    let mut db = UserDb::open(db_path)?;
    let summary = if dry_run {
        db.preview_dictionary_import(&records, source_name)?
    } else {
        db.import_dictionary_records(&records, source_name)?
    };

    Ok(render_import_summary(
        &summary,
        source_name,
        &file_path,
        dry_run,
    ))
}

fn run_dict_import_batches(args: &[String]) -> Result<String, CliError> {
    let options = parse_named_options(args, 3, &["db"], "dict import-batches")?;
    let db_path = required_named_option(&options, "db")?;
    let db = UserDb::open(db_path)?;
    let batches = db.list_import_batches()?;

    let mut output = String::from("import_batches:\n");
    if batches.is_empty() {
        output.push_str("  <none>\n");
    } else {
        for batch in batches {
            output.push_str(&format!(
                "  {}. source={} imported={} inserted={} updated={} skipped_deleted={} skipped_duplicate={} total={} created_at_ms={}\n",
                batch.id,
                batch.source_name,
                batch.imported_terms,
                batch.inserted_terms,
                batch.updated_terms,
                batch.skipped_deleted_terms,
                batch.skipped_duplicate_terms,
                batch.total_records,
                batch.created_at_ms
            ));
        }
    }
    Ok(output)
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
    rank_smoke: Option<&RankSmokeOptions>,
) -> Result<String, CliError> {
    for ch in input_code.chars() {
        session.push_key(KeyEvent::press_char(ch))?;
    }
    for key in extra_keys {
        session.push_key(KeyEvent::press(Key::Named(*key)))?;
    }

    let state = session.state()?;
    if let Some(rank_smoke) = rank_smoke {
        let ranked = rank_state_candidates(input_code, &state, rank_smoke)?;
        let commit_engine_index = ranked_commit_engine_index(&ranked, selected_index)?;
        let commit_text = if let Some(index) = commit_engine_index {
            Some(session.commit_candidate(index)?.text().to_owned())
        } else {
            None
        };

        return Ok(render_ranked_session(
            input_code,
            &state,
            &ranked,
            rank_smoke,
            commit_text.as_deref(),
            commit_engine_index,
        ));
    }

    let commit_engine_index = selected_index.or(default_index(&state));
    let commit_text = if let Some(index) = commit_engine_index {
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
    rank_smoke: Option<RankSmokeOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RankSmokeOptions {
    db_path: PathBuf,
    context_kind: String,
}

fn parse_rime_args(args: &[String]) -> Result<RimeCommandOptions, CliError> {
    let mut schema = None;
    let mut shared_data = None;
    let mut user_data = None;
    let mut rank_db = None;
    let mut context_kind = None;
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
            "--rank-db" => {
                index += 1;
                rank_db = Some(PathBuf::from(required_option_value(
                    args,
                    index,
                    "--rank-db",
                )?));
            }
            "--context" => {
                index += 1;
                context_kind = Some(required_option_value(args, index, "--context")?.to_owned());
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

    let rank_smoke = match rank_db {
        Some(db_path) => Some(RankSmokeOptions {
            db_path,
            context_kind: context_kind.unwrap_or_else(|| "general".to_owned()),
        }),
        None if context_kind.is_some() => {
            return Err(CliError::Usage(
                "--context requires --rank-db for rime".to_owned(),
            ));
        }
        None => None,
    };

    Ok(RimeCommandOptions {
        schema: schema.ok_or_else(|| CliError::Usage("missing --schema".to_owned()))?,
        shared_data: shared_data
            .ok_or_else(|| CliError::Usage("missing --shared-data".to_owned()))?,
        user_data: user_data.ok_or_else(|| CliError::Usage("missing --user-data".to_owned()))?,
        input_code: input_code.to_owned(),
        extra_keys,
        selected_index: parse_optional_candidate_index(positional.get(1))?,
        rank_smoke,
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

struct ParsedNamedOptions {
    options: BTreeMap<String, String>,
    flags: BTreeSet<String>,
}

fn parse_named_options_with_flags(
    args: &[String],
    start: usize,
    allowed_options: &[&str],
    allowed_flags: &[&str],
    command_name: &str,
) -> Result<ParsedNamedOptions, CliError> {
    let mut options = BTreeMap::new();
    let mut flags = BTreeSet::new();
    let mut index = start;

    while index < args.len() {
        let option = args[index].as_str();
        if !option.starts_with("--") {
            return Err(CliError::Usage(format!(
                "unexpected positional argument for {command_name}: {option}"
            )));
        }

        let name = option.trim_start_matches("--");
        if allowed_flags.contains(&name) {
            if !flags.insert(name.to_owned()) {
                return Err(CliError::Usage(format!(
                    "duplicate {command_name} flag: {option}"
                )));
            }
            index += 1;
            continue;
        }

        if !allowed_options.contains(&name) {
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

    Ok(ParsedNamedOptions { options, flags })
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

fn required_one_of_options(
    options: &BTreeMap<String, String>,
    first: &'static str,
    second: &'static str,
) -> Result<PathBuf, CliError> {
    match (options.get(first), options.get(second)) {
        (Some(_), Some(_)) => Err(CliError::Usage(format!(
            "--{first} and --{second} cannot be used together"
        ))),
        (Some(value), None) | (None, Some(value)) => Ok(PathBuf::from(value)),
        (None, None) => Err(CliError::Usage(format!("missing --{first}"))),
    }
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

fn validate_import_input_codes(records: &[DictionaryTermRecord]) -> Result<(), CliError> {
    for record in records {
        if record.input_code.is_empty()
            || !record
                .input_code
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '\'')
        {
            return Err(CliError::Data(format!(
                "invalid import file: input code for term {} must contain only ASCII letters, digits, or apostrophes",
                record.text
            )));
        }
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

fn rank_state_candidates(
    input_code: &str,
    state: &SessionState,
    options: &RankSmokeOptions,
) -> Result<Vec<RankedCandidate>, CliError> {
    let db = UserDb::open(&options.db_path)?;
    let mut user_terms = Vec::new();
    let mut ranker_weights = Vec::new();

    for candidate in state.candidates() {
        if let Some(term) = fetch_candidate_user_term(&db, input_code, candidate)? {
            user_terms.push(term);
        }
        if let Some(weight) =
            fetch_candidate_ranker_weight(&db, input_code, candidate, &options.context_kind)?
        {
            ranker_weights.push(weight);
        }
    }

    let request = RankRequest::new(input_code, state.candidates().to_vec())
        .with_context_kind(&options.context_kind)
        .with_user_terms(user_terms)
        .with_ranker_weights(ranker_weights);
    Ok(Ranker::default().rank(request))
}

fn fetch_candidate_user_term(
    db: &UserDb,
    input_code: &str,
    candidate: &Candidate,
) -> Result<Option<UserTerm>, CliError> {
    let reading = candidate.reading().unwrap_or_default();
    if let Some(term) = db.fetch_term(input_code, candidate.text(), reading)? {
        return Ok(Some(term));
    }
    if !reading.is_empty() {
        return db
            .fetch_term(input_code, candidate.text(), "")
            .map_err(Into::into);
    }
    Ok(None)
}

fn fetch_candidate_ranker_weight(
    db: &UserDb,
    input_code: &str,
    candidate: &Candidate,
    context_kind: &str,
) -> Result<Option<RankerWeight>, CliError> {
    if let Some(weight) = db.ranker_weight(
        input_code,
        candidate.text(),
        candidate.reading(),
        context_kind,
    )? {
        return Ok(Some(weight));
    }
    if candidate.reading().is_some() {
        return db
            .ranker_weight(input_code, candidate.text(), None, context_kind)
            .map_err(Into::into);
    }
    Ok(None)
}

fn ranked_commit_engine_index(
    ranked: &[RankedCandidate],
    selected_index: Option<usize>,
) -> Result<Option<usize>, CliError> {
    let Some(index) = selected_index.or_else(|| if ranked.is_empty() { None } else { Some(0) })
    else {
        return Ok(None);
    };

    ranked
        .get(index)
        .map(|candidate| Some(candidate.original_index))
        .ok_or_else(|| CoreError::InvalidCandidateIndex {
            index,
            len: ranked.len(),
        })
        .map_err(Into::into)
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

fn render_ranked_session(
    input_code: &str,
    state: &SessionState,
    ranked: &[RankedCandidate],
    options: &RankSmokeOptions,
    commit_text: Option<&str>,
    commit_engine_index: Option<usize>,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("schema: {}\n", state.schema().as_str()));
    output.push_str(&format!("input: {input_code}\n"));
    output.push_str(&format!("composition: {}\n", state.composition().preedit()));
    output.push_str(&format!("rank_context: {}\n", options.context_kind));
    output.push_str("candidates:\n");

    if ranked.is_empty() {
        output.push_str("  <none>\n");
    } else {
        for (index, candidate) in ranked.iter().enumerate() {
            output.push_str(&format_ranked_candidate(index, candidate));
        }
    }

    match commit_text {
        Some(text) => output.push_str(&format!("commit: {text}\n")),
        None => output.push_str("commit: <none>\n"),
    }
    if let Some(index) = commit_engine_index {
        output.push_str(&format!("commit_engine_index: {index}\n"));
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

fn format_ranked_candidate(index: usize, ranked: &RankedCandidate) -> String {
    let candidate = &ranked.candidate;
    let reading = candidate
        .reading()
        .map(|value| format!(" [{value}]"))
        .unwrap_or_default();
    let annotation = candidate
        .annotation()
        .map(|value| format!(" - {value}"))
        .unwrap_or_default();
    let explanation = &ranked.explanation;

    format!(
        "  {index}. {}{reading}{annotation} (engine_index={} score={:.3})\n     explain: engine_order={:.3} user_term={:.3} frequency={:.3} recency={:.3} context={:.3} negative={:.3} suppressed={:.3} deleted={:.3}\n",
        candidate.text(),
        ranked.original_index,
        ranked.final_score,
        explanation.engine_order_factor,
        explanation.user_term_boost,
        explanation.frequency_boost,
        explanation.recency_boost,
        explanation.context_boost,
        explanation.negative_feedback_penalty,
        explanation.suppressed_penalty,
        explanation.deleted_penalty
    )
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

fn render_import_summary(
    summary: &radishlex_ime_userdb::DictionaryImportSummary,
    source_name: &str,
    file_path: &PathBuf,
    dry_run: bool,
) -> String {
    let mut output = String::new();
    if dry_run {
        output.push_str("dry_run: true\n");
        output.push_str(&format!("would_import: {}\n", summary.imported_terms));
    } else {
        if let Some(batch_id) = summary.import_batch_id {
            output.push_str(&format!("import_batch: {batch_id}\n"));
        }
        output.push_str(&format!("imported: {}\n", summary.imported_terms));
    }
    output.push_str(&format!("total: {}\n", summary.total_records));
    output.push_str(&format!("inserted: {}\n", summary.inserted_terms));
    output.push_str(&format!("updated: {}\n", summary.updated_terms));
    output.push_str(&format!(
        "skipped_deleted: {}\n",
        summary.skipped_deleted_terms
    ));
    output.push_str(&format!(
        "skipped_duplicate: {}\n",
        summary.skipped_duplicate_terms
    ));
    output.push_str(&format!("source: {source_name}\n"));
    output.push_str(&format!("file: {}\n", file_path.display()));
    output
}

#[cfg(test)]
mod tests;
