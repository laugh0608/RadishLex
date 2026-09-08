//! Isolated native config regression. All input, configs and databases are synthetic.
#![cfg(feature = "native-rime")]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use radishlex_ime_core::{Engine, KeyEvent, SchemaId};
use radishlex_ime_engine_rime::{shutdown_process_runtime, RimeEngine, RimeEngineConfig};

const CASES: &[&str] = &[
    "compiled-enabled",
    "compiled-missing",
    "custom-translator",
    "unselected-schema",
    "source-customization",
    "pinned-configs",
];
const SCHEMA: &str = "radishlex_pinyin";
const MARKER: &str = "RadishLex synthetic native learning guard v1\n";

#[test]
#[ignore = "explicit isolated native config regression; requires librime"]
fn rime_learning_guard_native_regression() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = env::temp_dir().canonicalize().unwrap().join(format!(
        "radishlex-learning-guard-{}-{stamp}",
        std::process::id()
    ));
    private_dir(&root);
    fs::write(root.join("marker"), MARKER).unwrap();
    println!("synthetic config evidence: {}", root.display());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .arg(repo.join("scripts/rime-product/product_data.py"))
        .args(["assemble", "--output"])
        .arg(root.join("shared"))
        .output()
        .unwrap();
    assert!(output.status.success(), "assemble: {:?}", output);
    for case in CASES {
        let case_root = root.join(case);
        private_dir(&case_root);
        private_dir(&case_root.join("user"));
        private_dir(&case_root.join("logs"));
        for phase in ["prepare", "check"] {
            let output = Command::new(env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "rime_learning_guard_worker",
                    "--nocapture",
                ])
                .env("RADISHLEX_LEARNING_GUARD_ROOT", &root)
                .env("RADISHLEX_LEARNING_GUARD_CASE", case)
                .env("RADISHLEX_LEARNING_GUARD_PHASE", phase)
                .current_dir(&case_root)
                .output()
                .unwrap();
            fs::write(case_root.join(format!("{phase}.stdout")), &output.stdout).unwrap();
            fs::write(case_root.join(format!("{phase}.stderr")), &output.stderr).unwrap();
            assert!(
                output.status.success(),
                "{case}/{phase}: {}",
                case_root.display()
            );
        }
        assert!(!case_root.join("user/pinyin_simp.userdb").exists());
        println!("{case}: passed, no Rime userdb created");
    }
}

#[test]
#[ignore = "internal worker of rime_learning_guard_native_regression"]
fn rime_learning_guard_worker() {
    let root = PathBuf::from(env::var_os("RADISHLEX_LEARNING_GUARD_ROOT").expect("parent"));
    assert_eq!(
        root.parent().unwrap(),
        env::temp_dir().canonicalize().unwrap()
    );
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("radishlex-learning-guard-"));
    assert_eq!(fs::read_to_string(root.join("marker")).unwrap(), MARKER);
    let case = env::var("RADISHLEX_LEARNING_GUARD_CASE").unwrap();
    let phase = env::var("RADISHLEX_LEARNING_GUARD_PHASE").unwrap();
    assert!(CASES.contains(&case.as_str()));
    let case_root = root.join(&case);
    let config = RimeEngineConfig::new(root.join("shared"), case_root.join("user"), schema(SCHEMA))
        .unwrap()
        .with_log_dir(case_root.join("logs"))
        .unwrap();
    if phase == "prepare" {
        drop(RimeEngine::new(config.with_deploy_on_start(true)).unwrap());
        shutdown_process_runtime().unwrap();
        alter_configs(&case_root, &case);
        return;
    }
    assert_eq!(phase, "check");
    if case == "pinned-configs" {
        check_pinned_configs(config, &case_root);
    } else {
        let deploy = case == "source-customization";
        let error = RimeEngine::new(config.with_deploy_on_start(deploy)).unwrap_err();
        assert!(error.to_string().contains("learning_guard"), "{error}");
        println!("rejected before native session creation: {error}");
        shutdown_process_runtime().unwrap();
    }
}

fn alter_configs(root: &Path, case: &str) {
    let compiled = root.join("user/build/radishlex_pinyin.schema.yaml");
    match case {
        "compiled-enabled" => replace(
            &compiled,
            "enable_user_dict: false",
            "enable_user_dict: true",
        ),
        "compiled-missing" => replace(
            &compiled,
            "enable_user_dict: false",
            "enable_sentence: true",
        ),
        "custom-translator" => replace(&compiled, "script_translator", "script_translator@other"),
        "unselected-schema" => {
            let other = root.join("user/build/unsafe.schema.yaml");
            fs::copy(&compiled, &other).unwrap();
            replace(&other, "schema_id: radishlex_pinyin", "schema_id: unsafe");
            replace(&other, "enable_user_dict: false", "enable_user_dict: true");
            replace(
                &root.join("user/build/default.yaml"),
                "schema: radishlex_pinyin",
                "schema: radishlex_pinyin\n  - schema: unsafe",
            );
            // The old preferred schema must never be initialized before the
            // adapter selects the caller's safe requested schema.
            fs::write(
                root.join("user/user.yaml"),
                "var:\n  previously_selected_schema: unsafe\n",
            )
            .unwrap();
        }
        "source-customization" => {
            fs::write(
                root.join("user/radishlex_pinyin.custom.yaml"),
                "patch:\n  translator/enable_user_dict: true\n",
            )
            .unwrap();
        }
        "pinned-configs" => {
            let other = root.join("user/build/safe_second.schema.yaml");
            fs::copy(&compiled, &other).unwrap();
            replace(
                &other,
                "schema_id: radishlex_pinyin",
                "schema_id: safe_second",
            );
            replace(
                &root.join("user/build/default.yaml"),
                "schema: radishlex_pinyin",
                "schema: radishlex_pinyin\n  - schema: safe_second",
            );
        }
        _ => panic!("unrecognized synthetic case"),
    }
}

fn check_pinned_configs(config: RimeEngineConfig, root: &Path) {
    let mut first = RimeEngine::new(config.clone()).unwrap();
    let expected = commit_synthetic(&mut first);
    // Simulate replaced deployed files, including a schema that has no active
    // engine yet. The running runtime must keep using its validated ConfigData.
    for name in [SCHEMA, "safe_second"] {
        replace(
            &root.join(format!("user/build/{name}.schema.yaml")),
            "enable_user_dict: false",
            "enable_user_dict: true",
        );
    }
    replace(
        &root.join("user/build/default.yaml"),
        "schema: safe_second",
        "schema: .unvalidated",
    );
    let mut second = RimeEngine::new(config.clone()).unwrap();
    assert_eq!(commit_synthetic(&mut second), expected);
    second.set_schema(schema("safe_second")).unwrap();
    assert_eq!(commit_synthetic(&mut second), expected);
    assert!(second.set_schema(schema(".unvalidated")).is_err());
    drop(second);
    drop(first);
    let mut after_gap = RimeEngine::new(config.clone()).unwrap();
    assert_eq!(commit_synthetic(&mut after_gap), expected);
    drop(after_gap);
    assert!(!root.join("user/pinyin_simp.userdb").exists());
    shutdown_process_runtime().unwrap();
    let error = RimeEngine::new(config).unwrap_err();
    assert!(error.to_string().contains("learning_guard"), "{error}");
    shutdown_process_runtime().unwrap();
}

fn commit_synthetic(engine: &mut RimeEngine) -> Vec<String> {
    for ch in "shi".chars() {
        engine.push_key(KeyEvent::press_char(ch)).unwrap();
    }
    let candidates = engine
        .candidates()
        .unwrap()
        .iter()
        .map(|candidate| candidate.text().to_owned())
        .collect::<Vec<_>>();
    assert!(candidates.len() > 1);
    let outcome = engine.select_candidate(1).unwrap();
    assert_eq!(outcome.commit().unwrap().text(), candidates[1]);
    candidates
}

fn replace(path: &Path, before: &str, after: &str) {
    let value = fs::read_to_string(path).unwrap();
    assert_eq!(value.matches(before).count(), 1, "{}", path.display());
    fs::write(path, value.replace(before, after)).unwrap();
}

fn schema(value: &str) -> SchemaId {
    SchemaId::new(value).unwrap()
}

fn private_dir(path: &Path) {
    fs::create_dir(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}
