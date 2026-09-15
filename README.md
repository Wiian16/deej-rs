# deej-rs

> [!WARNING]
> **This project is a work in progress and is not ready for general use.**

A Rust implementation of [deej](https://github.com/omriharel/deej), focused on Linux.

Deej lets you control the volume of individual applications with physical sliders. An Arduino (or similar) reads a set
of potentiometers and streams their positions over serial. Deej reads that stream and maps each slider to something on
your system: the master output, your mic, a specific application, or a group of them.

## Credit

All credit for the original idea, the hardware design, the firmware, and the configuration format goes to
**[omriharel](https://github.com/omriharel)** and the contributors to
[**omriharel/deej**](https://github.com/omriharel/deej). That project is the reference implementation, it supports both
Windows and Linux, and is probably what you want if you're looking for something that works today.

Go there first for:

- [Hardware build instructions and wiring diagrams](https://github.com/omriharel/deej#build-your-own)
- [The Arduino firmware](https://github.com/omriharel/deej/blob/master/arduino/deej-5-sliders-vanilla/deej-5-sliders-vanilla.ino)
- [Releases and installation](https://github.com/omriharel/deej/releases)
- [The community Discord](https://discord.gg/nf88NJu)

Deej-rs uses the same serial protocol and the same `config.yaml` schema, so hardware built for the original works here
unmodified.

## Why

The original deej is written in Go and targets Windows first. It's Linux support works, but isn't the primary focus.
Deej-rs exists to be a Linux-native implementation: it talks to PulseAudio/PipeWire directly through `libpulse`
rather than shelling out, and it's structured so other audio backends can be added behind a trait later.

## Status

### Working

- **Serial input**: reads `401|410|517|561|612`-style frames over a serial port, with automatic reconnection of the
  device dissapears or is unplugged.
- **PulseAudio backend**: sets volumes over the native PulseAudio protocol via `libpulse-binding`, so PipeWire's
  PulseAudio compatibility layer is also supported.
- **Slider targets**: `master`, `mic`, individual processes, groups of processes, and `deej.unmapped` (everything not
  bound to another slider).
- **Noise reduction**: moving-average smoothing with a deadband, configurable as `low`, `default`, or `high`, matching
  the original's behavior
- **Inverted sliders**: via `invert_sliders`.
- **Live config reload**: edits to `config.yaml` are picked up without restarting the process.
- **Graceful shutdown**: handles `SIGINT` and `SIGTERM` cleanly.

### New

- **Config flag**: provide a config file with `--config` or `-c`, rather than requiring `config.yaml` to live next to
  the binary.
- **Verbose logging**: meaningful logs are output for all errors and warnings by default, info, debug, and trace logs
  can be enabled with the `--verbose` or `-v` flag.

### Roadmap

- **PulseAudio subscriptions**: set an app's volume when its stream is first detected, so an app launched after you
  moved the slider still picks up the right level.
- **Notifications**: alert the user about critical issues (lost serial device, dead PulseAudio connection) instead of
  only logging issues.
- **Packages for Linux Distributions.**
- **Systemd service.**

### Not planned right now

Windows support. `tokio-serial`'s async reads don't behave well on Windows COM ports. Working around it needs a blocking
reader on a dedicated thread. The code is structured to support it, but Linux support is prioritized.

## Requirements

- A recent stable Rust toolchain (the workspace used edition 2024).
- `libpulse` development headers.
  - Debian/Ubuntu: `sudo apt install libpulse-dev`.
  - Fedora: `sudo dnf install pulseaudio-libs-devel`.
  - Arch: `libpulse` is part of `pulseaudio`/`pipewire-pulse`.
- PulseAudio or PipeWire with `pipewire-pulse`.
- deej-compatible hardware running the original firmware, or one of the fake serial scripts below.

## Building

```sh
git clone https://github.com/wiian16/deej-rs.git
cd deej-rs
cargo build --release
```

The binary lands at `target/release/deej-rs`.

## Running

```sh
deej-rs                                      # looks for config.yaml next to the executable
deej-rs --config ~/.config/deej/config.yaml  # Provide  a config file from somewhere else on your disk
deej-rs  -- verbose                          # trace-level logs, useful for debugging serial
```

Without `--verbose`, logging is quiet: warnings and errors only.

You'll likely need permission to read the serial device. On most distributions that means adding yourself to the
`dialout` (or `uucp`) group and logging back in.

## Configuration

Configuration uses the same `config.yaml` format as the original deej. A copy lives in the repo root. Note that the
file is still the upstream Windows-flavored example, so at minimum you'll want to change `com_port` to a Linux device
path.

```yaml
# process names are case-insensitive
# use 'master' for the master channel, or a list of process names to create a group
# use 'mic' to control your mic input level
# use 'deej.unmapped' to control every app not bound to another slider
# slider indexes start at 0, regardless of which analog pins you're using
slider_mapping:
  0: master
  1: firefox
  2: spotify
  3:
    - steam
    - rocketleague
  4: discord

# set to true if you want the controls inverted (top is 0%, bottom is 100%)
invert_sliders: false

# settings for connecting to the board
com_port: /dev/ttyUSB0
baud_rate: 9600

# "low" (excellent hardware), "default" (regular hardware) or "high" (noisy hardware)
noise_reduction: default
```

## Development

### Testing without hardware

Two scripts under `scripts/` emulate a deej board on a pseudo-terminal, so you can work on this without an Arduino
plugged in:

```sh
python3 scripts/fake_serial_sweek.py  # sweeps all sliders 0% -> 100% -> 0%, on a loop
python3 scripts/fake_serial_hold.py   # holds all sliders at a fixed value
```

Each prints the tty path it created, put that in `com_port` and start deej-rs.

### Pre-commit hooks

Install the [pre-commit](https://pre-commit.com/) package manager, then run `pre-commit install`. The hooks
(`cargo fmt`, `cargo check`, `cargo clippy`) will run before every commit.

### Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs all three, plus `cargo audit` on every PR and weekly on a schedule.

The workspace denies `clippy::pedantic` and `clippy::nursery`, along with the usual paic-adjacent lints (`unwrap_used`,
`expect_used`, `index_slicing`, `arithmetic_side_effects`, `as_conversions`, etc.). Expect to justify any with `#[allow(clippy::<lint>)]`.

### Layout

| Crate                | What it does                                                                                                                                                       |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `deej-rs`            | The binary: CLI parsing, YAML config loading, file watching, logging, signal handling.                                                                             |
| `deej-lib`           | The service itself: serial reading, frame parsing, smoothing, the `AudioAdapter` trait and the PulseAudio implementation of it. Knows nothing about files or YAML. |
| `pulseaudio-wrapper` | A small async wrapper over `libpulse-binding`. Runs the PulseAudio mainloop on a dedicated thread and exposes an async, `Send`-safe handle over it.                |

The split between `deej-rs` and `deej-lib` is deliberate: `deej-lib` takes a fully-resolved
`ServiceConfig` with no notion of magic strings, which keeps the config format and the service
independent of each other. Audio backends sit behind the `AudioAdapter` trait, so adding one
(PipeWire natively, ALSA, whatever) shouldn't require touching the service loop.

## License

Deej-rs is split-licensed by crate:

| Crate | License |
| --- | --- |
| [`deej-rs`](./deej-rs) | [GPL-3.0-or-later](./LICENSE-GPL) |
| [`deej-lib`](./deej-lib) | [LGPL-3.0-or-later](./LICENSE-LGPL) |
| [`pulseaudio-wrapper`](./pulseaudio-wrapper) | [LGPL-3.0-or-later](./LICENSE-LGPL) |

Full license texts are at [`LICENSE-GPL`](./LICENSE-GPL) and [`LICENSE-LGPL`](./LICENSE-LGPL) in the
repo root.

### Third-party licenses

Deej-rs depends on third-party crates, all under permissive or GPL-compatible licenses. Full attribution and license
text for every dependency is generated with [`cargo about`](https://github.com/EmbarkStudios/cargo-about) and kept in
[`NOTICE.md`](./NOTICE.md).

### Upstream credit

Deej-rs implements the same serial protocol and configuration format as
[omriharel/deej](https://github.com/omriharel/deej), which is released under the
[MIT License](https://github.com/omriharel/deej/blob/master/LICENSE). No code has been copied from that project, but the
format and the
protocol are unchanged. Full credit for the original design goes to omriharel and deej's contributors.

See [Credit](#credit) above.