# xvisualizer

Terminal spectrum visualizer for system audio. Captures playback via a PulseAudio monitor source, analyzes it with an FFT pipeline, and renders smooth animated bars in the terminal.

## Contents

- [Features](#features)
- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
- [User interface](#user-interface)
- [Capture sources](#capture-sources)
- [Signal processing](#signal-processing)
- [Rendering](#rendering)
- [Project structure](#project-structure)
- [Customization](#customization)
- [Development](#development)
- [Dependencies](#dependencies)
- [Limitations](#limitations)

## Features

- System audio capture through the PulseAudio monitor of the default sink (works on PipeWire via its Pulse compatibility layer).
- Optional solo capture of a single application or window, rerouted through a private null sink while staying audible.
- 2048-point Hann-windowed FFT mapped onto a logarithmic band grid (30 Hz – 16 kHz).
- Inertial bar motion: fast attack (~12 ms), soft release (~160 ms), falling peak markers (~0.8 s).
- Fixed 120 Hz render loop; one buffered write plus flush per frame, no full-screen clear, no flicker.
- Two bar themes: `Classic` (1-column bars with gaps) and `Solid` (2-column joined bars).
- Three bar color modes: `Height` (green-yellow-red by level), `Frequency` (color by band), `Mono` (uniform white).
- Two spectrum layouts: `Symmetric` (bass in the center, mirrored) and `Left-Right` (bass on the left).
- Manual sensitivity control (0.2x–8.0x, reset to 1.0x).
- Startup setup wizard: theme → bar color → layout → capture source.
- In-program menus for theme, color, layout, and capture source; help overlay; pause.
- Bar count adapts to terminal width (up to 64 bars).
- Alternate screen with full terminal state restore on exit, including on panic.
- Honors the [`NO_COLOR`](https://no-color.org/) environment variable.

## Requirements

- Linux with PulseAudio or PipeWire, and `pactl` available (used for monitor auto-detection, stream listing, and solo rerouting).
- Rust toolchain that supports edition 2024.
- Terminal with ANSI escape sequence and UTF-8 support. The following glyphs must render: `█ ▁▂▃▄▅▆▇ ─`.

## Installation

Clone the repository and run the release build:

```bash
git clone https://github.com/xTerion-code/xvisualizer-rust.git
cd xvisualizer-rust
cargo run --release
```

Quit with `Q` or `Ctrl+C`.

## Usage

```text
xvisualizer [OPTIONS]

Options:
  -d, --device NAME   Pulse source (default: auto-detected monitor of the default sink)
  -t, --theme NAME    start theme: classic | solid (default: classic)
      --no-color      disable ANSI colors
  -h, --help          show help
```

Notes:

- `--device` and `--theme` accept both `--flag value` and `--flag=value` forms.
- `--theme` also accepts the aliases `1` (classic) and `2` (solid).
- Setting the `NO_COLOR` environment variable has the same effect as `--no-color`.
- Unknown flags and unknown theme names terminate the program with exit code 2 and an error message.

Examples:

```bash
cargo run --release -- --device alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
cargo run --release -- --theme solid
cargo run --release -- --no-color
```

## User interface

### Startup wizard

The application starts in a four-step setup wizard:

1. Theme (`Setup 1/4`)
2. Bar color (`Setup 2/4`)
3. Layout (`Setup 3/4`)
4. Capture source (`Setup 4/4`)

`Enter` applies the current step and advances to the next one. `Esc` skips the remaining setup and goes straight to the visualizer. `Q` quits from any step.

### Visualizer screen

The main screen shows the spectrum bars with falling peak markers and a two-line footer. The first footer line reports the capture source, theme, layout, sensitivity, and color mode; it is temporarily replaced by a 4-second notice whenever the source or sensitivity changes (for example, `Capturing system mix` or `Sensitivity 1.25x`). The second footer line lists the available keys.

### Key reference

Visualizer keys:

| Key | Action |
| --- | ------ |
| `T` / `Tab` / `M` / `F2`, or any arrow key | Open the theme menu |
| `S` | Open the capture source menu |
| `Space` / `P` | Pause / resume (freezes bars and peaks) |
| `+` / `=` | Sensitivity up (multiplied by 1.25, clamped to 8.0x) |
| `-` / `_` | Sensitivity down (divided by 1.25, clamped to 0.2x) |
| `G` | Reset sensitivity to 1.0x |
| `L` | Toggle layout (`Symmetric` / `Left-Right`) |
| `C` | Cycle bar color (`Height` / `Frequency` / `Mono`) |
| `H` / `?` / `F1` | Toggle the help overlay |
| `1` / `2` | Quick theme switch (`Classic` / `Solid`) |
| `Q` or `Ctrl+C` | Quit |

Menu keys (theme, color, layout, source menus):

| Key | Action |
| --- | ------ |
| `Up` / `Down` / `Left` / `Right` | Move the selection |
| `Enter` / `Space` | Apply the selection |
| `Esc` | Back to the visualizer without applying (cancels the remaining setup when in the wizard) |
| `1`–`3` | Select an option directly (range depends on the menu) |
| `R` (source menu only) | Refresh the application list |

The help overlay closes with `Esc`, `Enter`, `Space`, `H`, `?`, or `F1`. Visualizer keys have no effect while it is open.

## Capture sources

Two capture modes are available, selectable at startup (wizard step 4/4) or later with `S`:

- **System mix (row `System`).** Records the monitor source of the default sink: everything audible on the system. The monitor is auto-detected with `pactl`: the monitor matching `<default-sink>.monitor` is preferred, otherwise the first available monitor is used. `--device` overrides the detection.
- **Single application.** The source menu lists currently playing sink inputs by application and stream name. Selecting one moves that stream to a private null sink (`xviz_cap_<pid>_<time>_<seq>`) whose monitor is recorded. A `module-loopback` (50 ms latency) keeps the application audible through its original sink.

Behavioral notes:

- If the loopback module cannot be loaded, capture continues but the application is muted; the footer notice reports `muted: no loopback`.
- If solo capture setup fails (for example, the stream ended), the application falls back to the system mix and reports the cause in the footer notice.
- Returning to `System`, or exiting, drops the solo session: the stream is moved back to its original sink and the helper modules are unloaded. Cleanup also runs on panic.
- PulseAudio errors during capture are fatal by design: the capture thread stops the whole application instead of spinning silently.

## Signal processing

Pipeline per frame:

```text
Pulse monitor -> capture thread -> short sample queue -> newest 2048-sample window
  -> Hann window -> forward FFT -> peak magnitude per log band -> manual gain
  -> gamma mapping -> inertial smoothing + peak tracking -> frame -> stdout
```

Parameters:

| Stage | Value |
| ----- | ----- |
| Sample rate / format | 48 kHz, stereo S16LE averaged to mono, normalized to [-1, 1] |
| Capture reads | 512 stereo frames (~10.7 ms) with low-latency `BufferAttr` (`fragsize` ~10 ms) |
| Sample queue | Newest 8192 mono samples (~170 ms); each frame analyzes the newest 2048 (~43 ms) |
| FFT | 2048-point forward FFT (`rustfft`), Hann window |
| Band grid | Logarithmic, 30 Hz – 16 kHz, peak magnitude per band |
| Reference / gain | Fixed reference magnitude `0.03`, manual gain 0.2x–8.0x, gamma `0.6` so quiet bands stay visible |
| Bar smoothing | Exponential easing: attack tau ~12 ms (1–2 frames), release tau ~160 ms |
| Peak markers | Snap to the target instantly, fall linearly at 1.2 units/s (~0.8 s full scale) |
| Bar count | Up to 64; fitted to terminal width (4-column margin; Classic cell 1+1, Solid cell 2+0) |
| Symmetric layout | `n` unique bands mirrored to `2n - 1` display bars, bass in the center |

Typical end-to-end latency is 60–100 ms: ~10 ms fragment + ~21 ms FFT window center + ~8 ms frame + ~20–50 ms Pulse server latency.

## Rendering

- 120 Hz frame loop (`dt` clamped to 1–50 ms so smoothing stays stable under lag or resize spikes).
- One buffered `write` plus `flush` per frame; cursor homed once (`ESC[H`); no full-screen clear.
- Bar bodies use full blocks (`█`); fractional bar tops use eighth blocks (`▁▂▃▄▅▆▇`); peak markers use `─` (cyan when colors are enabled).
- Colors: `Height` mode colors rows green (low), yellow (mid), red (top); `Frequency` mode colors bands red → yellow → green → cyan → magenta from bass to treble; `Mono` is uniform white. Peak markers keep their own color in all modes.

Example frame:

```text
                  █                                                       █
        █         █         █         █         █         █         █       █
        █   █     █   █     █   █     █   █     █   █     █   █     █   █   █
    ─   █   █ ─   █   █ ─   █   █ ─   █   █ ─   █   █ ─   █   █ ─   █   █   █
    █   █   █ █   █   █ █   █   █ █   █   █ █   █   █ █   █   █ █   █   █   █
    █   █   █ █   █   █ █   █   █ █   █   █ █   █   █ █   █   █ █   █   █   █
    █   █   █ █   █   █ █   █   █ █   █   █ █   █   █ █   █   █ █   █   █   █

            alsa_output.pci-0000_00_1f.3.analog-stereo.monitor  |  Ctrl+C
```

The exact appearance depends on the terminal font and width; the bar count scales with the window.

## Project structure

```text
src/
  main.rs         entry point and frame-loop orchestration
  args.rs         CLI parsing (device, theme, color flags) and --help text
  audio.rs        PulseAudio monitor capture into a sample queue; monitor auto-detection
  dsp.rs          Hann window, FFT, log bands, gain/gamma mapping, smoothing, peaks
  gain.rs         manual sensitivity multiplier (+/-, G reset; 0.2x-8.0x)
  color.rs        bar color modes (Height / Frequency / Mono) and ANSI codes
  theme.rs        bar-style themes (Classic / Solid: names, geometry, width fitting)
  symmetry.rs     spectrum layouts (Symmetric / Left-Right) and mirror mapping
  visualizer.rs   spectrum frame building and footer status
  ui.rs           screen-state composition (menus, pause, help, setup) and app events
  input.rs        keyboard transport, global visualizer keys, menu dispatch, wizard chaining
  theme_menu.rs   theme menu state, keys, and screen
  source_menu.rs  capture-source menu state, keys, and screen
  color_menu.rs   bar-color menu state, keys, and screen
  layout_menu.rs  layout menu state, keys, and screen
  choice_menu.rs  shared option-list screen for the color/layout menus
  help.rs         help overlay keys and screen
  frame.rs        shared frame-buffer text helpers (truncation, centering)
  source.rs       capture source selection: playback stream listing via pactl
  solo.rs         solo null-sink session and stream rerouting via loopback
  terminal.rs     alternate screen / raw mode setup and restore (RAII)
```

Design rules: one file per feature, wired through `main.rs`. Feature state lives in the feature's own file; `main` owns the instances and passes plain values down. Other modules use only the owning file's public API.

## Customization

| Goal | Location |
| ---- | -------- |
| Different source or latency | `audio.rs`: `CaptureSession::switch()`, `BufferAttr`, `READ_FRAMES` |
| Different solo capture behavior | `solo.rs`: `SoloSession`, null sink and loopback management |
| Different spectrum | `dsp.rs`: `WINDOW`, Hann table, `F_MIN` / `F_MAX`, `GAMMA`, reference magnitude |
| Different motion | `dsp.rs`: attack/release time constants, peak fall rate |
| Different look | `theme.rs` (`dims`, `fit_bar_count`), `color.rs` (`ColorMode`, `bar_color`), `symmetry.rs` (`LayoutMode`), `visualizer.rs` (`PARTS`, footer) |
| Different keys | `input.rs` (`handle_visualizer_key`, menu dispatch), per-menu `handle_key` functions, state in `ui.rs` |

Adding a theme requires extending `theme.rs`, `theme_menu.rs`, and `visualizer.rs` together. Adding a color mode requires extending `color.rs` and `color_menu.rs` together.

## Development

```bash
cargo build
cargo clippy --all-targets
cargo test
cargo fmt --check
```

Standards: `cargo clippy --all-targets` must report zero warnings. `cargo test` covers the `pactl` output parsers in `src/source.rs` and the pure helpers (`frame`, `gain`, `symmetry`, `theme`); these run without audio hardware. Use `cargo run --release` for representative frame timing. Audio capture, solo rerouting, and rendering require a live Linux audio stack and cannot be verified headlessly.

## Dependencies

- `crossterm 0.29`: alternate screen, cursor control, terminal size, keyboard events.
- `ctrlc 3`: graceful `Ctrl+C` shutdown.
- `rustfft 6`: forward FFT for spectrum analysis.
- `libpulse-binding 2` and `libpulse-simple-binding 2`: blocking monitor capture.

## Limitations

- Linux only. Requires PulseAudio or PipeWire with a working Pulse layer and `pactl`.
- Without `pactl` or without any monitor source, monitor auto-detection yields nothing and capture falls back to the server default source.
- The application list in the source menu reflects currently playing streams; silent or idle applications do not appear (use `R` to refresh).
- Audio device failures terminate the application by design; there is no automatic reconnection.
- Minimum usable terminal size is small (bars shrink to a single bar), but very narrow or short windows degrade readability.
