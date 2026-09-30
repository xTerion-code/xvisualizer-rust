# xvisualizer-rust

Terminal ASCII visualizer for system audio: low-latency PulseAudio monitor capture, FFT spectrum, smooth bar rendering on crossterm.

## Features

- System audio capture via the PulseAudio monitor source (works on PipeWire via its Pulse layer)
- Auto-detects the monitor of the default sink, override with `--device`
- Stereo capture averaged down to mono, 48 kHz / S16
- 2048-point FFT with Hann window, logarithmic band grid 30 Hz – 16 kHz
- Fixed ~120 Hz render loop, inertial bar smoothing (fast attack, soft release)
- Sub-cell bar tops (`▁▂▃▄▅▆▇█`) plus falling peak markers (`─`)
- Row colors by height: green → yellow → red, `--no-color` to disable
- Two bar themes with an in-program menu (arrows + Enter):
  `Classic` (thin bars with gaps) and `Solid` (wide bars without gaps).
  Press `T`/`Tab` or any arrow to open the menu, `1`/`2` for a quick switch
- Bar count adapts to terminal width (8–64), flicker-free single-write frames
- Alternate screen: the terminal is restored on exit
- `Ctrl+C` exit with cursor and screen restored

## Requirements

- Rust 1.98+ / Cargo (edition 2024)
- Linux with PulseAudio or PipeWire + `pactl` (for source auto-detection)
- Unix terminal with ANSI and UTF-8 support (glyphs `█ ▁▂▃▄▅▆▇ ─`)

## Install and run

```bash
git clone git@github.com:xTerion-code/xvisualizer-rust.git
cd xvisualizer-rust
cargo run --release
```

Exit: `Ctrl+C` or `Q`.

## Options

```text
-d, --device NAME   Pulse source (default: auto-detected monitor of the default sink)
-t, --theme NAME    start theme: classic | solid (default: classic)
    --no-color      disable ANSI colors
-h, --help          show help
```

Keys (in program):

```text
Up/Down or Left/Right  choose theme in the menu
Enter                  apply theme
T / Tab                open theme menu, Esc - back
1 / 2                  quick theme switch
Q or Ctrl+C            quit
```

Examples:

```bash
cargo run --release -- --device alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
cargo run --release -- --no-color
cargo run --release -- --theme solid
```

## Example

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

Rendering depends on the terminal font and width; the bar count scales with the window.

## How it works

```text
Pulse monitor → capture thread → short sample queue → Hann + FFT (per frame)
  → log bands → auto-gain → inertial smoothing → fixed-height frame → stdout
```

- Capture: blocking `pa_simple` reads of 512 stereo frames (~10.7 ms), explicit low-latency `BufferAttr` (`fragsize` ~10 ms, buffer ~43 ms) instead of the ~2 s server default.
- Queue: only the newest ~170 ms of mono samples are kept; each frame analyzes the newest 2048-sample window (~43 ms).
- Spectrum: Hann-windowed forward FFT, peak magnitude per log band, gamma `0.6` so quiet bands stay visible, adaptive gain (fast attack, ~0.4 s release).
- Motion: per-bar exponential easing — attack tau ~12 ms (1–2 frames), release tau ~160 ms — peak markers fall linearly in ~0.8 s.
- Render: one buffered `write` + `flush` per frame at ~120 Hz, cursor homed once (`ESC[H`), no full-screen clear, so no flicker.

End-to-end latency is roughly: ~10 ms (fragment) + ~21 ms (FFT window center) + ~8 ms (frame) + Pulse server latency (~20–50 ms) ≈ 60–100 ms.

## Project structure

```text
src/
  main.rs      — entry point and frame-loop orchestration
  args.rs      — CLI parsing (device, theme, color flags)
  audio.rs     — PulseAudio monitor capture into a sample queue
  dsp.rs       — Hann window, FFT, log bands, adaptive gain, smoothing
  theme.rs     — bar-style themes (geometry and names)
  input.rs     — keyboard: theme menu navigation (arrows + Enter), hotkeys
  render.rs    — frame building: theme menu and spectrum visualizer
  terminal.rs  — alternate screen / raw mode setup and restore
```

Extension points:

- different source / latency — `audio.rs`: `capture_loop()`, `BufferAttr`, `READ_FRAMES`
- different spectrum — `dsp.rs`: `WINDOW`, Hann table, `F_MIN`/`F_MAX`, `GAMMA`
- different motion — `dsp.rs`: attack/release taus, gain release, peak fall
- different look — `theme.rs` (`dims`, new themes) and `render.rs` (`PARTS`, color thresholds, footer)
- different keys — `input.rs`: `handle_menu_key()`, `handle_visualizer_key()`

## Development

```bash
cargo build
cargo clippy --all-targets
```

Verified: `cargo build` and `cargo clippy --all-targets` with no warnings.

## Dependencies

- `crossterm 0.29` — alternate screen, cursor, terminal size
- `ctrlc 3` — graceful `Ctrl+C` exit
- `rustfft 6` — spectrum analysis
- `libpulse-binding 2` + `libpulse-simple-binding 2` — monitor capture
