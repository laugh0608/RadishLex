use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_core::{InputSession, NamedKey};

use super::{parse_rime_args, run, run_input_session, CliError, RankSmokeOptions};
use crate::DemoEngine;

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

fn temp_file_path(test_name: &str, extension: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!(
        "radishlex-cli-{test_name}-{}-{timestamp}.{extension}",
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
    let output =
        run(&args(&["radishlex-ime-cli", "demo", "luobo", "1"])).expect("demo command succeeds");

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
fn rime_args_parse_rank_smoke_options() {
    let options = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "--schema",
        "luna_pinyin",
        "--shared-data",
        "shared",
        "--user-data",
        "user",
        "--rank-db",
        "/tmp/radishlex-userdb.sqlite",
        "--context",
        "chat",
        "luobo",
    ]))
    .expect("rime args should parse");

    assert_eq!(
        options.rank_smoke,
        Some(RankSmokeOptions {
            db_path: PathBuf::from("/tmp/radishlex-userdb.sqlite"),
            context_kind: "chat".to_owned(),
        })
    );
}

#[test]
fn rime_args_reject_context_without_rank_db() {
    let err = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "--schema",
        "luna_pinyin",
        "--shared-data",
        "shared",
        "--user-data",
        "user",
        "--context",
        "chat",
        "luobo",
    ]))
    .expect_err("context without rank db must fail");

    assert!(err.to_string().contains("--context requires --rank-db"));
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
fn ranked_input_session_promotes_user_term_and_commits_engine_index() {
    let db = temp_db_path("ranked-session");
    run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "add",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "萝卜词核",
    ]))
    .expect("dict add succeeds");

    let options = RankSmokeOptions {
        db_path: PathBuf::from(&db),
        context_kind: "general".to_owned(),
    };
    let output = run_input_session(
        InputSession::new(DemoEngine::new()),
        "luobo",
        &[],
        None,
        Some(&options),
    )
    .expect("ranked session succeeds");

    assert!(output.contains("rank_context: general"));
    assert!(output.contains("0. 萝卜词核 [luobo] - project term (engine_index=1"));
    assert!(output.contains("user_term=1.000"));
    assert!(output.contains("commit: 萝卜词核"));
    assert!(output.contains("commit_engine_index: 1"));

    let _ = fs::remove_file(db);
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
fn dict_export_import_round_trip_feeds_rank_explain() {
    let source_db = temp_db_path("dict-export-source");
    let target_db = temp_db_path("dict-export-target");
    let export_file = temp_file_path("dict-export", "tsv");

    run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "add",
        "--db",
        &source_db,
        "--input",
        "luobo",
        "--text",
        "萝卜",
        "--reading",
        "luo bo",
    ]))
    .expect("add succeeds");
    run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "select",
        "--db",
        &source_db,
        "--input",
        "luobo",
        "--text",
        "萝卜",
        "--reading",
        "luo bo",
        "--session",
        "session-private",
        "--context",
        "chat",
    ]))
    .expect("selection succeeds");

    let exported = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "export",
        "--db",
        &source_db,
        "--file",
        &export_file,
    ]))
    .expect("export succeeds");
    assert!(exported.contains("exported: 1"));

    let export_text = fs::read_to_string(&export_file).expect("export file is readable");
    assert!(export_text.contains("# radishlex-user-terms-v1"));
    assert!(export_text.contains("input_code\ttext\treading\tsource\tweight\tstatus"));
    assert!(export_text.contains("luobo\t萝卜\tluo bo"));
    assert!(!export_text.contains("session-private"));
    assert!(!export_text.contains("chat"));

    let preview = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import",
        "--db",
        &target_db,
        "--file",
        &export_file,
        "--source",
        "round-trip",
        "--dry-run",
    ]))
    .expect("preview succeeds");
    assert!(preview.contains("dry_run: true"));
    assert!(preview.contains("would_import: 1"));
    assert!(preview.contains("inserted: 1"));

    let batches = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import-batches",
        "--db",
        &target_db,
    ]))
    .expect("batch list succeeds");
    assert!(batches.contains("import_batches:\n  <none>"));

    let imported = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import",
        "--db",
        &target_db,
        "--file",
        &export_file,
        "--source",
        "round-trip",
    ]))
    .expect("import succeeds");
    assert!(imported.contains("import_batch: 1"));
    assert!(imported.contains("imported: 1"));
    assert!(imported.contains("inserted: 1"));
    assert!(imported.contains("updated: 0"));
    assert!(imported.contains("skipped_deleted: 0"));

    let batches = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import-batches",
        "--db",
        &target_db,
    ]))
    .expect("batch list succeeds");
    assert!(batches.contains("1. source=round-trip imported=1 inserted=1 updated=0"));

    let explain = run(&args(&[
        "radishlex-ime-cli",
        "rank",
        "explain",
        "--db",
        &target_db,
        "--input",
        "luobo",
        "--candidate",
        "萝卜",
        "--reading",
        "luo bo",
    ]))
    .expect("rank explain succeeds");
    assert!(explain.contains("user_term_boost: 2.000"));

    let _ = fs::remove_file(source_db);
    let _ = fs::remove_file(target_db);
    let _ = fs::remove_file(export_file);
}

#[test]
fn dict_import_reports_updates_and_duplicate_records() {
    let db = temp_db_path("dict-import-stats");
    let import_file = temp_file_path("dict-import-stats", "tsv");
    fs::write(
        &import_file,
        "\
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\tluo bo\tmanual_import\t3.0\tsuppressed
luobo\t萝卜\tluo bo\tmanual_import\t9.0\tactive
cihe\t词核\t\tmanual_import\t1.0\tactive
",
    )
    .expect("import file is written");

    run(&args(&[
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

    let preview = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import",
        "--db",
        &db,
        "--file",
        &import_file,
        "--dry-run",
    ]))
    .expect("preview succeeds");
    assert!(preview.contains("would_import: 2"));
    assert!(preview.contains("inserted: 1"));
    assert!(preview.contains("updated: 1"));
    assert!(preview.contains("skipped_duplicate: 1"));

    let imported = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import",
        "--db",
        &db,
        "--file",
        &import_file,
    ]))
    .expect("import succeeds");
    assert!(imported.contains("imported: 2"));
    assert!(imported.contains("updated: 1"));
    assert!(imported.contains("skipped_duplicate: 1"));

    let listed =
        run(&args(&["radishlex-ime-cli", "dict", "list", "--db", &db])).expect("list succeeds");
    assert!(listed
        .contains("luobo -> 萝卜 [luo bo] source=manual_import status=suppressed weight=3.000"));
    assert!(listed.contains("cihe -> 词核 source=manual_import status=active weight=1.000"));

    let _ = fs::remove_file(db);
    let _ = fs::remove_file(import_file);
}

#[test]
fn dict_import_respects_deleted_tombstone() {
    let db = temp_db_path("dict-import-deleted");
    let import_file = temp_file_path("dict-import-deleted", "tsv");
    fs::write(
        &import_file,
        "\
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\tluo bo\tmanual_import\t1.0\tactive
",
    )
    .expect("import file is written");

    run(&args(&[
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
    run(&args(&[
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

    let imported = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import",
        "--db",
        &db,
        "--file",
        &import_file,
    ]))
    .expect("import succeeds");
    assert!(imported.contains("imported: 0"));
    assert!(imported.contains("skipped_deleted: 1"));

    let listed =
        run(&args(&["radishlex-ime-cli", "dict", "list", "--db", &db])).expect("list succeeds");
    assert!(listed.contains("terms:\n  <none>"));

    let _ = fs::remove_file(db);
    let _ = fs::remove_file(import_file);
}

#[test]
fn dict_import_rejects_malformed_file() {
    let db = temp_db_path("dict-import-bad");
    let import_file = temp_file_path("dict-import-bad", "tsv");
    fs::write(
        &import_file,
        "\
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\t\tmanual_import\tbad\tactive
",
    )
    .expect("import file is written");

    let err = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import",
        "--db",
        &db,
        "--file",
        &import_file,
    ]))
    .expect_err("bad import file must fail");

    assert!(matches!(err, CliError::Data(_)));
    assert!(err.to_string().contains("invalid weight"));

    let _ = fs::remove_file(db);
    let _ = fs::remove_file(import_file);
}

#[test]
fn dict_import_rejects_invalid_source_name() {
    let db = temp_db_path("dict-import-source");
    let import_file = temp_file_path("dict-import-source", "tsv");
    fs::write(
        &import_file,
        "\
# radishlex-user-terms-v1
input_code\ttext\treading\tsource\tweight\tstatus
luobo\t萝卜\t\tmanual_import\t1.0\tactive
",
    )
    .expect("import file is written");

    let err = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "import",
        "--db",
        &db,
        "--file",
        &import_file,
        "--source",
        "bad source",
        "--dry-run",
    ]))
    .expect_err("bad source must fail");

    assert!(matches!(err, CliError::Data(_)));
    assert!(err.to_string().contains("source_name"));

    let _ = fs::remove_file(db);
    let _ = fs::remove_file(import_file);
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
    let err = run(&args(&["radishlex-ime-cli", "dict", "list"])).expect_err("db path is required");

    assert!(matches!(err, CliError::Usage(_)));
    assert!(err.to_string().contains("missing --db"));
}
