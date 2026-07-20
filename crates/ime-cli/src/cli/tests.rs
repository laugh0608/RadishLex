use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_core::{InputSession, NamedKey};
use radishlex_ime_runtime::{LearningContext, PersonalizedInputSession};
use radishlex_ime_userdb::UserDb;

#[cfg(feature = "native-rime")]
use super::build_rime_snapshot_config;
use super::rime_snapshot::{
    require_fresh_user_data, run_engine_snapshot, run_personalized_snapshot,
};
use super::{
    parse_rime_args, prepare_rime_snapshot_options, run, run_input_session, CliError,
    RankSmokeOptions, RimeCommandAction,
};
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

#[cfg(unix)]
fn snapshot_temp_path(test_name: &str) -> PathBuf {
    let unique_path = PathBuf::from(temp_file_path(test_name, "dir"));
    let canonical_temp = fs::canonicalize(std::env::temp_dir()).expect("temp directory resolves");
    canonical_temp.join(
        unique_path
            .file_name()
            .expect("temporary path has a file name"),
    )
}

#[cfg(unix)]
fn create_private_snapshot_directory(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir(path).expect("fresh snapshot directory is created");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("snapshot directory permissions are private");
}

fn remove_temp_db(path: &str) {
    for candidate in [
        path.to_owned(),
        format!("{path}-wal"),
        format!("{path}-shm"),
    ] {
        let _ = fs::remove_file(candidate);
    }
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
fn engine_snapshot_keeps_order_without_calling_selection_api_or_observing_commit() {
    let output = run_engine_snapshot(InputSession::new(DemoEngine::new()), "luobo", false)
        .expect("engine snapshot succeeds");

    assert!(output.contains("personalization_status: not_requested"));
    assert!(output.contains("0. 萝卜 [luobo]"));
    assert!(output.contains("display_index=0 engine_index=0 score=<none>"));
    assert!(output.contains("selection_api_called: false"));
    assert!(output.contains("commit_observed: false"));
}

#[test]
#[cfg(not(feature = "native-rime"))]
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

    assert_eq!(options.action, RimeCommandAction::Select);
    assert!(!options.deploy_on_start);
    assert_eq!(options.input_code, "luobo");
    assert_eq!(options.extra_keys, vec![NamedKey::PageDown]);
    assert_eq!(options.selected_index, Some(0));
}

#[test]
fn rime_snapshot_requires_explicit_deployment_and_rejects_commit_controls() {
    let options = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "luna_pinyin",
        "--shared-data",
        "shared",
        "--user-data",
        "fresh-user",
        "--deploy-on-start",
        "1",
        "--rank-db",
        "/tmp/radishlex-userdb.sqlite",
        "--context",
        "editor",
        "luobo",
    ]))
    .expect("snapshot args should parse");

    assert_eq!(options.action, RimeCommandAction::Snapshot);
    assert!(options.deploy_on_start);
    assert_eq!(options.selected_index, None);
    assert_eq!(
        options.rank_smoke,
        Some(RankSmokeOptions {
            db_path: PathBuf::from("/tmp/radishlex-userdb.sqlite"),
            context_kind: "editor".to_owned(),
        })
    );

    let missing_deploy = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "luna_pinyin",
        "--shared-data",
        "shared",
        "--user-data",
        "fresh-user",
        "luobo",
    ]))
    .expect_err("snapshot deployment choice must be explicit");
    assert!(missing_deploy
        .to_string()
        .contains("requires explicit --deploy-on-start 0|1"));

    let invalid_deploy = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "luna_pinyin",
        "--shared-data",
        "shared",
        "--user-data",
        "fresh-user",
        "--deploy-on-start",
        "2",
        "luobo",
    ]))
    .expect_err("snapshot deployment choice must be binary");
    assert!(invalid_deploy.to_string().contains("must be 0 or 1"));

    let candidate_index = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "luna_pinyin",
        "--shared-data",
        "shared",
        "--user-data",
        "fresh-user",
        "--deploy-on-start",
        "0",
        "luobo",
        "0",
    ]))
    .expect_err("snapshot must reject candidate selection");
    assert!(candidate_index
        .to_string()
        .contains("does not accept a candidate index"));

    let commit_key = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "luna_pinyin",
        "--shared-data",
        "shared",
        "--user-data",
        "fresh-user",
        "--deploy-on-start",
        "0",
        "--key",
        "space",
        "luobo",
    ]))
    .expect_err("snapshot must reject commit-capable keys");
    assert!(commit_key.to_string().contains("does not accept --key"));
}

#[test]
fn rime_snapshot_accepts_lowercase_pinyin_and_rejects_selection_capable_input_shapes() {
    let valid = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "pinyin_simp",
        "--shared-data",
        "shared",
        "--user-data",
        "fresh-user",
        "--deploy-on-start",
        "0",
        "shi",
    ]))
    .expect("fixed lowercase pinyin case must parse");
    assert_eq!(valid.input_code, "shi");

    for invalid_input in ["shi1", "Shi", "shi ", "shi-", "时", ""] {
        let error = parse_rime_args(&args(&[
            "radishlex-ime-cli",
            "rime",
            "snapshot",
            "--schema",
            "pinyin_simp",
            "--shared-data",
            "shared",
            "--user-data",
            "fresh-user",
            "--deploy-on-start",
            "0",
            invalid_input,
        ]))
        .expect_err("snapshot input outside lowercase pinyin characters must fail");
        assert!(error.to_string().contains("rime snapshot input code"));
    }

    let interactive = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "--schema",
        "pinyin_simp",
        "--shared-data",
        "shared",
        "--user-data",
        "user",
        "shi1",
    ]))
    .expect("interactive rime input keeps its existing digit support");
    assert_eq!(interactive.input_code, "shi1");
}

#[cfg(unix)]
#[test]
fn rime_snapshot_requires_an_existing_empty_non_symlink_user_data_directory() {
    let directory = snapshot_temp_path("rime-snapshot-user");
    create_private_snapshot_directory(&directory);
    let canonical = require_fresh_user_data(&directory).expect("fresh empty directory is accepted");
    assert_eq!(
        canonical,
        fs::canonicalize(&directory).expect("path resolves")
    );

    fs::write(directory.join("existing.yaml"), "existing").expect("fixture is written");
    let non_empty =
        require_fresh_user_data(&directory).expect_err("non-empty directory must be rejected");
    assert!(non_empty
        .to_string()
        .contains("must be empty before startup"));

    fs::remove_dir_all(directory).expect("fixture directory is removed");
}

#[cfg(unix)]
#[test]
fn rime_snapshot_rejects_symlinked_user_data_directory() {
    use std::os::unix::fs::symlink;

    let target = snapshot_temp_path("rime-snapshot-target");
    let link = snapshot_temp_path("rime-snapshot-link");
    create_private_snapshot_directory(&target);
    symlink(&target, &link).expect("symlink is created");
    let error = require_fresh_user_data(&link).expect_err("symlink must be rejected");
    assert!(error.to_string().contains("symlink component"));
    fs::remove_file(link).expect("symlink is removed");
    fs::remove_dir(target).expect("target directory is removed");
}

#[cfg(unix)]
#[test]
fn rime_snapshot_rejects_a_symlinked_ancestor() {
    use std::os::unix::fs::symlink;

    let target_parent = snapshot_temp_path("rime-snapshot-ancestor-target");
    let linked_parent = snapshot_temp_path("rime-snapshot-ancestor-link");
    fs::create_dir(&target_parent).expect("target parent is created");
    let child = target_parent.join("fresh-user");
    create_private_snapshot_directory(&child);
    symlink(&target_parent, &linked_parent).expect("ancestor symlink is created");
    let error = require_fresh_user_data(&linked_parent.join("fresh-user"))
        .expect_err("symlinked ancestor must be rejected");
    assert!(error.to_string().contains("symlink component"));
    fs::remove_file(linked_parent).expect("ancestor symlink is removed");
    fs::remove_dir(child).expect("child is removed");
    fs::remove_dir(target_parent).expect("target parent is removed");
}

#[cfg(unix)]
#[test]
fn rime_snapshot_requires_mode_0700() {
    use std::os::unix::fs::PermissionsExt;

    let directory = snapshot_temp_path("rime-snapshot-permissions");
    fs::create_dir(&directory).expect("snapshot directory is created");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
        .expect("unsafe fixture mode is set");
    let error = require_fresh_user_data(&directory).expect_err("public mode must be rejected");
    assert!(error.to_string().contains("mode 0700"));

    fs::remove_dir(directory).expect("fixture is removed");
}

#[cfg(unix)]
#[test]
fn rime_snapshot_preparation_stores_the_canonical_user_data_path() {
    let directory = snapshot_temp_path("rime-snapshot-canonical-options");
    create_private_snapshot_directory(&directory);
    let options = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "pinyin_simp",
        "--shared-data",
        "shared",
        "--user-data",
        directory.to_str().expect("temporary path is UTF-8"),
        "--deploy-on-start",
        "0",
        "shi",
    ]))
    .expect("snapshot arguments parse");

    let prepared = prepare_rime_snapshot_options(options).expect("snapshot options are prepared");
    assert_eq!(
        prepared.user_data,
        fs::canonicalize(&directory).expect("snapshot path resolves")
    );

    fs::remove_dir(directory).expect("fixture is removed");
}

#[cfg(all(unix, feature = "native-rime"))]
#[test]
fn rime_snapshot_config_uses_the_prepared_canonical_user_data_path() {
    let directory = snapshot_temp_path("rime-snapshot-canonical-config");
    create_private_snapshot_directory(&directory);
    let options = parse_rime_args(&args(&[
        "radishlex-ime-cli",
        "rime",
        "snapshot",
        "--schema",
        "pinyin_simp",
        "--shared-data",
        "shared",
        "--user-data",
        directory.to_str().expect("temporary path is UTF-8"),
        "--deploy-on-start",
        "1",
        "shi",
    ]))
    .expect("snapshot arguments parse");
    let prepared = prepare_rime_snapshot_options(options).expect("snapshot options are prepared");
    let config = build_rime_snapshot_config(&prepared).expect("snapshot config is built");

    assert_eq!(config.user_data_dir(), prepared.user_data.as_path());
    assert!(config.deploy_on_start());

    fs::remove_dir(directory).expect("fixture is removed");
}

#[cfg(not(unix))]
#[test]
fn rime_snapshot_fails_closed_without_unix_account_and_mode_checks() {
    let error = require_fresh_user_data(PathBuf::from("fresh-user").as_path())
        .expect_err("unsupported platform must fail closed");
    assert!(error.to_string().contains("unsupported on this platform"));
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
fn personalized_snapshot_reports_display_and_engine_indices_without_learning() {
    let db_path = temp_db_path("personalized-snapshot");
    run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "select",
        "--db",
        &db_path,
        "--input",
        "luobo",
        "--text",
        "萝卜词核",
        "--reading",
        "luobo",
        "--context",
        "editor",
        "--session",
        "seed-private-session",
    ]))
    .expect("selection seed succeeds");

    let db = UserDb::open(&db_path).expect("userdb opens for runtime");
    let mut session =
        PersonalizedInputSession::with_userdb(DemoEngine::new(), db, "snapshot-must-not-learn")
            .expect("personalized session opens");
    session.set_learning_context(LearningContext::new("editor").expect("context is valid"));
    let output =
        run_personalized_snapshot(session, "luobo", false, "editor").expect("snapshot succeeds");

    assert!(output.contains("rime_snapshot: ready"));
    assert!(output.contains("deploy_on_start: 0"));
    assert!(output.contains("personalization_status: ready"));
    assert!(output.contains("rank_context: editor"));
    assert!(output.contains("0. 萝卜词核 [luobo] - project term (display_index=0 engine_index=1"));
    assert!(output.contains("selection_api_called: false"));
    assert!(output.contains("commit_observed: false"));
    assert!(!output.contains("snapshot-must-not-learn"));
    assert!(!output.contains("seed-private-session"));

    let reopened = UserDb::open(&db_path).expect("userdb reopens");
    let summary = reopened
        .learning_status_summary()
        .expect("summary remains readable");
    assert_eq!(summary.selection_events, 1);
    let ranker_weight = reopened
        .ranker_weight("luobo", "萝卜词核", Some("luobo"), "editor")
        .expect("ranker weight query")
        .expect("seed ranker weight remains");
    assert_eq!(ranker_weight.frequency, 1);

    drop(reopened);
    remove_temp_db(&db_path);
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

    let add_error = run(&args(&[
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
    .expect_err("add cannot implicitly restore");
    assert!(add_error.to_string().contains("explicit restore"));

    let restored = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "restore",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "萝卜",
        "--reading",
        "luo bo",
    ]))
    .expect("explicit restore succeeds");
    assert!(restored.contains("restored: 萝卜"));
    assert!(restored.contains("status: active"));
    assert!(restored.contains("version_ms:"));

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

    let inspected = run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "inspect",
        "--file",
        &export_file,
    ]))
    .expect("inspect succeeds");
    assert!(inspected.contains("format: radishlex-user-terms-v1"));
    assert!(inspected.contains("records: 1"));
    assert!(inspected.contains("sync_class: P2"));
    assert!(inspected.contains("compatible: true"));

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
    assert!(explain.contains("user_term_boost: 1.000"));

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
    assert!(explain.contains("frequency_boost: 0.243"));
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
    assert!(explain.contains("negative_feedback_penalty: 0.832"));
    assert!(explain.contains("suppressed_penalty: 2.000"));

    let _ = fs::remove_file(db);
}

#[test]
fn learn_status_reports_read_only_summary_without_p1_details() {
    let db = temp_db_path("learn-status");

    run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "add",
        "--db",
        &db,
        "--input",
        "cihe",
        "--text",
        "词核",
    ]))
    .expect("dict add succeeds");
    run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "select",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "萝卜",
        "--session",
        "session-private",
        "--context",
        "chat",
    ]))
    .expect("selection succeeds");
    run(&args(&[
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
        "--context",
        "chat",
    ]))
    .expect("feedback succeeds");

    let output = run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "status",
        "--db",
        &db,
    ]))
    .expect("status succeeds");

    assert!(output.contains("learning_status: ready"));
    assert!(output.contains("plaintext_payload: false"));
    assert!(output.contains("p1_raw_details: false"));
    assert!(output.contains("context_stats: false"));
    assert!(output.contains("active_user_terms: 1"));
    assert!(output.contains("suppressed_user_terms: 1"));
    assert!(output.contains("ranker_weights: 1"));
    assert!(output.contains("deleted_tombstones: 0"));
    assert!(output.contains("selection_events: 1"));
    assert!(output.contains("negative_feedback: 1"));
    assert!(output.contains("import_batches: 0"));
    assert!(output.contains("overall_at_ms: "));
    assert!(!output.contains("session-private"));
    assert!(!output.contains("chat"));
    assert!(!output.contains("manual_suppress"));
    assert!(!output.contains("萝卜"));
    assert!(!output.contains("词核"));

    let _ = fs::remove_file(db);
}

#[test]
fn learn_case_status_is_exact_redacted_and_tracks_delete_restore() {
    let db = temp_db_path("learn-case-status");
    for _ in 0..2 {
        run(&args(&[
            "radishlex-ime-cli",
            "learn",
            "select",
            "--db",
            &db,
            "--input",
            "luobo",
            "--text",
            "合成萝卜",
            "--reading",
            "luo bo",
            "--session",
            "target-private-session",
            "--context",
            "editor",
        ]))
        .expect("target selection succeeds");
    }
    run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "select",
        "--db",
        &db,
        "--input",
        "other",
        "--text",
        "不应显示的合成词",
        "--session",
        "unrelated-private-session",
        "--context",
        "code",
    ]))
    .expect("unrelated private selection succeeds");

    let active = run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "case-status",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "合成萝卜",
        "--reading",
        "luo bo",
        "--context",
        "editor",
    ]))
    .expect("active case status succeeds");
    assert!(active.contains("learning_case_status: ready"));
    assert!(active.contains("inspection_version: 1"));
    assert!(active.contains("selection_events: 3"));
    assert!(active.contains("term:\n  present: true\n  source: engine_selection\n  status: active"));
    assert!(active.contains("  version_ms: "));
    assert!(active.contains("ranker_weight:\n  present: true\n  frequency: 2"));
    assert!(active.contains("  negative_score: 0"));
    assert!(active.contains("deleted_tombstone:\n  present: false"));
    assert!(active.contains("p1_rows: omitted"));
    assert!(!active.contains("target-private-session"));
    assert!(!active.contains("unrelated-private-session"));
    assert!(!active.contains("不应显示的合成词"));

    run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "delete",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "合成萝卜",
        "--reading",
        "luo bo",
    ]))
    .expect("target term deletes");
    let deleted = run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "case-status",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "合成萝卜",
        "--reading",
        "luo bo",
        "--context",
        "editor",
    ]))
    .expect("deleted case status succeeds");
    assert!(
        deleted.contains("term:\n  present: true\n  source: engine_selection\n  status: deleted")
    );
    assert!(deleted.contains("ranker_weight:\n  present: false"));
    assert!(deleted.contains("deleted_tombstone:\n  present: true"));
    assert!(deleted.contains("negative_feedback: 1"));

    run(&args(&[
        "radishlex-ime-cli",
        "dict",
        "restore",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "合成萝卜",
        "--reading",
        "luo bo",
    ]))
    .expect("target term restores");
    let restored = run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "case-status",
        "--db",
        &db,
        "--input",
        "luobo",
        "--text",
        "合成萝卜",
        "--reading",
        "luo bo",
        "--context",
        "editor",
    ]))
    .expect("restored case status succeeds");
    assert!(restored.contains("term:\n  present: true\n  source: manual_add\n  status: active"));
    assert!(!restored.contains("restored_at_ms: <none>"));
    assert!(restored.contains("ranker_weight:\n  present: false"));
    assert!(restored.contains("deleted_tombstone:\n  present: false"));
    assert!(!restored.contains("target-private-session"));

    remove_temp_db(&db);
}

#[test]
fn sync_preflight_reports_syncable_and_local_only_counts() {
    let db = temp_db_path("sync-preflight");

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
        "learn",
        "select",
        "--db",
        &db,
        "--input",
        "cihe",
        "--text",
        "词核",
        "--context",
        "chat",
    ]))
    .expect("selection succeeds");
    run(&args(&[
        "radishlex-ime-cli",
        "learn",
        "suppress",
        "--db",
        &db,
        "--input",
        "cihe",
        "--text",
        "词核",
        "--context",
        "chat",
    ]))
    .expect("feedback succeeds");
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

    let output = run(&args(&[
        "radishlex-ime-cli",
        "sync",
        "preflight",
        "--db",
        &db,
    ]))
    .expect("preflight succeeds");

    assert!(output.contains("sync_preflight: ready"));
    assert!(output.contains("plaintext_payload: false"));
    assert!(output.contains("dictionary.user_terms: 1"));
    assert!(output.contains("ranker.weights: 1"));
    assert!(output.contains("dictionary.deleted_terms: 1"));
    assert!(output.contains("selection_events: 1"));
    assert!(output.contains("negative_feedback: 2"));
    assert!(output.contains("import_batches: 0"));

    let _ = fs::remove_file(db);
}

#[test]
fn learning_commands_require_explicit_database_path() {
    let err = run(&args(&["radishlex-ime-cli", "dict", "list"])).expect_err("db path is required");

    assert!(matches!(err, CliError::Usage(_)));
    assert!(err.to_string().contains("missing --db"));
}
