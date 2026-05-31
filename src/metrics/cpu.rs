use std::{error::Error, fmt, fs, io};

use crate::diagnostics::{TraceCollector, TraceEvent};

const PROC_STAT_PATH: &str = "/proc/stat";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuSample {
    pub cores: Vec<CpuCoreCounters>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuCoreCounters {
    pub id: usize,
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuCoreCounters {
    fn total(self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }

    fn idle_total(self) -> u64 {
        self.idle + self.iowait
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CpuUsage {
    pub per_core: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CpuSamplerStatus {
    Primed { core_count: usize },
    Usage(CpuUsage),
}

#[derive(Clone, Debug, Default)]
pub struct CpuSampler {
    previous: Option<CpuSample>,
}

impl CpuSampler {
    pub fn sample_from_system(
        &mut self,
        traces: &mut TraceCollector,
    ) -> Result<CpuSamplerStatus, CpuSamplerError> {
        let proc_stat = fs::read_to_string(PROC_STAT_PATH)?;
        self.sample_from_proc_stat(&proc_stat, traces)
    }

    pub fn sample_from_proc_stat(
        &mut self,
        input: &str,
        traces: &mut TraceCollector,
    ) -> Result<CpuSamplerStatus, CpuSamplerError> {
        let current = parse_proc_stat(input)?;
        if self.previous.is_none() {
            let core_count = current.cores.len();
            self.previous = Some(current);
            traces.record(TraceEvent::new(
                "metrics.cpu",
                format!("primed with {core_count} cores"),
            ));
            return Ok(CpuSamplerStatus::Primed { core_count });
        }

        let previous = self
            .previous
            .as_ref()
            .expect("previous sample exists after priming check");
        let usage = calculate_cpu_usage(previous, &current)?;
        traces.record(TraceEvent::new(
            "metrics.cpu",
            format!("sampled usage for {} cores", usage.per_core.len()),
        ));
        self.previous = Some(current);

        Ok(CpuSamplerStatus::Usage(usage))
    }
}

pub fn parse_proc_stat(input: &str) -> Result<CpuSample, CpuParseError> {
    let mut cores = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        let Some(rest) = line.strip_prefix("cpu") else {
            continue;
        };

        // The aggregate line starts with "cpu " and must not become a fake core.
        if !rest.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
            continue;
        }

        let mut parts = rest.split_whitespace();
        let Some(id_text) = parts.next() else {
            continue;
        };

        if id_text.is_empty() || !id_text.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }

        let id = id_text
            .parse::<usize>()
            .map_err(|_| CpuParseError::InvalidCoreId {
                line: line_index + 1,
                value: id_text.to_owned(),
            })?;

        let mut counters = [0_u64; 8];
        for (counter_index, slot) in counters.iter_mut().enumerate() {
            let Some(value) = parts.next() else {
                return Err(CpuParseError::MissingCounter {
                    line: line_index + 1,
                    index: counter_index,
                });
            };
            *slot = value
                .parse::<u64>()
                .map_err(|_| CpuParseError::InvalidCounter {
                    line: line_index + 1,
                    value: value.to_owned(),
                })?;
        }

        cores.push(CpuCoreCounters {
            id,
            user: counters[0],
            nice: counters[1],
            system: counters[2],
            idle: counters[3],
            iowait: counters[4],
            irq: counters[5],
            softirq: counters[6],
            steal: counters[7],
        });
    }

    Ok(CpuSample { cores })
}

pub fn calculate_cpu_usage(
    previous: &CpuSample,
    current: &CpuSample,
) -> Result<CpuUsage, CpuUsageError> {
    if previous.cores.len() != current.cores.len() {
        return Err(CpuUsageError::CoreCountChanged {
            previous: previous.cores.len(),
            current: current.cores.len(),
        });
    }

    let mut per_core = Vec::with_capacity(current.cores.len());
    for (previous_core, current_core) in previous.cores.iter().zip(&current.cores) {
        let previous_total = previous_core.total();
        let current_total = current_core.total();
        let total_delta = current_total.saturating_sub(previous_total);

        if total_delta == 0 {
            per_core.push(0.0);
            continue;
        }

        let previous_idle = previous_core.idle_total();
        let current_idle = current_core.idle_total();
        // Linux reports CPU usage as monotonically increasing counters. Usage is
        // therefore a rate between two samples, with idle+iowait treated as
        // non-busy time so sleeping cores naturally normalize to 0.0.
        let idle_delta = current_idle.saturating_sub(previous_idle);
        let busy_delta = total_delta.saturating_sub(idle_delta);
        let usage = (busy_delta as f32 / total_delta as f32).clamp(0.0, 1.0);
        per_core.push(usage);
    }

    Ok(CpuUsage { per_core })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuParseError {
    InvalidCoreId { line: usize, value: String },
    MissingCounter { line: usize, index: usize },
    InvalidCounter { line: usize, value: String },
}

impl fmt::Display for CpuParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCoreId { line, value } => {
                write!(f, "invalid CPU core id '{value}' on line {line}")
            }
            Self::MissingCounter { line, index } => {
                write!(f, "missing CPU counter {index} on line {line}")
            }
            Self::InvalidCounter { line, value } => {
                write!(f, "invalid CPU counter '{value}' on line {line}")
            }
        }
    }
}

impl Error for CpuParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuUsageError {
    CoreCountChanged { previous: usize, current: usize },
}

impl fmt::Display for CpuUsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreCountChanged { previous, current } => write!(
                f,
                "CPU sample core count changed: previous {previous}, current {current}"
            ),
        }
    }
}

impl Error for CpuUsageError {}

#[derive(Debug)]
pub enum CpuSamplerError {
    Io(io::Error),
    Parse(CpuParseError),
    Usage(CpuUsageError),
}

impl fmt::Display for CpuSamplerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read {PROC_STAT_PATH}: {error}"),
            Self::Parse(error) => error.fmt(f),
            Self::Usage(error) => error.fmt(f),
        }
    }
}

impl Error for CpuSamplerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Usage(error) => Some(error),
        }
    }
}

impl From<io::Error> for CpuSamplerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<CpuParseError> for CpuSamplerError {
    fn from(error: CpuParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<CpuUsageError> for CpuSamplerError {
    fn from(error: CpuUsageError) -> Self {
        Self::Usage(error)
    }
}
