pub fn assert_runtime_logs_redacted(log_text: &str) {
    assert_runtime_logs_redacted_without_conflict(log_text);
    assert!(
        log_text.contains(r#"route="devices.wrapped_epoch.get""#),
        "runtime log missing wrapped epoch route: {log_text}"
    );
    assert!(
        log_text.contains(r#"route="epoch_distributions.create""#),
        "runtime log missing epoch distribution route: {log_text}"
    );
    assert!(
        log_text.contains(r#"result_code="conflict_stale_base_version""#),
        "runtime log missing stale conflict: {log_text}"
    );
}

pub fn assert_runtime_logs_redacted_without_conflict(log_text: &str) {
    for forbidden in [
        "radish-alpha",
        "blocked-alpha",
        "ranker-alpha",
        "client-b-alpha",
        "input_code",
        "reading",
        "plaintext",
        "wrapping-key-device-b",
        "wrapping-key-device-a-2",
        "wrapping-key-device-c-2",
    ] {
        assert!(
            !log_text.contains(forbidden),
            "runtime log leaked {forbidden}: {log_text}"
        );
    }
    for required in [
        r#"route="domains.create""#,
        r#"route="join_requests.collection""#,
        r#"route="join_requests.authorize""#,
        r#"route="objects.versions.create""#,
        r#"route="objects.versions.get""#,
        r#"route="objects.versions.payload""#,
    ] {
        assert!(
            log_text.contains(required),
            "runtime log missing {required}: {log_text}"
        );
    }
}
