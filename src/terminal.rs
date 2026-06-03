use std::{
    error::Error,
    fmt,
    io::{self, Write},
};

pub const MIN_TERMINAL_WIDTH: u16 = 80;
pub const MIN_TERMINAL_HEIGHT: u16 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    pub width: u16,
    pub height: u16,
}

impl TerminalSize {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

impl From<(u16, u16)> for TerminalSize {
    fn from((width, height): (u16, u16)) -> Self {
        Self::new(width, height)
    }
}

pub fn current_terminal_size() -> std::io::Result<TerminalSize> {
    crossterm::terminal::size().map(TerminalSize::from)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalColorEnvironment {
    pub colorterm: Option<String>,
    pub term: Option<String>,
}

impl TerminalColorEnvironment {
    pub fn new(colorterm: Option<&str>, term: Option<&str>) -> Self {
        Self {
            colorterm: colorterm.map(str::to_owned),
            term: term.map(str::to_owned),
        }
    }

    pub fn from_process_env() -> Self {
        Self {
            colorterm: std::env::var("COLORTERM").ok(),
            term: std::env::var("TERM").ok(),
        }
    }
}

impl Default for TerminalColorEnvironment {
    fn default() -> Self {
        Self::from_process_env()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorCapability {
    Truecolor,
    Limited,
}

pub fn detect_color_capability(environment: &TerminalColorEnvironment) -> ColorCapability {
    let has_truecolor_marker = environment
        .colorterm
        .as_deref()
        .is_some_and(is_truecolor_marker)
        || environment.term.as_deref().is_some_and(is_truecolor_marker);

    if has_truecolor_marker {
        ColorCapability::Truecolor
    } else {
        ColorCapability::Limited
    }
}

fn is_truecolor_marker(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("truecolor") || value.contains("24bit") || value.contains("direct")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalValidationError {
    StdoutNotTerminal,
    TooSmall {
        actual: TerminalSize,
        minimum: TerminalSize,
    },
}

impl fmt::Display for TerminalValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StdoutNotTerminal => write!(
                f,
                "stdout is not a terminal; run ecosystem directly in an interactive terminal"
            ),
            Self::TooSmall { actual, minimum } => write!(
                f,
                "terminal is too small: got {}x{}, minimum is {}x{}",
                actual.width, actual.height, minimum.width, minimum.height
            ),
        }
    }
}

impl Error for TerminalValidationError {}

pub fn validate_terminal_environment(
    stdout_is_terminal: bool,
    size: TerminalSize,
) -> Result<(), TerminalValidationError> {
    if !stdout_is_terminal {
        return Err(TerminalValidationError::StdoutNotTerminal);
    }

    let minimum = TerminalSize::new(MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT);
    if size.width < minimum.width || size.height < minimum.height {
        return Err(TerminalValidationError::TooSmall {
            actual: size,
            minimum,
        });
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSessionOptions {
    pub enable_raw_mode: bool,
}

impl Default for TerminalSessionOptions {
    fn default() -> Self {
        Self {
            enable_raw_mode: true,
        }
    }
}

pub struct TerminalSession<W: Write> {
    writer: W,
    raw_mode_enabled: bool,
    active: bool,
}

impl<W: Write> TerminalSession<W> {
    pub fn start(mut writer: W, options: TerminalSessionOptions) -> io::Result<Self> {
        if options.enable_raw_mode {
            crossterm::terminal::enable_raw_mode()?;
        }

        writer.write_all(enter_alternate_screen())?;
        writer.write_all(hide_cursor())?;
        writer.write_all(clear_screen())?;
        writer.flush()?;

        Ok(Self {
            writer,
            raw_mode_enabled: options.enable_raw_mode,
            active: true,
        })
    }

    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    pub fn finish(mut self) -> io::Result<()> {
        self.restore()
    }

    fn restore(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }

        self.writer.write_all(reset_style())?;
        self.writer.write_all(show_cursor())?;
        self.writer.write_all(leave_alternate_screen())?;
        self.writer.flush()?;

        if self.raw_mode_enabled {
            crossterm::terminal::disable_raw_mode()?;
        }

        self.active = false;
        Ok(())
    }
}

impl<W: Write> Drop for TerminalSession<W> {
    fn drop(&mut self) {
        // Terminal restoration is best-effort during unwinding. Callers that
        // need to surface I/O errors should use `finish()` before dropping.
        let _ = self.restore();
    }
}

pub const fn enter_alternate_screen() -> &'static [u8] {
    b"\x1b[?1049h"
}

pub const fn leave_alternate_screen() -> &'static [u8] {
    b"\x1b[?1049l"
}

pub const fn hide_cursor() -> &'static [u8] {
    b"\x1b[?25l"
}

pub const fn show_cursor() -> &'static [u8] {
    b"\x1b[?25h"
}

pub const fn clear_screen() -> &'static [u8] {
    b"\x1b[2J\x1b[H"
}

pub const fn reset_style() -> &'static [u8] {
    b"\x1b[0m"
}
