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
- A first Kitty graphics protocol renderer that streams a generated RGBA canvas with explicit placement.
- Double-buffered Kitty image ids to reduce visible delete/recreate flicker.
- Renderer-side frame byte counters and protocol statistics for performance checks.
- Reused Kitty encode scratch buffers for RGBA packing and base64 output.
- Deadline-based frame pacing that preserves the 30 FPS target cadence and skips missed frame slots after overruns.
- A temporary layered probe scene with background, activity pulse, lifeform trail, lifeform seed, and flow tint layers.
- In-place activity smoothing to avoid per-frame activity buffer clones.
- Trace diagnostics for development and verification, including structured `graphics.frame` snapshots with measured FPS, skipped deadline counts, resize/suspend interruption markers, encode time, frame time, placement, image ids, and protocol bytes.

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

Trace mode is useful when checking Kitty graphics behavior:

```sh
ECOSYSTEM_TRACE=1 cargo run
```

For the current 240x135 probe canvas, local Kitty runs have usually held about 28-30 FPS outside resize-heavy windows, with roughly 173 KB/frame of protocol output. In `graphics.frame` traces, `skipped ... deadlines` indicates frame slots missed after an overrun, while `interrupted yes` usually means resize or suspend handling affected that measurement window.

## Project Layout

```text
src/canvas.rs       RGB/RGBA pixel canvas and dirty-region tracking
src/kitty.rs        Kitty graphics protocol command encoding
src/layout.rs       Terminal image placement calculations
src/renderer.rs     Stateful Kitty frame presentation
src/visual.rs       Temporary layered probe scene, lifeform state, and canvas composition
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
- Advance frame scheduling from previous frame deadlines instead of frame completion time.
- Measure frame time, encode time, bytes sent, FPS, memory, and CPU.
- Keep the frame pipeline allocation-conscious as scene complexity grows.
- Keep graphics measurement trace output stable enough to compare manual Kitty runs across visual changes.
- Defer dirty-region Kitty updates until visual layers stop repainting most of the canvas.
- Defer SIGINT/SIGTERM image cleanup until production hardening adds a signal handling dependency.
- Prefer measured protocol improvements over guessing about terminal throughput.
- Prefer deterministic motion over random effects.
- Avoid harsh flicker.
- Use dependencies carefully.
- Keep visual changes testable.
