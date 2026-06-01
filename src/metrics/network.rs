use std::{error::Error, fmt, fs, io, time::Duration};

use crate::diagnostics::{TraceCollector, TraceEvent};

const PROC_NET_DEV_PATH: &str = "/proc/net/dev";
// Phase 2 uses a fixed soft cap instead of adaptive normalization so network
// behavior stays deterministic while the rest of the ecosystem pipeline forms.
const NETWORK_ACTIVITY_SOFT_CAP_BYTES_PER_SECOND: f32 = 2_000_000.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSample {
    pub interfaces: Vec<NetworkInterfaceCounters>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkInterfaceCounters {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetworkFlow {
    pub download: f32,
    pub upload: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NetworkSamplerStatus {
    Primed { interface_count: usize },
    Flow(NetworkFlow),
}

#[derive(Clone, Debug, Default)]
pub struct NetworkSampler {
    previous: Option<NetworkSample>,
}

impl NetworkSampler {
    pub fn sample_from_system(
        &mut self,
        elapsed: Duration,
        traces: &mut TraceCollector,
    ) -> Result<NetworkSamplerStatus, NetworkSamplerError> {
        let proc_net_dev = fs::read_to_string(PROC_NET_DEV_PATH)?;
        self.sample_from_proc_net_dev(&proc_net_dev, elapsed, traces)
    }

    pub fn sample_from_proc_net_dev(
        &mut self,
        input: &str,
        elapsed: Duration,
        traces: &mut TraceCollector,
    ) -> Result<NetworkSamplerStatus, NetworkSamplerError> {
        let current = parse_proc_net_dev(input)?;
        if self.previous.is_none() {
            let interface_count = current.interfaces.len();
            self.previous = Some(current);
            traces.record(TraceEvent::new(
                "metrics.network",
                format!("primed with {interface_count} interfaces"),
            ));
            return Ok(NetworkSamplerStatus::Primed { interface_count });
        }

        let previous = self
            .previous
            .as_ref()
            .expect("previous sample exists after priming check");
        let flow = calculate_network_flow(previous, &current, elapsed)?;
        traces.record(TraceEvent::new(
            "metrics.network",
            format!(
                "sampled flow down {:.2} up {:.2}",
                flow.download, flow.upload
            ),
        ));
        self.previous = Some(current);

        Ok(NetworkSamplerStatus::Flow(flow))
    }
}

pub fn parse_proc_net_dev(input: &str) -> Result<NetworkSample, NetworkParseError> {
    let mut interfaces = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || is_excluded_interface(name) {
            continue;
        }

        let mut parts = counters.split_whitespace();
        let rx_bytes = parse_counter(line_index + 1, parts.next())?;

        for _ in 0..7 {
            parts.next();
        }

        let tx_bytes = parse_counter(line_index + 1, parts.next())?;
        interfaces.push(NetworkInterfaceCounters {
            name: name.to_owned(),
            rx_bytes,
            tx_bytes,
        });
    }

    Ok(NetworkSample { interfaces })
}

pub fn calculate_network_flow(
    previous: &NetworkSample,
    current: &NetworkSample,
    elapsed: Duration,
) -> Result<NetworkFlow, NetworkFlowError> {
    if elapsed.is_zero() {
        return Ok(NetworkFlow {
            download: 0.0,
            upload: 0.0,
        });
    }

    let previous_rx = sum_rx(previous);
    let current_rx = sum_rx(current);
    let previous_tx = sum_tx(previous);
    let current_tx = sum_tx(current);

    let elapsed_seconds = elapsed.as_secs_f32();
    if elapsed_seconds <= 0.0 {
        return Err(NetworkFlowError::InvalidElapsed);
    }

    let download_bytes_per_second = current_rx.saturating_sub(previous_rx) as f32 / elapsed_seconds;
    let upload_bytes_per_second = current_tx.saturating_sub(previous_tx) as f32 / elapsed_seconds;

    Ok(NetworkFlow {
        download: normalize_network_rate(download_bytes_per_second),
        upload: normalize_network_rate(upload_bytes_per_second),
    })
}

fn parse_counter(line: usize, value: Option<&str>) -> Result<u64, NetworkParseError> {
    let value = value.ok_or(NetworkParseError::MissingCounter { line })?;
    value
        .parse::<u64>()
        .map_err(|_| NetworkParseError::InvalidCounter {
            line,
            value: value.to_owned(),
        })
}

fn is_excluded_interface(name: &str) -> bool {
    // Avoid letting loopback and common container/VM interfaces dominate the
    // ambient water signal. Physical, Wi-Fi, and most VPN interfaces remain in.
    name == "lo"
        || name.starts_with("docker")
        || name.starts_with("veth")
        || name.starts_with("br-")
        || name.starts_with("virbr")
        || name.starts_with("vmnet")
        || name.starts_with("vboxnet")
        || name.starts_with("ifb")
        || name.starts_with("dummy")
}

fn sum_rx(sample: &NetworkSample) -> u64 {
    sample
        .interfaces
        .iter()
        .map(|interface| interface.rx_bytes)
        .sum()
}

fn sum_tx(sample: &NetworkSample) -> u64 {
    sample
        .interfaces
        .iter()
        .map(|interface| interface.tx_bytes)
        .sum()
}

fn normalize_network_rate(bytes_per_second: f32) -> f32 {
    (bytes_per_second / NETWORK_ACTIVITY_SOFT_CAP_BYTES_PER_SECOND).clamp(0.0, 1.0)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkParseError {
    MissingCounter { line: usize },
    InvalidCounter { line: usize, value: String },
}

impl fmt::Display for NetworkParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCounter { line } => {
                write!(f, "missing network counter on line {line}")
            }
            Self::InvalidCounter { line, value } => {
                write!(f, "invalid network counter '{value}' on line {line}")
            }
        }
    }
}

impl Error for NetworkParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkFlowError {
    InvalidElapsed,
}

impl fmt::Display for NetworkFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidElapsed => write!(f, "network elapsed duration must be positive"),
        }
    }
}

impl Error for NetworkFlowError {}

#[derive(Debug)]
pub enum NetworkSamplerError {
    Io(io::Error),
    Parse(NetworkParseError),
    Flow(NetworkFlowError),
}

impl fmt::Display for NetworkSamplerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read {PROC_NET_DEV_PATH}: {error}"),
            Self::Parse(error) => error.fmt(f),
            Self::Flow(error) => error.fmt(f),
        }
    }
}

impl Error for NetworkSamplerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Flow(error) => Some(error),
        }
    }
}

impl From<io::Error> for NetworkSamplerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<NetworkParseError> for NetworkSamplerError {
    fn from(error: NetworkParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<NetworkFlowError> for NetworkSamplerError {
    fn from(error: NetworkFlowError) -> Self {
        Self::Flow(error)
    }
}
