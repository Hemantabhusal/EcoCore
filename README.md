# Ecosystem

Ecosystem is a Rust terminal graphics experiment that turns live system activity
into a small ambient world.

It is not a replacement for `htop` or `btop`. The goal is a smooth, visual,
metric-influenced scene that feels alive without displaying raw numbers.

## Current Direction

The project targets modern terminals with Kitty graphics protocol support. It
uses an internal RGB/RGBA canvas, renders into that canvas, and streams the
result to the terminal through a Kitty graphics backend.

The next major visual direction is **Midnight Cat Cafe**: a macro-readable
pixel-art cafe scene with a large cat character, warm interior light, night
window rain, steam, and small metric-driven environmental motion.

Core art rule:

> Macro readability first, micro detail second.

The main planning context lives outside this crate in
`../Terminal_Project/project_direction.md` and
`../Terminal_Project/project_phase.md`. Those files should be read before
starting new visual or architecture work.

## Current Foundation

The project currently includes:

- Terminal startup, cleanup, input, and resize handling.
- Linux CPU, memory, network, and disk sampling.
- Smoothed activity state for visual systems.
- RGB/RGBA canvas storage with dirty-region tracking.
- Kitty graphics protocol encoding and stateful image presentation.
- RGBA PNG sprite loading and nearest-neighbor sprite blitting.
- A Midnight Cat Cafe runtime scene with a cached procedural background,
  warm/cool macro regions, asleep/idle/walk cat sprite states, and a measured
  larger `560x264` cafe canvas target.
- Full-frame and partial-frame protocol byte counters.
- Deadline-based 30 FPS frame pacing.
- Trace diagnostics for render, encode, write, frame time, FPS, skipped
  deadlines, image ids, placement, and protocol bytes.

## Run

From this directory:

```sh
cargo run
```

Quit with `q` or `Esc`.

A terminal of at least `80x24` is expected. High-end visuals require a modern
terminal with Kitty graphics protocol support; GPU acceleration alone is not
enough.

## Development

Useful checks:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Trace mode is useful when checking graphics behavior:

```sh
ECOSYSTEM_TRACE=1 cargo run
```

Trace output is the main way to compare visual changes. Use it to check FPS,
frame time, render time, encode time, terminal write time, full/partial bytes,
and dirty-region decisions before increasing canvas size or visual complexity.

## Project Layout

```text
src/canvas.rs       RGB/RGBA pixel canvas and dirty-region tracking
src/kitty.rs        Kitty graphics protocol command encoding
src/layout.rs       Terminal image placement calculations
src/renderer.rs     Stateful Kitty frame presentation
src/visual.rs       Public visual module exports
src/visual/         Cafe scene and generic scene helpers
assets/             Pixel-art assets and attribution notes
src/terminal.rs     Terminal session, validation, and control sequences
src/simulation.rs   Smoothed activity model
src/metrics/        CPU, memory, network, and disk sampling
src/main.rs         Runtime loop
tests/              Behavior and integration tests
```

## Design Principles

- Target high-end terminal visuals instead of broad terminal compatibility.
- Use a shared pixel canvas as the art boundary.
- Treat Kitty graphics protocol output as the primary backend.
- Keep simulation, rendering, encoding, and terminal output separable.
- Measure frame time, encode time, bytes sent, FPS, memory, and CPU.
- Prefer cached backgrounds and sparse animation for larger scenes.
- Use imported assets only with clear permission or license notes.
- Keep visual changes testable and traceable.
