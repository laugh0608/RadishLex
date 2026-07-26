use std::env;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use radishlex_ime_sync_runtime::{QualificationRequest, QualificationRun, QualificationRunState};

const RUN_ENV: &str = "RADISHLEX_RUN_LOCAL_HTTPS_QUALIFICATION_TEST";
const ENDPOINT_ENV: &str = "RADISHLEX_LOCAL_HTTPS_ENDPOINT";
const ACCESS_TOKEN_ENV: &str = "RADISHLEX_LOCAL_HTTPS_ACCESS_TOKEN";
const ROOT_DER_PATH_ENV: &str = "RADISHLEX_LOCAL_HTTPS_ROOT_DER_PATH";

#[test]
fn manager_runner_completes_synthetic_two_client_https_qualification() {
    if env::var(RUN_ENV).as_deref() != Ok("1") {
        return;
    }

    let endpoint = required_env(ENDPOINT_ENV);
    let access_token = required_env(ACCESS_TOKEN_ENV);
    let root_der_path = required_env(ROOT_DER_PATH_ENV);
    let root_der = fs::read(root_der_path).expect("read temporary local HTTPS root certificate");
    let request = QualificationRequest::new(endpoint, access_token, Some(root_der), 30_000)
        .expect("qualification request");
    let run = QualificationRun::start(request).expect("start qualification run");
    let deadline = Instant::now() + Duration::from_secs(40);
    let snapshot = loop {
        let snapshot = run.poll();
        if snapshot.state.is_terminal() {
            break snapshot;
        }
        assert!(Instant::now() < deadline, "qualification did not finish");
        thread::sleep(Duration::from_millis(20));
    };

    assert_eq!(snapshot.state, QualificationRunState::Completed);
    assert!(snapshot.discovered >= 6);
    assert!(snapshot.downloaded >= 6);
    assert!(snapshot.applied >= 3);
    assert!(snapshot.uploaded >= 4);
    assert!(snapshot.conflicts >= 1);
    assert_eq!(snapshot.convergence_rounds, 2);
    assert!(snapshot.temporary_files_cleaned);
    assert!(snapshot.worker_stopped);
    assert!(snapshot.transient_inputs_cleared);
    assert!(snapshot.error.is_none());
}

fn required_env(name: &str) -> String {
    env::var(name)
        .unwrap_or_else(|_| panic!("required local HTTPS test environment is missing: {name}"))
}
