# Ecosystem

Ecosystem is a Rust terminal graphics experiment that turns system activity into a small ambient world.

It is built for fun, visual experimentation, and smooth real-time terminal art. The goal is not to replace system monitors like `htop` or `btop`, but to make a terminal scene that feels alive.

The current implementation has moved past the old Unicode art renderer. It now keeps the terminal/runtime foundation and introduces the RGB/RGBA canvas boundary that future Kitty graphics output will use.

## Current Shape

The project currently contains:

- Terminal startup, cleanup, input, and resize handling.
- Linux CPU, memory, network, and disk sampling.
- Smoothed activity state for future visual systems.
- RGB/RGBA canvas storage with dirty-region tracking.
- A first Kitty graphics protocol spike that streams a generated RGBA canvas with explicit placement.
- Trace diagnostics for development and verification.

The current Kitty spike is intentionally simple: it proves canvas-to-terminal image output before final art systems are built.

## Run

From this directory:

```sh
cargo run
```

Quit with `q` or `Esc`.

A terminal of at least `80x24` is expected for the current renderer.

High-end visuals target modern terminals with Kitty graphics protocol support. GPU acceleration alone is not enough; the terminal must support an image/graphics protocol that the application can send frames to. Broad compatibility with old or limited terminals is not a project goal.

## Development

Useful checks:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

## Project Layout

```text
src/canvas.rs       RGB/RGBA pixel canvas and dirty-region tracking
src/kitty.rs        Kitty graphics protocol command encoding
src/layout.rs       Terminal image placement calculations
src/visual.rs       Temporary generated probe canvas
src/terminal.rs     Terminal session, validation, and control sequences
src/simulation.rs   Smoothed activity model for future visual systems
src/metrics/        CPU, memory, network, and disk sampling
src/main.rs         Runtime loop
tests/              Canvas, terminal, runtime, simulation, and metric tests
```

## Design Principles

- Target high-end terminal visuals instead of broad terminal compatibility.
- Use a shared pixel canvas as the future art boundary.
- Treat Kitty graphics protocol output as the primary future backend.
- Do not add alternate render backends unless they solve a real implementation problem.
- Keep simulation/update rates separate from display FPS.
- Measure frame time, encode time, bytes sent, FPS, memory, and CPU.
- Prefer measured protocol improvements over guessing about terminal throughput.
- Prefer deterministic motion over random effects.
- Avoid harsh flicker.
- Use dependencies carefully.
- Keep visual changes testable.
