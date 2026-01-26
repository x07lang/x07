use x07_host_runner::{parse_metrics, parse_trap_stderr};

#[test]
fn parse_trap_stderr_handles_non_utf8() {
    let stderr = b"\xffoops\nos.threads.blocking disabled by policy\n{\"fuel_used\":1}\n";
    assert_eq!(
        parse_trap_stderr(stderr),
        Some("os.threads.blocking disabled by policy".to_string())
    );
}

#[test]
fn parse_metrics_handles_non_utf8() {
    let stderr = b"\xffoops\n{\"fuel_used\":7}\n";
    let metrics = parse_metrics(stderr).expect("metrics must parse");
    assert_eq!(metrics.fuel_used, Some(7));
}
