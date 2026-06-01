# Ecosystem

Ecosystem is a Rust terminal renderer that turns system activity into a small ambient terminal world.

It is built for fun, visual experimentation, and smooth real-time terminal art. The goal is not to replace system monitors like `htop` or `btop`, but to make a lightweight terminal scene that feels alive while staying simple and inspectable.

## Current Shape

The project currently renders a Unicode-based landscape with:

- Layered sky, horizon, shoreline, water, and ground.
- Small block-based CPU creatures.
- Memory growth clusters.
- Network-driven water motion.
- Disk activity sparks.
- Diff-based ANSI rendering to avoid full-screen redraws.
- Basic resize handling and terminal validation.

The visual language is still evolving. The current focus is building a strong renderer foundation first, then improving the art direction incrementally.

## Run

From this directory:

```sh
cargo run
```

Quit with `q` or `Esc`.

A terminal of at least `80x24` is expected. Truecolor terminals such as Kitty, Ghostty, WezTerm, or modern xterm-compatible terminals are recommended.

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

- Keep the default renderer lightweight and Unicode-first.
- Prefer deterministic motion over random effects.
- Avoid harsh flicker.
- Keep redraws bounded through framebuffer diffing.
- Use dependencies carefully.
- Keep visual changes testable.

