use ecosystem::{
    diagnostics::TraceCollector,
    metrics::cpu::{CpuSampler, CpuSamplerStatus, calculate_cpu_usage, parse_proc_stat},
    metrics::disk::{
        DiskSampler, DiskSamplerStatus, calculate_disk_activity, parse_proc_diskstats,
    },
    metrics::memory::{MemorySampler, calculate_memory_pressure, parse_meminfo},
    metrics::network::{
        NetworkSampler, NetworkSamplerStatus, calculate_network_flow, parse_proc_net_dev,
    },
};
use std::time::Duration;

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

const MEMINFO_NORMAL: &str = "\
MemTotal:       1000000 kB
MemFree:         100000 kB
MemAvailable:    250000 kB
Buffers:          10000 kB
Cached:          200000 kB
";

const MEMINFO_PRESSURE: &str = "\
MemTotal:       1000000 kB
MemAvailable:     50000 kB
";

const PROC_NET_DEV_PREVIOUS: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1000 10 0 0 0 0 0 0 1000 10 0 0 0 0 0 0
docker0: 4000 10 0 0 0 0 0 0 5000 10 0 0 0 0 0 0
 enp3s0: 100000 20 0 0 0 0 0 0 50000 20 0 0 0 0 0 0
  wlan0: 300000 30 0 0 0 0 0 0 200000 30 0 0 0 0 0 0
";

const PROC_NET_DEV_CURRENT: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 9000 10 0 0 0 0 0 0 9000 10 0 0 0 0 0 0
docker0: 9000 10 0 0 0 0 0 0 9000 10 0 0 0 0 0 0
 enp3s0: 1100000 20 0 0 0 0 0 0 250000 20 0 0 0 0 0 0
  wlan0: 1300000 30 0 0 0 0 0 0 1100000 30 0 0 0 0 0 0
";

const PROC_DISKSTATS_PREVIOUS: &str = "\
   7       0 loop0 10 0 100 0 10 0 100 0 0 0 0 0 0 0 0 0 0
   8       0 sda 100 0 1000 0 50 0 500 0 0 0 0 0 0 0 0 0 0
   8       1 sda1 100 0 99999 0 50 0 99999 0 0 0 0 0 0 0 0 0 0
 259       0 nvme0n1 100 0 2000 0 50 0 1000 0 0 0 0 0 0 0 0 0 0
 259       1 nvme0n1p1 100 0 99999 0 50 0 99999 0 0 0 0 0 0 0 0 0 0
 254       0 dm-0 100 0 99999 0 50 0 99999 0 0 0 0 0 0 0 0 0 0
";

const PROC_DISKSTATS_CURRENT: &str = "\
   7       0 loop0 10 0 5000 0 10 0 5000 0 0 0 0 0 0 0 0 0 0
   8       0 sda 110 0 3048 0 60 0 1524 0 0 0 0 0 0 0 0 0 0
   8       1 sda1 110 0 199999 0 60 0 199999 0 0 0 0 0 0 0 0 0 0
 259       0 nvme0n1 110 0 4048 0 60 0 2024 0 0 0 0 0 0 0 0 0 0
 259       1 nvme0n1p1 110 0 199999 0 60 0 199999 0 0 0 0 0 0 0 0 0 0
 254       0 dm-0 110 0 199999 0 60 0 199999 0 0 0 0 0 0 0 0 0 0
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

#[test]
fn cpu_sampler_stores_first_sample_without_emitting_usage() {
    let mut sampler = CpuSampler::default();
    let mut traces = TraceCollector::enabled();

    let status = sampler
        .sample_from_proc_stat(PROC_STAT_PREVIOUS, &mut traces)
        .expect("first sample parses");

    assert_eq!(status, CpuSamplerStatus::Primed { core_count: 2 });
    assert!(
        traces
            .snapshot()
            .iter()
            .any(|event| event.message.contains("primed with 2 cores"))
    );
}

#[test]
fn cpu_sampler_emits_usage_after_second_sample() {
    let mut sampler = CpuSampler::default();
    let mut traces = TraceCollector::enabled();

    sampler
        .sample_from_proc_stat(PROC_STAT_PREVIOUS, &mut traces)
        .expect("first sample parses");
    let status = sampler
        .sample_from_proc_stat(PROC_STAT_CURRENT, &mut traces)
        .expect("second sample parses");

    let CpuSamplerStatus::Usage(usage) = status else {
        panic!("expected usage status");
    };
    assert_eq!(usage.per_core.len(), 2);
    assert!((usage.per_core[0] - 0.40).abs() < f32::EPSILON);
    assert!((usage.per_core[1] - 0.50).abs() < f32::EPSILON);
    assert!(
        traces
            .snapshot()
            .iter()
            .any(|event| event.message.contains("sampled usage for 2 cores"))
    );
}

#[test]
fn meminfo_parser_extracts_total_and_available_memory() {
    let sample = parse_meminfo(MEMINFO_NORMAL).expect("valid meminfo fixture");

    assert_eq!(sample.total_kib, 1_000_000);
    assert_eq!(sample.available_kib, 250_000);
}

#[test]
fn memory_pressure_uses_available_memory_instead_of_free_memory() {
    let sample = parse_meminfo(MEMINFO_NORMAL).expect("valid meminfo fixture");

    let pressure = calculate_memory_pressure(&sample).expect("valid pressure");

    assert!((pressure.value - 0.75).abs() < f32::EPSILON);
}

#[test]
fn memory_pressure_clamps_available_memory_above_total() {
    let sample = parse_meminfo(
        "\
MemTotal:       1000000 kB
MemAvailable:  1200000 kB
",
    )
    .expect("available above total still parses");

    let pressure = calculate_memory_pressure(&sample).expect("valid pressure");

    assert_eq!(pressure.value, 0.0);
}

#[test]
fn meminfo_parser_reports_missing_available_memory() {
    let error = parse_meminfo("MemTotal: 1000000 kB\n").expect_err("missing MemAvailable");

    assert_eq!(error.to_string(), "missing MemAvailable in /proc/meminfo");
}

#[test]
fn meminfo_parser_reports_invalid_numeric_values() {
    let error = parse_meminfo("MemTotal: nope kB\nMemAvailable: 1 kB\n")
        .expect_err("invalid MemTotal value");

    assert_eq!(
        error.to_string(),
        "invalid MemTotal value 'nope' in /proc/meminfo"
    );
}

#[test]
fn memory_sampler_emits_current_pressure_and_trace_event() {
    let mut sampler = MemorySampler;
    let mut traces = TraceCollector::enabled();

    let pressure = sampler
        .sample_from_meminfo(MEMINFO_PRESSURE, &mut traces)
        .expect("valid pressure sample");

    assert!((pressure.value - 0.95).abs() < f32::EPSILON);
    assert!(traces.snapshot().iter().any(|event| {
        event.target == "metrics.memory" && event.message.contains("sampled pressure 0.95")
    }));
}

#[test]
fn proc_net_dev_parser_filters_loopback_and_container_interfaces() {
    let sample = parse_proc_net_dev(PROC_NET_DEV_PREVIOUS).expect("valid network fixture");

    assert_eq!(sample.interfaces.len(), 2);
    assert_eq!(sample.interfaces[0].name, "enp3s0");
    assert_eq!(sample.interfaces[0].rx_bytes, 100_000);
    assert_eq!(sample.interfaces[0].tx_bytes, 50_000);
    assert_eq!(sample.interfaces[1].name, "wlan0");
}

#[test]
fn network_flow_delta_normalizes_download_and_upload_activity() {
    let previous = parse_proc_net_dev(PROC_NET_DEV_PREVIOUS).expect("valid previous sample");
    let current = parse_proc_net_dev(PROC_NET_DEV_CURRENT).expect("valid current sample");

    let flow =
        calculate_network_flow(&previous, &current, Duration::from_secs(1)).expect("valid flow");

    assert!((flow.download - 1.0).abs() < f32::EPSILON);
    assert!((flow.upload - 0.55).abs() < f32::EPSILON);
}

#[test]
fn network_sampler_primes_then_emits_flow_and_trace_event() {
    let mut sampler = NetworkSampler::default();
    let mut traces = TraceCollector::enabled();

    let status = sampler
        .sample_from_proc_net_dev(PROC_NET_DEV_PREVIOUS, Duration::ZERO, &mut traces)
        .expect("first sample parses");

    assert_eq!(status, NetworkSamplerStatus::Primed { interface_count: 2 });

    let status = sampler
        .sample_from_proc_net_dev(PROC_NET_DEV_CURRENT, Duration::from_secs(1), &mut traces)
        .expect("second sample parses");

    let NetworkSamplerStatus::Flow(flow) = status else {
        panic!("expected flow status");
    };
    assert!((flow.download - 1.0).abs() < f32::EPSILON);
    assert!((flow.upload - 0.55).abs() < f32::EPSILON);
    assert!(traces.snapshot().iter().any(|event| {
        event.target == "metrics.network"
            && event.message.contains("sampled flow down 1.00 up 0.55")
    }));
}

#[test]
fn proc_diskstats_parser_filters_partitions_and_virtual_devices() {
    let sample = parse_proc_diskstats(PROC_DISKSTATS_PREVIOUS).expect("valid diskstats fixture");

    assert_eq!(sample.devices.len(), 2);
    assert_eq!(sample.devices[0].name, "sda");
    assert_eq!(sample.devices[0].sectors_read, 1_000);
    assert_eq!(sample.devices[0].sectors_written, 500);
    assert_eq!(sample.devices[1].name, "nvme0n1");
}

#[test]
fn disk_activity_delta_normalizes_read_and_write_rates() {
    let previous = parse_proc_diskstats(PROC_DISKSTATS_PREVIOUS).expect("valid previous sample");
    let current = parse_proc_diskstats(PROC_DISKSTATS_CURRENT).expect("valid current sample");

    let activity =
        calculate_disk_activity(&previous, &current, Duration::from_secs(1)).expect("valid delta");

    assert!((activity.read - 1.0).abs() < f32::EPSILON);
    assert!((activity.write - 0.5).abs() < f32::EPSILON);
}

#[test]
fn disk_sampler_primes_then_emits_activity_and_trace_event() {
    let mut sampler = DiskSampler::default();
    let mut traces = TraceCollector::enabled();

    let status = sampler
        .sample_from_proc_diskstats(PROC_DISKSTATS_PREVIOUS, Duration::ZERO, &mut traces)
        .expect("first sample parses");

    assert_eq!(status, DiskSamplerStatus::Primed { device_count: 2 });

    let status = sampler
        .sample_from_proc_diskstats(PROC_DISKSTATS_CURRENT, Duration::from_secs(1), &mut traces)
        .expect("second sample parses");

    let DiskSamplerStatus::Activity(activity) = status else {
        panic!("expected disk activity status");
    };
    assert!((activity.read - 1.0).abs() < f32::EPSILON);
    assert!((activity.write - 0.5).abs() < f32::EPSILON);
    assert!(traces.snapshot().iter().any(|event| {
        event.target == "metrics.disk"
            && event
                .message
                .contains("sampled activity read 1.00 write 0.50")
    }));
}
