//! Opt-in native privacy regression with fresh synthetic data; no platform claims.
#![cfg(feature = "native-rime")]

use std::collections::BTreeMap;
use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;
use std::slice;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_ffi::*;
use serde_json::{json, Value};

const CASES: &[&str] = &[
    "normal",
    "privacy",
    "unknown",
    "secure",
    "sensitive",
    "normal-to-privacy",
    "privacy-to-normal",
    "normal-to-unknown",
    "unknown-to-normal",
    "seeded-normal",
    "seeded-privacy",
    "seeded-unknown",
];
const PHASES: &[&str] = &["baseline", "cancel", "commit", "reopen"];
const MARKER: &str = "RadishLex synthetic REV-01 probe v2\n";

#[test]
#[ignore = "explicit isolated native regression; requires librime and rime_dict_manager"]
fn rime_privacy_storage_probe() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = env::temp_dir().canonicalize().unwrap().join(format!(
        "radishlex-rime-privacy-{}-{stamp}",
        std::process::id()
    ));
    private_dir(&root);
    fs::write(root.join("probe-marker"), MARKER).unwrap();
    println!("synthetic evidence: {}", root.display());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .arg(repo.join("scripts/rime-product/product_data.py"))
        .args(["assemble", "--output"])
        .arg(root.join("shared"))
        .output()
        .unwrap();
    assert!(output.status.success(), "product data: {:?}", output);
    let mut reports: Vec<Value> = Vec::new();
    for case in CASES {
        let case_root = root.join(case);
        private_dir(&case_root);
        private_dir(&case_root.join("user"));
        private_dir(&case_root.join("logs"));
        if case.starts_with("seeded-") {
            seed_userdict(&case_root);
        }
        let initial_userdict = userdict_files(&case_root);
        let mut previous_files = BTreeMap::new();
        let mut stages = Vec::new();
        for phase in PHASES {
            // Each phase is a new OS process; no process-level Rime cache survives.
            let output = Command::new(env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "rime_privacy_probe_worker",
                    "--nocapture",
                ])
                .env("RADISHLEX_PRIVACY_PROBE_ROOT", &root)
                .env("RADISHLEX_PRIVACY_PROBE_CASE", case)
                .env("RADISHLEX_PRIVACY_PROBE_PHASE", phase)
                .current_dir(&case_root)
                .output()
                .unwrap();
            fs::write(case_root.join(format!("{phase}.stdout")), &output.stdout).unwrap();
            fs::write(case_root.join(format!("{phase}.stderr")), &output.stderr).unwrap();
            assert!(
                output.status.success(),
                "{case}/{phase}: see {}",
                case_root.display()
            );
            let mut stage: Value =
                serde_json::from_slice(&fs::read(case_root.join(format!("{phase}.json"))).unwrap())
                    .unwrap();
            let files = read_files(&case_root.join("user"));
            let changed: Vec<_> = files
                .iter()
                .filter(|(name, bytes)| previous_files.get(*name) != Some(*bytes))
                .map(|(name, _)| name.clone())
                .collect();
            let removed: Vec<_> = previous_files
                .keys()
                .filter(|name| !files.contains_key(*name))
                .cloned()
                .collect();
            previous_files = files;
            assert_eq!(
                userdict_files(&case_root),
                initial_userdict,
                "adapter must never create or modify Rime learned data: {case}/{phase}"
            );
            let rows = export_rows(&case_root, phase, !case.starts_with("seeded-"));
            if *phase == "baseline" || *phase == "cancel" {
                assert_eq!(rows.len(), usize::from(case.starts_with("seeded-")));
                assert_eq!(stage["selection_events"], 0);
            }
            stage["rime_rows"] = json!(rows);
            stage["changed_user_files"] = json!(changed);
            stage["removed_user_files"] = json!(removed);
            stages.push(stage);
        }
        let commit = &stages[2];
        let selected = commit["selected"].as_str().unwrap();
        let persisted = stages[3]["rime_rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row.as_str().unwrap().split('\t').next() == Some(selected));
        let log_matches: Vec<_> = read_files(&case_root.join("logs"))
            .iter()
            .filter(|(_, bytes)| {
                bytes
                    .windows(selected.len())
                    .any(|w| w == selected.as_bytes())
            })
            .map(|(name, _)| name.clone())
            .collect();
        assert!(!persisted, "selected text must not enter Rime userdict");
        assert_eq!(
            stages[1]["candidates"], stages[3]["candidates"],
            "engine-only order must survive commit/restart without learned influence"
        );
        assert_eq!(
            stages[3]["selection_events"],
            u64::from(*case == "normal" || *case == "seeded-normal")
        );
        assert!(
            log_matches.is_empty(),
            "designated log directory contains synthetic selection"
        );
        if case.starts_with("seeded-") {
            assert_eq!(
                stages[3]["candidates"], reports[0]["stages"][3]["candidates"],
                "old Rime userdict must not affect engine-only candidates"
            );
        }
        let report = json!({
            "case": case,
            "selected_persisted_in_rime_after_restart": persisted,
            "sqlite_skipped_but_rime_persisted":
                commit["disposition"] == RADISHLEX_LEARNING_SKIPPED_BY_POLICY && persisted,
            "logs_containing_selected_utf8": log_matches,
            "stages": stages,
        });
        println!(
            "{}",
            json!({
                "case": case,
                "sqlite_events": report["stages"][3]["selection_events"],
                "rime_rows": report["stages"][3]["rime_rows"].as_array().unwrap().len(),
                "sqlite_skipped_but_rime_persisted": report["sqlite_skipped_but_rime_persisted"],
            })
        );
        reports.push(report);
    }
    fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&json!({
            "purpose": "native privacy regression, not real platform acceptance",
            "synthetic_input": "shi",
            "schema": "radishlex_pinyin",
            "reports": reports,
        }))
        .unwrap(),
    )
    .unwrap();
    // Keep synthetic artifacts for inspection; never clean existing probe directories.
}

#[test]
#[ignore = "internal subprocess of rime_privacy_storage_probe"]
fn rime_privacy_probe_worker() {
    let root = PathBuf::from(env::var_os("RADISHLEX_PRIVACY_PROBE_ROOT").expect("probe parent"));
    assert_eq!(
        root.parent().unwrap(),
        env::temp_dir().canonicalize().unwrap()
    );
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-rime-privacy-"));
    assert_eq!(
        fs::read_to_string(root.join("probe-marker")).unwrap(),
        MARKER
    );
    let case = env::var("RADISHLEX_PRIVACY_PROBE_CASE").unwrap();
    let phase = env::var("RADISHLEX_PRIVACY_PROBE_PHASE").unwrap();
    assert!(CASES.contains(&case.as_str()) && PHASES.contains(&phase.as_str()));
    let case_root = root.join(&case);
    let policy_case = case.strip_prefix("seeded-").unwrap_or(&case);
    let shared = path_string(&root.join("shared"));
    let user = path_string(&case_root.join("user"));
    let logs = path_string(&case_root.join("logs"));
    let db = path_string(&case_root.join("radishlex.sqlite3"));
    let schema = CString::new("radishlex_pinyin").unwrap();
    let session_id = CString::new(format!("synthetic-{case}-{phase}")).unwrap();
    let options = RadishLexPersonalizedRimeSessionOptions {
        version: RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
        shared_data_dir: shared.as_ptr(),
        user_data_dir: user.as_ptr(),
        schema: schema.as_ptr(),
        log_dir: logs.as_ptr(),
        deploy_on_start: u8::from(phase == "baseline"),
        userdb_path: db.as_ptr(),
        session_id: session_id.as_ptr(),
    };
    let mut error = ptr::null_mut();
    let session = radishlex_session_new_personalized_rime(&options, &mut error);
    assert!(!session.is_null(), "{}", error_text(error));
    let (initial, final_context) = policy_case
        .split_once("-to-")
        .unwrap_or((policy_case, policy_case));
    // Reopen deliberately blocks RadishLex ranking, isolating Rime's learned order.
    set_context(
        session,
        if phase == "reopen" {
            "unknown"
        } else {
            initial
        },
        &mut error,
    );
    let mut before = Vec::new();
    let mut selected = None;
    let mut disposition = None;
    if phase != "baseline" {
        for ch in "shi".chars() {
            let mut result = ptr::null_mut();
            check(
                unsafe {
                    radishlex_session_handle_key_event(
                        session,
                        RadishLexKeyEvent::press_char(ch),
                        &mut result,
                        &mut error,
                    )
                },
                error,
            );
            assert_eq!(unsafe { radishlex_key_result_commit_present(result) }, 0);
            unsafe {
                radishlex_key_result_free(result);
            }
        }
        if phase == "commit" {
            set_context(session, final_context, &mut error);
        }
        before = candidates(session, &mut error);
        assert!(before.len() > 1);
        if phase == "commit" {
            let mut result = ptr::null_mut();
            check(
                unsafe { radishlex_session_select_candidate(session, 1, &mut result, &mut error) },
                error,
            );
            assert_eq!(unsafe { radishlex_key_result_commit_present(result) }, 1);
            let text = view(unsafe { radishlex_key_result_commit(result) });
            assert_eq!(text, before[1]);
            selected = Some(text);
            let actual = unsafe { radishlex_key_result_learning_disposition(result) };
            let expected = if initial == "normal" && final_context == "normal" {
                RADISHLEX_LEARNING_RECORDED
            } else {
                RADISHLEX_LEARNING_SKIPPED_BY_POLICY
            };
            assert_eq!(actual, expected);
            disposition = Some(actual);
            unsafe {
                radishlex_key_result_free(result);
            }
        } else {
            check(radishlex_session_reset(session, &mut error), error);
        }
    }
    unsafe {
        radishlex_session_free(session);
    }
    check(
        unsafe { radishlex_rime_runtime_shutdown(&mut error) },
        error,
    );
    let mut summary = RadishLexLearningStatusSummary::empty();
    check(
        unsafe { radishlex_userdb_learning_status(db.as_ptr(), &mut summary, &mut error) },
        error,
    );
    fs::write(
        case_root.join(format!("{phase}.json")),
        serde_json::to_vec_pretty(&json!({
            "phase": phase, "candidates": before, "selected": selected,
            "disposition": disposition, "selection_events": summary.selection_events,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn seed_userdict(case_root: &Path) {
    let user = case_root.join("user");
    let seed = case_root.join("seed.txt");
    fs::write(&seed, "# Rime user dictionary\n#@/db_name\tpinyin_simp\n#@/db_type\tuserdb\n合成隐私回归词\tshi \t1000000\n").unwrap();
    let output = Command::new("rime_dict_manager")
        .args(["--import", "pinyin_simp"])
        .arg(&seed)
        .env("GLOG_log_dir", case_root.join("logs"))
        .current_dir(&user)
        .output()
        .unwrap();
    assert!(output.status.success(), "synthetic seed: {:?}", output);
    assert!(!userdict_files(case_root).is_empty());
}

fn userdict_files(case_root: &Path) -> BTreeMap<String, Vec<u8>> {
    read_files(&case_root.join("user"))
        .into_iter()
        .filter(|(name, _)| {
            name.starts_with("pinyin_simp.userdb/") || name.ends_with(".userdb.txt")
        })
        .collect()
}

fn private_dir(path: &Path) {
    fs::create_dir(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

fn read_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, path: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                visit(root, &entry.path(), files);
            } else if kind.is_file() {
                files.insert(
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    fs::read(entry.path()).unwrap(),
                );
            } else {
                // glog creates convenience symlinks; never follow them.
                assert!(kind.is_symlink());
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn export_rows(case_root: &Path, phase: &str, disabled: bool) -> Vec<String> {
    if disabled {
        assert!(
            !case_root.join("user/pinyin_simp.userdb").exists(),
            "disabled fresh-schema control unexpectedly created a user dictionary"
        );
        return Vec::new();
    }
    // Export opens LevelDB for writing. Inspect a copy of the closed synthetic DB.
    let copy = case_root.join(format!("{phase}-export"));
    private_dir(&copy);
    let db_copy = copy.join("pinyin_simp.userdb");
    private_dir(&db_copy);
    for (name, bytes) in read_files(&case_root.join("user/pinyin_simp.userdb")) {
        assert!(!name.contains('/'), "unexpected nested LevelDB file");
        fs::write(db_copy.join(name), bytes).unwrap();
    }
    let output = Command::new("rime_dict_manager")
        .args(["--export", "pinyin_simp", "export.txt"])
        .env("GLOG_log_dir", &copy)
        .current_dir(&copy)
        .output()
        .unwrap();
    fs::write(copy.join("stdout"), &output.stdout).unwrap();
    fs::write(copy.join("stderr"), &output.stderr).unwrap();
    assert!(output.status.success(), "export failed: {}", copy.display());
    fs::read_to_string(copy.join("export.txt"))
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn set_context(session: *mut RadishLexSession, kind: &str, error: &mut *mut RadishLexError) {
    let name = b"general";
    let context = RadishLexLearningContext {
        version: RADISHLEX_LEARNING_CONTEXT_VERSION,
        secure_input: u8::from(kind == "secure"),
        sensitive_application: u8::from(kind == "sensitive"),
        privacy_mode: u8::from(kind == "privacy"),
        context_known: u8::from(kind != "unknown"),
        context_kind: RadishLexStringView {
            data: name.as_ptr(),
            len: name.len(),
        },
    };
    check(
        unsafe { radishlex_session_set_learning_context(session, context, error) },
        *error,
    );
}

fn candidates(session: *mut RadishLexSession, error: &mut *mut RadishLexError) -> Vec<String> {
    let snapshot = radishlex_session_snapshot_new(session, error);
    assert!(!snapshot.is_null(), "{}", error_text(*error));
    let mut result = Vec::new();
    for index in 0..radishlex_snapshot_candidate_count(snapshot) {
        let mut candidate = RadishLexCandidateView::empty();
        check(
            unsafe { radishlex_snapshot_candidate(snapshot, index, &mut candidate, error) },
            *error,
        );
        result.push(view(candidate.text));
    }
    unsafe {
        radishlex_snapshot_free(snapshot);
    }
    result
}

fn path_string(path: &Path) -> CString {
    CString::new(path.to_str().unwrap()).unwrap()
}

fn view(value: RadishLexStringView) -> String {
    if value.len == 0 {
        return String::new();
    }
    assert!(!value.data.is_null());
    // FFI views are borrowed; copy before freeing their owning result/snapshot.
    String::from_utf8(unsafe { slice::from_raw_parts(value.data, value.len) }.to_vec()).unwrap()
}

fn check(status: RadishLexStatusCode, error: *mut RadishLexError) {
    assert_eq!(status, RadishLexStatusCode::Ok, "{}", error_text(error));
}

fn error_text(error: *mut RadishLexError) -> String {
    if error.is_null() {
        return "no FFI error".to_owned();
    }
    unsafe { CStr::from_ptr(radishlex_error_message(error)) }
        .to_string_lossy()
        .into_owned()
}
