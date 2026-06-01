use std::{error::Error, fmt, fs, io, time::Duration};

use crate::diagnostics::{TraceCollector, TraceEvent};

const PROC_DISKSTATS_PATH: &str = "/proc/diskstats";
const DISK_ACTIVITY_SOFT_CAP_SECTORS_PER_SECOND: f32 = 4_096.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskSample {
    pub devices: Vec<DiskDeviceCounters>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiskDeviceCounters {
    pub name: String,
    pub sectors_read: u64,
    pub sectors_written: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiskActivity {
    pub read: f32,
    pub write: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiskSamplerStatus {
    Primed { device_count: usize },
    Activity(DiskActivity),
}

#[derive(Clone, Debug, Default)]
pub struct DiskSampler {
    previous: Option<DiskSample>,
}

impl DiskSampler {
    pub fn sample_from_system(
        &mut self,
        elapsed: Duration,
        traces: &mut TraceCollector,
    ) -> Result<DiskSamplerStatus, DiskSamplerError> {
        let diskstats = fs::read_to_string(PROC_DISKSTATS_PATH)?;
        self.sample_from_proc_diskstats(&diskstats, elapsed, traces)
    }

    pub fn sample_from_proc_diskstats(
        &mut self,
        input: &str,
        elapsed: Duration,
        traces: &mut TraceCollector,
    ) -> Result<DiskSamplerStatus, DiskSamplerError> {
        let current = parse_proc_diskstats(input)?;
        if self.previous.is_none() {
            let device_count = current.devices.len();
            self.previous = Some(current);
            traces.record(TraceEvent::new(
                "metrics.disk",
                format!("primed with {device_count} devices"),
            ));
            return Ok(DiskSamplerStatus::Primed { device_count });
        }

        let previous = self
            .previous
            .as_ref()
            .expect("previous sample exists after priming check");
        let activity = calculate_disk_activity(previous, &current, elapsed)?;
        traces.record(TraceEvent::new(
            "metrics.disk",
            format!(
                "sampled activity read {:.2} write {:.2}",
                activity.read, activity.write
            ),
        ));
        self.previous = Some(current);

        Ok(DiskSamplerStatus::Activity(activity))
    }
}

pub fn parse_proc_diskstats(input: &str) -> Result<DiskSample, DiskParseError> {
    let mut devices = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 10 {
            continue;
        }

        let name = parts[2];
        if is_excluded_device(name) {
            continue;
        }

        let sectors_read = parse_counter(line_index + 1, parts[5])?;
        let sectors_written = parse_counter(line_index + 1, parts[9])?;
        devices.push(DiskDeviceCounters {
            name: name.to_owned(),
            sectors_read,
            sectors_written,
        });
    }

    Ok(DiskSample { devices })
}

pub fn calculate_disk_activity(
    previous: &DiskSample,
    current: &DiskSample,
    elapsed: Duration,
) -> Result<DiskActivity, DiskActivityError> {
    if elapsed.is_zero() {
        return Ok(DiskActivity {
            read: 0.0,
            write: 0.0,
        });
    }

    let elapsed_seconds = elapsed.as_secs_f32();
    if elapsed_seconds <= 0.0 {
        return Err(DiskActivityError::InvalidElapsed);
    }

    let read_sectors_per_second =
        sum_read(current).saturating_sub(sum_read(previous)) as f32 / elapsed_seconds;
    let write_sectors_per_second =
        sum_written(current).saturating_sub(sum_written(previous)) as f32 / elapsed_seconds;

    Ok(DiskActivity {
        read: normalize_disk_rate(read_sectors_per_second),
        write: normalize_disk_rate(write_sectors_per_second),
    })
}

fn parse_counter(line: usize, value: &str) -> Result<u64, DiskParseError> {
    value
        .parse::<u64>()
        .map_err(|_| DiskParseError::InvalidCounter {
            line,
            value: value.to_owned(),
        })
}

fn is_excluded_device(name: &str) -> bool {
    // Keep the signal focused on whole physical-ish block devices. Partitions
    // and mapper layers often duplicate the same I/O and make the scene jumpy.
    is_partition(name)
        || name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("zram")
        || name.starts_with("fd")
        || name.starts_with("sr")
        || name.starts_with("dm-")
        || name.starts_with("md")
        || name.starts_with("nbd")
}

fn is_partition(name: &str) -> bool {
    if name.starts_with("nvme") || name.starts_with("mmcblk") {
        return name
            .rsplit_once('p')
            .is_some_and(|(_, suffix)| suffix.chars().all(|ch| ch.is_ascii_digit()));
    }

    name.chars().last().is_some_and(|ch| ch.is_ascii_digit())
}

fn sum_read(sample: &DiskSample) -> u64 {
    sample
        .devices
        .iter()
        .map(|device| device.sectors_read)
        .sum()
}

fn sum_written(sample: &DiskSample) -> u64 {
    sample
        .devices
        .iter()
        .map(|device| device.sectors_written)
        .sum()
}

fn normalize_disk_rate(sectors_per_second: f32) -> f32 {
    (sectors_per_second / DISK_ACTIVITY_SOFT_CAP_SECTORS_PER_SECOND).clamp(0.0, 1.0)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiskParseError {
    InvalidCounter { line: usize, value: String },
}

impl fmt::Display for DiskParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCounter { line, value } => {
                write!(f, "invalid disk counter '{value}' on line {line}")
            }
        }
    }
}

impl Error for DiskParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiskActivityError {
    InvalidElapsed,
}

impl fmt::Display for DiskActivityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidElapsed => write!(f, "disk elapsed duration must be positive"),
        }
    }
}

impl Error for DiskActivityError {}

#[derive(Debug)]
pub enum DiskSamplerError {
    Io(io::Error),
    Parse(DiskParseError),
    Activity(DiskActivityError),
}

impl fmt::Display for DiskSamplerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read {PROC_DISKSTATS_PATH}: {error}"),
            Self::Parse(error) => error.fmt(f),
            Self::Activity(error) => error.fmt(f),
        }
    }
}

impl Error for DiskSamplerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Activity(error) => Some(error),
        }
    }
}

impl From<io::Error> for DiskSamplerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<DiskParseError> for DiskSamplerError {
    fn from(error: DiskParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<DiskActivityError> for DiskSamplerError {
    fn from(error: DiskActivityError) -> Self {
        Self::Activity(error)
    }
}
