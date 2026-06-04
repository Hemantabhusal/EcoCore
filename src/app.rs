use std::{error::Error, fmt};

use crate::{
    diagnostics::{TraceCollector, TraceEvent},
    terminal::{
        ColorCapability, TerminalColorEnvironment, TerminalGraphicsEnvironment, TerminalSize,
        TerminalValidationError, detect_color_capability, summarize_graphics_environment,
        validate_terminal_environment,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupEnvironment {
    pub stdout_is_terminal: bool,
    pub terminal_size: TerminalSize,
    pub color_environment: TerminalColorEnvironment,
    pub graphics_environment: TerminalGraphicsEnvironment,
}

impl StartupEnvironment {
    pub fn new(stdout_is_terminal: bool, terminal_size: TerminalSize) -> Self {
        Self {
            stdout_is_terminal,
            terminal_size,
            color_environment: TerminalColorEnvironment::default(),
            graphics_environment: TerminalGraphicsEnvironment::default(),
        }
    }

    pub fn with_color_environment(mut self, color_environment: TerminalColorEnvironment) -> Self {
        self.color_environment = color_environment;
        self
    }

    pub fn with_graphics_environment(
        mut self,
        graphics_environment: TerminalGraphicsEnvironment,
    ) -> Self {
        self.graphics_environment = graphics_environment;
        self
    }
}

#[derive(Debug)]
pub enum AppError {
    Terminal(TerminalValidationError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Terminal(error) => error.fmt(f),
        }
    }
}

impl Error for AppError {}

impl From<TerminalValidationError> for AppError {
    fn from(error: TerminalValidationError) -> Self {
        Self::Terminal(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupReport {
    pub terminal_size: TerminalSize,
    pub color_capability: ColorCapability,
}

pub fn prepare_startup(
    environment: StartupEnvironment,
    traces: &mut TraceCollector,
) -> Result<StartupReport, AppError> {
    traces.record(TraceEvent::new(
        "startup",
        format!(
            "validating terminal environment for {}x{}",
            environment.terminal_size.width, environment.terminal_size.height
        ),
    ));

    validate_terminal_environment(environment.stdout_is_terminal, environment.terminal_size)?;
    let color_capability = record_color_capability(&environment, traces);
    record_graphics_environment(&environment, traces);

    traces.record(TraceEvent::new(
        "startup",
        "startup validated for pixel canvas graphics runtime",
    ));

    Ok(StartupReport {
        terminal_size: environment.terminal_size,
        color_capability,
    })
}

fn record_color_capability(
    environment: &StartupEnvironment,
    traces: &mut TraceCollector,
) -> ColorCapability {
    let capability = detect_color_capability(&environment.color_environment);
    let message = match capability {
        ColorCapability::Truecolor => "truecolor capability detected".to_owned(),
        ColorCapability::Limited => {
            // Truecolor is not a hard startup requirement because some terminal
            // emulators approximate 24-bit ANSI color without advertising it.
            "limited color capability detected; truecolor is recommended".to_owned()
        }
    };

    traces.record(TraceEvent::new("terminal.color", message));
    capability
}

fn record_graphics_environment(environment: &StartupEnvironment, traces: &mut TraceCollector) {
    traces.record(TraceEvent::new(
        "terminal.graphics",
        summarize_graphics_environment(&environment.graphics_environment),
    ));
}
