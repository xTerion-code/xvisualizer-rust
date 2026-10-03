# AGENTS.md

Single-crate Rust TUI app (`edition 2024`): terminal ASCII spectrum visualizer capturing system audio via PulseAudio monitor. No workspace, no CI, no existing lint/format config.

## Commands

- Build: `cargo build` — `cargo run --release` for real use (debug build works but frame timing/noise differs).
- Verify: `cargo clippy --all-targets` (repo standard is zero warnings), then `cargo test`.
- Format: `cargo fmt --check` (default rustfmt, no custom config).
- Run with args after `--`: `cargo run --release -- --device <monitor> --theme solid --no-color`.
- Only tests are pure `pactl` output parsers in `src/source.rs`: `cargo test source` runs them without audio hardware.

## Code rules

- Keep code modular: one file per feature/module (existing split: `args` / `audio` / `dsp` / `color` / `theme` / `ui` / `input` / `menu` / `visualizer` / `help` / `frame` / `source` / `solo` / `terminal`). New features get their own file wired through `main.rs`, not appended to an unrelated module.
- Comments: minimal but practical — only where the logic is non-obvious (timing constants, PulseAudio quirks, render escape sequences). No comment per line, no restating what the code says.
- English only: all code, identifiers, comments, CLI output, and docs strictly in English.

## Gotchas

- `src/audio.rs` and solo-capture in `src/solo.rs` shell out to `pactl`/`pactl`-managed Pulse/PipeWire and need a live Linux audio stack. Do not try to verify capture, solo rerouting (`SoloSession` null-sink + loopback), or rendering headlessly — there is no mock/fixture for them.
- On Pulse failure `capture_loop` sets the global `running` flag false, which shuts down the whole app; treat audio errors as fatal-by-design, not something to "retry in place".
- `CaptureSession::switch()` joins the old worker thread and clears the queue before spawning a new one. Keep that order; reusing the queue without clearing plays stale audio.
- Frame loop in `src/main.rs`: single buffered `write` + `flush` per frame at ~120 Hz, `dt` clamped to `0.001–0.05` s. Preserve both when touching timing/smoothing.
- DSP constants live in `src/dsp.rs` (`WINDOW=2048`, `RATE=48000`, `READ_FRAMES=512`, `F_MIN/F_MAX 30Hz–16kHz`, `GAMMA 0.6`); audio chunk sizes in `src/audio.rs` (`fragsize = READ_FRAMES*4`, `QUEUE_CAP=8192`). These are coupled — changing one without the other breaks latency math documented in `README.md`.
- Terminal state is RAII via `terminal::TerminalGuard` + `terminal::enter()` (alternate screen + raw mode). Any early return must go through it or the terminal is left broken.
- Key handling is split: `input.rs` (key decoding per screen), state in `ui.rs` (`UiState` / `UiEvent`), screens in `menu.rs` / `visualizer.rs` / `help.rs`, bar colors in `color.rs` (`bar_color`), theme geometry in `theme.rs` (`dims`). Add a theme by extending `theme.rs` + `visualizer.rs` (`PARTS`, footer) together; add a color mode via `color.rs`.
