# Ecosystem

Ecosystem is a Rust terminal graphics experiment that turns system activity into a small ambient world.

It is built for fun, visual experimentation, and smooth real-time terminal art. The goal is not to replace system monitors like `htop` or `btop`, but to make a terminal scene that feels alive.

The current implementation is a Unicode renderer foundation. The long-term direction is a high-end graphics-capable terminal renderer using an internal pixel canvas and Kitty graphics protocol output.

## Current Shape

The project currently renders a Unicode-based landscape with:

- Layered sky, horizon, shoreline, water, and ground.
- Small block-based CPU creatures.
- Memory growth clusters.
- Network-driven water motion.
- Disk activity sparks.
- Diff-based ANSI rendering to avoid full-screen redraws.
- Basic resize handling and terminal validation.

The visual language is still evolving. Future work is moving away from direct glyph art toward a pixel-canvas renderer for modern graphics-capable terminals.

## Run

From this directory:

```sh
cargo run
```

Quit with `q` or `Esc`.

A terminal of at least `80x24` is expected for the current renderer.

Future high-end visuals will target modern terminals with Kitty graphics protocol support. GPU acceleration alone is not enough; the terminal must support an image/graphics protocol that the application can send frames to. Broad compatibility with old or limited terminals is not a project goal.

## Development

Useful checks:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

## Project Layout

```text
src/render.rs       Visual scene construction, glyphs, colors, motion
src/framebuffer.rs  Cell grid and color model
src/terminal.rs     ANSI diff encoder and terminal session handling
src/simulation.rs   Smoothed visual activity model
src/metrics/        CPU, memory, network, and disk sampling
src/main.rs         Runtime loop
tests/              Renderer, terminal, runtime, and metric behavior tests
```

## Design Principles

- Target high-end terminal visuals instead of broad terminal compatibility.
- Use a shared pixel canvas as the future art boundary.
- Treat Kitty graphics protocol output as the primary future backend.
- Do not add alternate render backends unless they solve a real implementation problem.
- Keep simulation/update rates separate from display FPS.
- Measure frame time, encode time, bytes sent, FPS, memory, and CPU.
- Prefer deterministic motion over random effects.
- Avoid harsh flicker.
- Keep redraws bounded through framebuffer diffing.
- Use dependencies carefully.
- Keep visual changes testable.
