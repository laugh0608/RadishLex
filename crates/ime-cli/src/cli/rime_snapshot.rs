#[cfg(unix)]
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use radishlex_ime_core::{Candidate, Engine, InputSession, KeyEvent, SessionState};
use radishlex_ime_ranker::CandidateExplanation;
use radishlex_ime_runtime::{PersonalizationStatus, PersonalizedInputSession, RuntimeSnapshot};

use super::CliError;

#[cfg(unix)]
pub(super) fn require_fresh_user_data(path: &Path) -> Result<PathBuf, CliError> {
    let absolute_path = absolute_path_without_parent_traversal(path)?;
    let account = authoritative_account()?;
    validate_user_data_path_components(&absolute_path, account.uid)?;

    let metadata = fs::symlink_metadata(&absolute_path).map_err(|error| {
        CliError::Data(format!(
            "rime snapshot user-data path must be an existing fresh empty directory: {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(CliError::Data(format!(
            "rime snapshot user-data path must not be a symlink: {}",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(CliError::Data(format!(
            "rime snapshot user-data path is not a directory: {}",
            path.display()
        )));
    }

    let canonical_path = fs::canonicalize(&absolute_path).map_err(|error| {
        CliError::Data(format!(
            "cannot resolve rime snapshot user-data directory {}: {error}",
            path.display()
        ))
    })?;
    let canonical_metadata = fs::symlink_metadata(&canonical_path).map_err(|error| {
        CliError::Data(format!(
            "cannot inspect canonical rime snapshot user-data directory {}: {error}",
            canonical_path.display()
        ))
    })?;
    require_same_directory_identity(&metadata, &canonical_metadata, &canonical_path)?;
    validate_private_directory_metadata(&canonical_metadata, account.uid, &canonical_path)?;

    reject_protected_user_data_path(&canonical_path, &account.home)?;

    let mut entries = fs::read_dir(&canonical_path).map_err(|error| {
        CliError::Data(format!(
            "cannot inspect rime snapshot user-data directory {}: {error}",
            canonical_path.display()
        ))
    })?;
    if entries
        .next()
        .transpose()
        .map_err(|error| {
            CliError::Data(format!(
                "cannot inspect rime snapshot user-data directory {}: {error}",
                canonical_path.display()
            ))
        })?
        .is_some()
    {
        return Err(CliError::Data(format!(
            "rime snapshot user-data directory must be empty before startup: {}",
            canonical_path.display()
        )));
    }

    let final_metadata = fs::symlink_metadata(&canonical_path).map_err(|error| {
        CliError::Data(format!(
            "cannot revalidate rime snapshot user-data directory {}: {error}",
            canonical_path.display()
        ))
    })?;
    require_same_directory_identity(&canonical_metadata, &final_metadata, &canonical_path)?;
    validate_private_directory_metadata(&final_metadata, account.uid, &canonical_path)?;
    Ok(canonical_path)
}

#[cfg(not(unix))]
pub(super) fn require_fresh_user_data(path: &Path) -> Result<PathBuf, CliError> {
    Err(CliError::Data(format!(
        "rime snapshot user-data safety checks are unsupported on this platform; refusing path {}",
        path.display()
    )))
}

#[cfg(unix)]
fn absolute_path_without_parent_traversal(path: &Path) -> Result<PathBuf, CliError> {
    use std::path::Component;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                CliError::Data(format!(
                    "cannot resolve the current directory for rime snapshot safety checks: {error}"
                ))
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::RootDir | Component::Normal(_) => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(CliError::Data(format!(
                    "rime snapshot user-data path must not contain parent traversal: {}",
                    path.display()
                )));
            }
            Component::Prefix(_) => {
                return Err(CliError::Data(format!(
                    "unsupported rime snapshot user-data path prefix: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(normalized)
}

#[cfg(unix)]
fn validate_user_data_path_components(path: &Path, expected_uid: u32) -> Result<(), CliError> {
    let mut ancestors: Vec<&Path> = path.ancestors().collect();
    ancestors.reverse();
    for component in ancestors {
        let metadata = fs::symlink_metadata(component).map_err(|error| {
            CliError::Data(format!(
                "cannot inspect rime snapshot user-data path component {}: {error}",
                component.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(CliError::Data(format!(
                "rime snapshot user-data path must not contain a symlink component: {}",
                component.display()
            )));
        }
        if component == path {
            continue;
        }
        if !metadata.is_dir() {
            return Err(CliError::Data(format!(
                "rime snapshot user-data ancestor is not a directory: {}",
                component.display()
            )));
        }

        let owner = metadata.uid();
        let mode = metadata.mode() & 0o7777;
        let group_or_other_writable = mode & 0o022 != 0;
        let root_owned_sticky = owner == 0 && mode & 0o1000 != 0;
        if owner == 0 && (!group_or_other_writable || root_owned_sticky) {
            continue;
        }
        if owner == expected_uid && !group_or_other_writable {
            continue;
        }
        if owner != 0 && owner != expected_uid {
            return Err(CliError::Data(format!(
                "rime snapshot user-data ancestor must be owned by root or the current user: {}",
                component.display()
            )));
        }
        return Err(CliError::Data(format!(
            "rime snapshot user-data ancestor must not be group/other writable unless it is root-owned and sticky: {}",
            component.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn require_same_directory_identity(
    expected: &fs::Metadata,
    actual: &fs::Metadata,
    path: &Path,
) -> Result<(), CliError> {
    if expected.dev() != actual.dev() || expected.ino() != actual.ino() {
        return Err(CliError::Data(format!(
            "rime snapshot user-data directory identity changed during validation: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_private_directory_metadata(
    metadata: &fs::Metadata,
    expected_uid: u32,
    path: &Path,
) -> Result<(), CliError> {
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(CliError::Data(format!(
            "rime snapshot user-data path is not a plain directory: {}",
            path.display()
        )));
    }
    if metadata.uid() != expected_uid {
        return Err(CliError::Data(format!(
            "rime snapshot user-data directory must be owned by the current user: {}",
            path.display()
        )));
    }
    if metadata.mode() & 0o7777 != 0o700 {
        return Err(CliError::Data(format!(
            "rime snapshot user-data directory must have mode 0700: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthoritativeAccount {
    uid: u32,
    home: PathBuf,
}

#[cfg(unix)]
fn authoritative_account() -> Result<AuthoritativeAccount, CliError> {
    let uid_output = run_trusted_account_command(Path::new("/usr/bin/id"), &["-u"])?;
    let uid = parse_uid(trim_account_record(&uid_output)?)?;
    let account_record = authoritative_account_record(uid)?;
    let fields: Vec<&[u8]> = trim_account_record(&account_record)?
        .split(|byte| *byte == b':')
        .collect();
    let (record_uid_index, home_index, expected_fields) = authoritative_account_record_layout()?;

    if fields.len() != expected_fields {
        return Err(CliError::Data(format!(
            "authoritative account record has {} fields; expected {expected_fields}",
            fields.len()
        )));
    }
    let record_uid = parse_uid(fields[record_uid_index])?;
    if record_uid != uid {
        return Err(CliError::Data(format!(
            "authoritative account record uid {record_uid} does not match current uid {uid}"
        )));
    }
    if fields[home_index].is_empty() {
        return Err(CliError::Data(format!(
            "authoritative account record for uid {uid} has an empty home directory"
        )));
    }
    let home = PathBuf::from(OsStr::from_bytes(fields[home_index]));
    if !home.is_absolute() {
        return Err(CliError::Data(format!(
            "authoritative account home is not absolute for uid {uid}: {}",
            home.display()
        )));
    }
    Ok(AuthoritativeAccount { uid, home })
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn authoritative_account_record_layout() -> Result<(usize, usize, usize), CliError> {
    Ok((2, 8, 10))
}

#[cfg(target_os = "linux")]
fn authoritative_account_record_layout() -> Result<(usize, usize, usize), CliError> {
    Ok((2, 5, 7))
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "linux"
)))]
fn authoritative_account_record_layout() -> Result<(usize, usize, usize), CliError> {
    Err(CliError::Data(
        "rime snapshot authoritative account record layout is unsupported on this Unix platform"
            .to_owned(),
    ))
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn authoritative_account_record(_uid: u32) -> Result<Vec<u8>, CliError> {
    run_trusted_account_command(Path::new("/usr/bin/id"), &["-P"])
}

#[cfg(target_os = "linux")]
fn authoritative_account_record(uid: u32) -> Result<Vec<u8>, CliError> {
    let program = [Path::new("/usr/bin/getent"), Path::new("/bin/getent")]
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            CliError::Data(
                "cannot query the authoritative account record: trusted getent is unavailable"
                    .to_owned(),
            )
        })?;
    run_trusted_account_command(program, &["passwd", &uid.to_string()])
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "linux"
)))]
fn authoritative_account_record(_uid: u32) -> Result<Vec<u8>, CliError> {
    Err(CliError::Data(
        "rime snapshot authoritative account lookup is unsupported on this Unix platform"
            .to_owned(),
    ))
}

#[cfg(unix)]
fn run_trusted_account_command(program: &Path, args: &[&str]) -> Result<Vec<u8>, CliError> {
    let output = Command::new(program)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            CliError::Data(format!(
                "cannot run trusted account lookup {}: {error}",
                program.display()
            ))
        })?;
    if !output.status.success() {
        return Err(CliError::Data(format!(
            "trusted account lookup {} failed with status {}",
            program.display(),
            output.status
        )));
    }
    Ok(output.stdout)
}

#[cfg(unix)]
fn trim_account_record(output: &[u8]) -> Result<&[u8], CliError> {
    let mut record = output;
    if let Some(without_newline) = record.strip_suffix(b"\n") {
        record = without_newline;
    }
    if let Some(without_carriage_return) = record.strip_suffix(b"\r") {
        record = without_carriage_return;
    }
    if record.is_empty() || record.contains(&b'\n') || record.contains(&b'\r') {
        return Err(CliError::Data(
            "trusted account lookup returned an empty or multiline record".to_owned(),
        ));
    }
    Ok(record)
}

#[cfg(unix)]
fn parse_uid(value: &[u8]) -> Result<u32, CliError> {
    let text = std::str::from_utf8(value).map_err(|_| {
        CliError::Data("trusted account lookup returned a non-ASCII uid".to_owned())
    })?;
    text.parse::<u32>().map_err(|error| {
        CliError::Data(format!(
            "trusted account lookup returned an invalid uid {text:?}: {error}"
        ))
    })
}

#[cfg(unix)]
fn reject_protected_user_data_path(path: &Path, home: &Path) -> Result<(), CliError> {
    let canonical_home = fs::canonicalize(home).map_err(|error| {
        CliError::Data(format!(
            "cannot resolve the authoritative account home {}: {error}",
            home.display()
        ))
    })?;
    let protected_roots = [
        canonical_home.join("Library/Rime"),
        canonical_home.join("Library/Input Methods"),
        canonical_home.join("Library/Application Support/Squirrel"),
        canonical_home.join("Library/Application Support/RadishLex/Rime"),
        canonical_home.join("Application Support/Squirrel"),
        canonical_home.join("RadishLex/Rime"),
    ];

    for root in protected_roots {
        if path == root || path.starts_with(&root) {
            return Err(CliError::Data(format!(
                "rime snapshot user-data path is inside a protected real-user location: {}",
                path.display()
            )));
        }
        match fs::canonicalize(&root) {
            Ok(canonical_root) if path == canonical_root || path.starts_with(&canonical_root) => {
                return Err(CliError::Data(format!(
                    "rime snapshot user-data path is inside a protected real-user location: {}",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(CliError::Data(format!(
                    "cannot resolve protected real-user location {}: {error}",
                    root.display()
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn run_engine_snapshot<E: Engine>(
    mut session: InputSession<E>,
    input_code: &str,
    deploy_on_start: bool,
) -> Result<String, CliError> {
    for ch in input_code.chars() {
        let outcome = session.push_key(KeyEvent::press_char(ch))?;
        if outcome.commit().is_some() {
            return Err(CliError::Data(
                "engine committed while entering the snapshot input; the result cannot be used as non-commit evidence"
                    .to_owned(),
            ));
        }
    }

    let state = session.state()?;
    Ok(render_engine_snapshot(input_code, &state, deploy_on_start))
}

pub(super) fn run_personalized_snapshot<E: Engine>(
    mut session: PersonalizedInputSession<E>,
    input_code: &str,
    deploy_on_start: bool,
    context_kind: &str,
) -> Result<String, CliError> {
    for ch in input_code.chars() {
        let result = session.handle_key(KeyEvent::press_char(ch))?;
        if result.outcome().commit().is_some() {
            return Err(CliError::Data(
                "engine committed while entering the snapshot input; the result cannot be used as non-commit evidence"
                    .to_owned(),
            ));
        }
    }

    let snapshot = session.snapshot()?;
    Ok(render_runtime_snapshot(
        input_code,
        &snapshot,
        deploy_on_start,
        context_kind,
    ))
}

fn render_engine_snapshot(input_code: &str, state: &SessionState, deploy_on_start: bool) -> String {
    let mut output = String::new();
    output.push_str("rime_snapshot: ready\n");
    output.push_str("user_data_isolation: fresh_empty_at_start\n");
    output.push_str(&format!(
        "deploy_on_start: {}\n",
        zero_or_one(deploy_on_start)
    ));
    output.push_str(&format!("schema: {}\n", state.schema().as_str()));
    output.push_str(&format!("input: {input_code}\n"));
    output.push_str(&format!("composition: {}\n", state.composition().preedit()));
    output.push_str("personalization_status: not_requested\n");
    output.push_str("rank_context: <none>\n");
    output.push_str("candidates:\n");
    if state.candidates().is_empty() {
        output.push_str("  <none>\n");
    } else {
        for (index, candidate) in state.candidates().iter().enumerate() {
            output.push_str(&format_snapshot_candidate(
                index, index, candidate, None, None,
            ));
        }
    }
    output.push_str("selection_api_called: false\n");
    output.push_str("commit_observed: false\n");
    output
}

fn render_runtime_snapshot(
    input_code: &str,
    snapshot: &RuntimeSnapshot,
    deploy_on_start: bool,
    context_kind: &str,
) -> String {
    let mut output = String::new();
    output.push_str("rime_snapshot: ready\n");
    output.push_str("user_data_isolation: fresh_empty_at_start\n");
    output.push_str(&format!(
        "deploy_on_start: {}\n",
        zero_or_one(deploy_on_start)
    ));
    output.push_str(&format!("schema: {}\n", snapshot.schema().as_str()));
    output.push_str(&format!("input: {input_code}\n"));
    output.push_str(&format!(
        "composition: {}\n",
        snapshot.composition().preedit()
    ));
    output.push_str(&format!(
        "personalization_status: {}\n",
        personalization_status_label(snapshot.personalization_status())
    ));
    output.push_str(&format!("rank_context: {context_kind}\n"));
    output.push_str("candidates:\n");
    if snapshot.candidates().is_empty() {
        output.push_str("  <none>\n");
    } else {
        for candidate in snapshot.candidates() {
            output.push_str(&format_snapshot_candidate(
                candidate.display_index(),
                candidate.engine_index(),
                candidate.candidate(),
                candidate.final_score(),
                candidate.explanation(),
            ));
        }
    }
    output.push_str("selection_api_called: false\n");
    output.push_str("commit_observed: false\n");
    output
}

fn format_snapshot_candidate(
    display_index: usize,
    engine_index: usize,
    candidate: &Candidate,
    final_score: Option<f64>,
    explanation: Option<&CandidateExplanation>,
) -> String {
    let reading = candidate
        .reading()
        .map(|value| format!(" [{value}]"))
        .unwrap_or_default();
    let annotation = candidate
        .annotation()
        .map(|value| format!(" - {value}"))
        .unwrap_or_default();
    let score = final_score
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "<none>".to_owned());
    let mut output = format!(
        "  {display_index}. {}{reading}{annotation} (display_index={display_index} engine_index={engine_index} score={score})\n",
        candidate.text()
    );
    if let Some(explanation) = explanation {
        output.push_str(&format!(
            "     explain: engine_order={:.3} user_term={:.3} frequency={:.3} recency={:.3} context={:.3} negative={:.3} suppressed={:.3} deleted={:.3}\n",
            explanation.engine_order_factor,
            explanation.user_term_boost,
            explanation.frequency_boost,
            explanation.recency_boost,
            explanation.context_boost,
            explanation.negative_feedback_penalty,
            explanation.suppressed_penalty,
            explanation.deleted_penalty
        ));
    } else {
        output.push_str("     explain: <none>\n");
    }
    output
}

fn personalization_status_label(status: PersonalizationStatus) -> &'static str {
    match status {
        PersonalizationStatus::Ready => "ready",
        PersonalizationStatus::PolicyBlocked => "policy_blocked",
        PersonalizationStatus::StorageUnavailable => "storage_unavailable",
        PersonalizationStatus::ReadFailed => "read_failed",
        PersonalizationStatus::RankFailed => "rank_failed",
    }
}

fn zero_or_one(value: bool) -> u8 {
    u8::from(value)
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        authoritative_account, reject_protected_user_data_path, require_fresh_user_data,
        validate_private_directory_metadata,
    };

    static ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

    struct HomeEnvironmentRestore(Option<OsString>);

    fn unique_directory(base: &Path, label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        base.join(format!(
            "radishlex-rime-snapshot-{label}-{}-{timestamp}",
            std::process::id()
        ))
    }

    fn create_directory_with_mode(path: &Path, mode: u32) {
        fs::create_dir(path).expect("fixture directory is created");
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .expect("fixture directory mode is set");
    }

    impl Drop for HomeEnvironmentRestore {
        fn drop(&mut self) {
            if let Some(value) = self.0.take() {
                std::env::set_var("HOME", value);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn protected_real_user_roots_and_descendants_are_rejected() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let home = std::env::temp_dir().join(format!(
            "radishlex-rime-snapshot-home-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir_all(&home).expect("synthetic home is created");
        let relative_roots = [
            PathBuf::from("Library/Rime"),
            PathBuf::from("Library/Input Methods"),
            PathBuf::from("Library/Application Support/Squirrel"),
            PathBuf::from("Library/Application Support/RadishLex/Rime"),
            PathBuf::from("Application Support/Squirrel"),
            PathBuf::from("RadishLex/Rime"),
        ];

        for relative_root in relative_roots {
            let candidate = home.join(relative_root).join("isolated-looking-child");
            fs::create_dir_all(&candidate).expect("protected descendant is created");
            let candidate = fs::canonicalize(candidate).expect("candidate canonicalizes");
            let error = reject_protected_user_data_path(&candidate, &home)
                .expect_err("protected descendant must be rejected");
            assert!(error.to_string().contains("protected real-user location"));
        }

        let allowed = home.join("tmp/rime-snapshot-user");
        fs::create_dir_all(&allowed).expect("allowed directory is created");
        let allowed = fs::canonicalize(allowed).expect("allowed path canonicalizes");
        reject_protected_user_data_path(&allowed, &home)
            .expect("directory outside protected roots is accepted");

        fs::remove_dir_all(home).expect("synthetic home is removed");
    }

    #[test]
    fn forged_home_environment_does_not_change_the_authoritative_home_or_protection() {
        let _lock = ENVIRONMENT_LOCK.lock().expect("environment lock");
        let authoritative = authoritative_account().expect("account home is available");
        let forged_home = authoritative.home.join("forged-rime-snapshot-home");
        let _restore = HomeEnvironmentRestore(std::env::var_os("HOME"));
        std::env::set_var("HOME", &forged_home);

        let resolved = authoritative_account().expect("account lookup ignores HOME");
        assert_eq!(resolved, authoritative);
        assert_ne!(resolved.home, forged_home);

        let protected = fs::canonicalize(&authoritative.home)
            .expect("account home resolves")
            .join("Library/Rime/forged-home-must-not-unprotect");
        let error = reject_protected_user_data_path(&protected, &resolved.home)
            .expect_err("real account Rime path remains protected");
        assert!(error.to_string().contains("protected real-user location"));
    }

    #[test]
    fn directory_owner_check_rejects_a_non_current_expected_owner() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let temp = fs::canonicalize(std::env::temp_dir()).expect("temp directory resolves");
        let directory = temp.join(format!(
            "radishlex-rime-snapshot-owner-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("owner fixture is created");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("owner fixture mode is private");
        let metadata = fs::symlink_metadata(&directory).expect("owner fixture is inspected");
        let wrong_uid = metadata.uid().checked_add(1).unwrap_or(metadata.uid() - 1);

        let error = validate_private_directory_metadata(&metadata, wrong_uid, &directory)
            .expect_err("mismatched owner must be rejected");
        assert!(error.to_string().contains("owned by the current user"));

        fs::remove_dir(directory).expect("owner fixture is removed");
    }

    #[test]
    fn non_sticky_world_writable_ancestor_is_rejected() {
        let temp = fs::canonicalize(std::env::temp_dir()).expect("temp directory resolves");
        let unsafe_parent = unique_directory(&temp, "unsafe-parent");
        create_directory_with_mode(&unsafe_parent, 0o777);
        let user_data = unsafe_parent.join("fresh-user");
        create_directory_with_mode(&user_data, 0o700);

        let error = require_fresh_user_data(&user_data)
            .expect_err("non-sticky world-writable ancestor must be rejected");
        assert!(error
            .to_string()
            .contains("must not be group/other writable"));

        fs::remove_dir(user_data).expect("user-data fixture is removed");
        fs::remove_dir(unsafe_parent).expect("unsafe parent fixture is removed");
    }

    #[test]
    fn current_user_0700_and_0755_ancestor_chain_is_accepted() {
        let temp = fs::canonicalize(std::env::temp_dir()).expect("temp directory resolves");
        let private_parent = unique_directory(&temp, "private-parent");
        create_directory_with_mode(&private_parent, 0o700);
        let readable_parent = private_parent.join("readable-parent");
        create_directory_with_mode(&readable_parent, 0o755);
        let user_data = readable_parent.join("fresh-user");
        create_directory_with_mode(&user_data, 0o700);

        let canonical = require_fresh_user_data(&user_data)
            .expect("current-user non-writable ancestor chain is safe");
        assert_eq!(
            canonical,
            fs::canonicalize(&user_data).expect("path resolves")
        );

        fs::remove_dir(user_data).expect("user-data fixture is removed");
        fs::remove_dir(readable_parent).expect("readable parent fixture is removed");
        fs::remove_dir(private_parent).expect("private parent fixture is removed");
    }

    #[test]
    fn root_owned_sticky_system_temp_root_is_accepted() {
        let system_temp = [Path::new("/private/tmp"), Path::new("/tmp")]
            .into_iter()
            .filter_map(|path| fs::canonicalize(path).ok())
            .find(|path| {
                fs::symlink_metadata(path)
                    .is_ok_and(|metadata| metadata.uid() == 0 && metadata.mode() & 0o1000 != 0)
            })
            .expect("a root-owned sticky system temporary directory is available");
        let user_data = unique_directory(&system_temp, "system-temp");
        create_directory_with_mode(&user_data, 0o700);

        let canonical = require_fresh_user_data(&user_data)
            .expect("root-owned sticky system temp root is safe");
        assert_eq!(
            canonical,
            fs::canonicalize(&user_data).expect("path resolves")
        );

        fs::remove_dir(user_data).expect("system temp fixture is removed");
    }
}
