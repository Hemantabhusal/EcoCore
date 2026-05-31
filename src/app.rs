use std::{error::Error, fmt};

use crate::{
    diagnostics::{TraceCollector, TraceEvent},
    framebuffer::{Cell, Framebuffer, FramebufferError},
    render::build_static_landscape_frame,
    terminal::{
        AnsiDiffEncoder, EncodeOutput, TerminalSize, TerminalValidationError,
        validate_terminal_environment,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupEnvironment {
    pub stdout_is_terminal: bool,
    pub terminal_size: TerminalSize,
}

impl StartupEnvironment {
    pub const fn new(stdout_is_terminal: bool, terminal_size: TerminalSize) -> Self {
        Self {
            stdout_is_terminal,
            terminal_size,
        }
    }
}

#[derive(Debug)]
pub enum AppError {
    Terminal(TerminalValidationError),
    Framebuffer(FramebufferError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Terminal(error) => error.fmt(f),
            Self::Framebuffer(error) => error.fmt(f),
        }
    }
}

impl Error for AppError {}

impl From<TerminalValidationError> for AppError {
    fn from(error: TerminalValidationError) -> Self {
        Self::Terminal(error)
    }
}

impl From<FramebufferError> for AppError {
    fn from(error: FramebufferError) -> Self {
        Self::Framebuffer(error)
    }
}

pub fn render_initial_frame(
    environment: StartupEnvironment,
    traces: &mut TraceCollector,
) -> Result<EncodeOutput, AppError> {
    traces.record(TraceEvent::new(
        "startup",
        format!(
            "validating terminal environment for {}x{}",
            environment.terminal_size.width, environment.terminal_size.height
        ),
    ));

    validate_terminal_environment(environment.stdout_is_terminal, environment.terminal_size)?;

    traces.record(TraceEvent::new(
        "startup",
        "building static Phase 1 MVP frame",
    ));

    let previous = Framebuffer::new(
        environment.terminal_size.width,
        environment.terminal_size.height,
        Cell::blank(),
    )?;
    let current = build_static_landscape_frame(
        environment.terminal_size.width,
        environment.terminal_size.height,
    )?;
    let encoded = AnsiDiffEncoder::new().encode_diff(&previous, &current)?;

    traces.record(TraceEvent::new(
        "render",
        format!("encoded {} changed cells", encoded.changed_cells),
    ));

    Ok(encoded)
}
