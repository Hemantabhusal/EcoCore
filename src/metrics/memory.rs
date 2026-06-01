use std::{error::Error, fmt, fs, io};

use crate::diagnostics::{TraceCollector, TraceEvent};

const PROC_MEMINFO_PATH: &str = "/proc/meminfo";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemorySample {
    pub total_kib: u64,
    pub available_kib: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemoryPressure {
    pub value: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemorySampler;

impl MemorySampler {
    pub fn sample_from_system(
        &mut self,
        traces: &mut TraceCollector,
    ) -> Result<MemoryPressure, MemorySamplerError> {
        let meminfo = fs::read_to_string(PROC_MEMINFO_PATH)?;
        self.sample_from_meminfo(&meminfo, traces)
    }

    pub fn sample_from_meminfo(
        &mut self,
        input: &str,
        traces: &mut TraceCollector,
    ) -> Result<MemoryPressure, MemorySamplerError> {
        let sample = parse_meminfo(input)?;
        let pressure = calculate_memory_pressure(&sample)?;
        traces.record(TraceEvent::new(
            "metrics.memory",
            format!(
                "sampled pressure {:.2} from {} KiB total, {} KiB available",
                pressure.value, sample.total_kib, sample.available_kib
            ),
        ));

        Ok(pressure)
    }
}

pub fn parse_meminfo(input: &str) -> Result<MemorySample, MemoryParseError> {
    let mut total_kib = None;
    let mut available_kib = None;

    for line in input.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        match key {
            "MemTotal" => total_kib = Some(parse_kib_value("MemTotal", rest)?),
            "MemAvailable" => available_kib = Some(parse_kib_value("MemAvailable", rest)?),
            _ => {}
        }
    }

    let total_kib = total_kib.ok_or(MemoryParseError::MissingField { field: "MemTotal" })?;
    let available_kib = available_kib.ok_or(MemoryParseError::MissingField {
        field: "MemAvailable",
    })?;

    Ok(MemorySample {
        total_kib,
        available_kib,
    })
}

pub fn calculate_memory_pressure(
    sample: &MemorySample,
) -> Result<MemoryPressure, MemoryPressureError> {
    if sample.total_kib == 0 {
        return Err(MemoryPressureError::ZeroTotalMemory);
    }

    // Linux MemAvailable includes reclaimable cache and buffers, so it avoids
    // showing healthy cached memory as pressure.
    let available_ratio = sample.available_kib as f32 / sample.total_kib as f32;
    let value = (1.0 - available_ratio).clamp(0.0, 1.0);
    Ok(MemoryPressure { value })
}

fn parse_kib_value(field: &'static str, rest: &str) -> Result<u64, MemoryParseError> {
    let value = rest
        .split_whitespace()
        .next()
        .ok_or(MemoryParseError::MissingValue { field })?;

    value
        .parse::<u64>()
        .map_err(|_| MemoryParseError::InvalidValue {
            field,
            value: value.to_owned(),
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryParseError {
    MissingField { field: &'static str },
    MissingValue { field: &'static str },
    InvalidValue { field: &'static str, value: String },
}

impl fmt::Display for MemoryParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { field } => write!(f, "missing {field} in {PROC_MEMINFO_PATH}"),
            Self::MissingValue { field } => {
                write!(f, "missing {field} value in {PROC_MEMINFO_PATH}")
            }
            Self::InvalidValue { field, value } => {
                write!(f, "invalid {field} value '{value}' in {PROC_MEMINFO_PATH}")
            }
        }
    }
}

impl Error for MemoryParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryPressureError {
    ZeroTotalMemory,
}

impl fmt::Display for MemoryPressureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTotalMemory => write!(f, "MemTotal must be greater than zero"),
        }
    }
}

impl Error for MemoryPressureError {}

#[derive(Debug)]
pub enum MemorySamplerError {
    Io(io::Error),
    Parse(MemoryParseError),
    Pressure(MemoryPressureError),
}

impl fmt::Display for MemorySamplerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read {PROC_MEMINFO_PATH}: {error}"),
            Self::Parse(error) => error.fmt(f),
            Self::Pressure(error) => error.fmt(f),
        }
    }
}

impl Error for MemorySamplerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Pressure(error) => Some(error),
        }
    }
}

impl From<io::Error> for MemorySamplerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<MemoryParseError> for MemorySamplerError {
    fn from(error: MemoryParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<MemoryPressureError> for MemorySamplerError {
    fn from(error: MemoryPressureError) -> Self {
        Self::Pressure(error)
    }
}
