use ecosystem::metrics::cpu::{calculate_cpu_usage, parse_proc_stat};

const PROC_STAT_PREVIOUS: &str = "\
cpu  100 0 50 1000 10 0 0 0 0 0
cpu0 40 0 10 500 5 0 0 0 0 0
cpu1 60 0 40 500 5 0 0 0 0 0
intr 1
ctxt 2
";

const PROC_STAT_CURRENT: &str = "\
cpu  130 0 70 1070 10 0 0 0 0 0
cpu0 50 0 20 530 5 0 0 0 0 0
cpu1 80 0 50 530 5 0 0 0 0 0
intr 3
ctxt 4
";

#[test]
fn proc_stat_parser_extracts_logical_cpu_core_counters_only() {
    let sample = parse_proc_stat(PROC_STAT_PREVIOUS).expect("valid proc stat fixture");

    assert_eq!(sample.cores.len(), 2);
    assert_eq!(sample.cores[0].id, 0);
    assert_eq!(sample.cores[0].user, 40);
    assert_eq!(sample.cores[0].system, 10);
    assert_eq!(sample.cores[0].idle, 500);
    assert_eq!(sample.cores[1].id, 1);
    assert_eq!(sample.cores[1].system, 40);
}

#[test]
fn cpu_usage_delta_treats_idle_and_iowait_as_not_busy() {
    let previous = parse_proc_stat(PROC_STAT_PREVIOUS).expect("valid previous sample");
    let current = parse_proc_stat(PROC_STAT_CURRENT).expect("valid current sample");

    let usage = calculate_cpu_usage(&previous, &current).expect("matching core samples");

    assert_eq!(usage.per_core.len(), 2);
    assert!((usage.per_core[0] - 0.40).abs() < f32::EPSILON);
    assert!((usage.per_core[1] - 0.50).abs() < f32::EPSILON);
}

#[test]
fn cpu_usage_delta_returns_zero_for_unchanged_counters() {
    let previous = parse_proc_stat(PROC_STAT_PREVIOUS).expect("valid previous sample");
    let current = parse_proc_stat(PROC_STAT_PREVIOUS).expect("valid current sample");

    let usage = calculate_cpu_usage(&previous, &current).expect("matching core samples");

    assert_eq!(usage.per_core, vec![0.0, 0.0]);
}

#[test]
fn cpu_usage_delta_rejects_changed_core_count() {
    let previous = parse_proc_stat(PROC_STAT_PREVIOUS).expect("valid previous sample");
    let current = parse_proc_stat("cpu0 50 0 20 530 5 0 0 0 0 0\n").expect("valid one-core sample");

    let error = calculate_cpu_usage(&previous, &current).expect_err("core count mismatch");

    assert_eq!(
        error.to_string(),
        "CPU sample core count changed: previous 2, current 1"
    );
}

#[test]
fn proc_stat_parser_reports_malformed_core_lines() {
    let error = parse_proc_stat("cpu0 10 20 nope 40\n").expect_err("invalid counter");

    assert!(
        error
            .to_string()
            .contains("invalid CPU counter 'nope' on line 1")
    );
}
