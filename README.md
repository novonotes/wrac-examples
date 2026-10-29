# wrac-examples

This repository is no longer maintained or updated for WRAC API changes.
For a current implementation example, see [WRAC Gain](https://github.com/novonotes/wrac-plugin-template/tree/main/plugins/wrac-gain) in [WRAC Plugin Template](https://github.com/novonotes/wrac-plugin-template).

> 日本語版: [README_JA.md](README_JA.md)

## Structure

```text
examples/
  gain-basic/   Basic audio-effect gain plugin.
  sine-synth/   MIDI note input to sine-wave instrument output.
  analyzer-3d/  3D rich Analyzer example.
  echo-lama/    Monophonic vowel-morphing synth with delay and a custom GUI.
```

## Setup

```sh
git submodule update --init --recursive
```

## Build

```sh
# Build every example as CLAP.
cargo xtask build --all --target=clap

# Build one example.
cargo xtask build --plugin=sine-synth --target=clap

# Build release version.
cargo xtask build --plugin=sine-synth --target=clap --release

# VST3 / AU / Standalone use the wrapper builder from the template submodule.
cargo xtask build --plugin=gain-basic --target=vst3,au,standalone
```

The value passed to `--plugin` is the example directory name under `examples/`.
For example, `examples/sine-synth` is selected with `--plugin=sine-synth`.

## Install Plugin

`install` copies previously built plugin artifacts into the directories that DAWs scan.
Run `build` first; `install` does not rebuild.

```sh
# Install one example into the user-local plugin folders.
cargo xtask install --plugin=gain-basic

# Install every example.
cargo xtask install --all

# Install release artifacts.
cargo xtask install --plugin=gain-basic --release

# Install only specific formats.
cargo xtask install --plugin=gain-basic --target=clap,vst3

# Install system-wide instead of user-local.
cargo xtask install --plugin=gain-basic --scope=system
```

The default target set follows the current OS (macOS: clap, vst3, au; Windows/Linux: clap, vst3).
`standalone` is not a plugin format and cannot be installed with this command — use `launch` instead.

Installation directories per scope:

| OS      | Scope  | CLAP                                 | VST3                                 | AU                                          |
| ------- | ------ | ------------------------------------ | ------------------------------------ | ------------------------------------------- |
| macOS   | user   | `~/Library/Audio/Plug-Ins/CLAP`      | `~/Library/Audio/Plug-Ins/VST3`      | `~/Library/Audio/Plug-Ins/Components`       |
| macOS   | system | `/Library/Audio/Plug-Ins/CLAP`       | `/Library/Audio/Plug-Ins/VST3`       | `/Library/Audio/Plug-Ins/Components`        |
| Windows | user   | `%LOCALAPPDATA%\Programs\Common\CLAP` | `%LOCALAPPDATA%\Programs\Common\VST3` | —                                           |
| Windows | system | `%CommonProgramFiles%\CLAP`           | `%CommonProgramFiles%\VST3`           | —                                           |
| Linux   | user   | `~/.clap`                            | `~/.vst3`                            | —                                           |
| Linux   | system | `/usr/lib/clap`                      | `/usr/lib/vst3`                      | —                                           |

To remove installed artifacts, use `cargo xtask uninstall`. It accepts the same `--plugin` / `--all` / `--target` flags, plus `--scope=all|user|system` (defaults to `all`) and `--dry-run` to preview the paths without deleting them.

```sh
cargo xtask uninstall --plugin=gain-basic
cargo xtask uninstall --plugin=gain-basic --dry-run
```

## Launch Standalone

```sh
cargo xtask build --plugin=gain-basic --target=standalone --release
cargo xtask launch --plugin=gain-basic --release
```

To launch the debug build instead, drop the `--release` flag from both commands. Additionally, the debug build loads its GUI from the Vite dev server, so follow the [About the Debug Build GUI](#about-the-debug-build-gui) section below to start the dev server before launching.

## About the Debug Build GUI

Debug builds load the GUI from the Vite dev server instead of the embedded bundle.
The dev server must be running before the plugin window is opened in a DAW or in the standalone — otherwise the WebView shows a blank screen.

Release builds embed the built `src-gui/dist` directly into the plugin binary, so the dev server is not needed.

### Start the dev server

Open a terminal for the example you want to debug and start Vite. Keep it running for the entire debug session. For instance if you want to debug the gain-basic plugin, run commands like this:

```sh
cd examples/gain-basic/src-gui
npm install   # first time only
npm run dev
```

Then, in another terminal, build the plugin in debug mode and install or launch it.

```sh
cargo xtask build --plugin=gain-basic
cargo xtask install --plugin=gain-basic # install the plugin
# or
cargo xtask launch --plugin=gain-basic # launch the standalone plugin
```

Editing files under `src-gui/` triggers Vite's hot reload, so frontend changes do not require a Rust rebuild.

### Changing the port

The dev server URL is fixed to `http://127.0.0.1:5173/` on both the Vite side and the Rust side, so changing the port requires editing both.

1. `examples/<plugin>/src-gui/vite.config.ts` — update `server.port`. `strictPort: true` is set, so Vite will not silently fall back to another port if the configured one is taken.
2. `examples/<plugin>/src-plugin/src/gui/runtime.rs` — update the `url` literal inside the `#[cfg(debug_assertions)]` branch to match.
3. Rebuild the plugin (`cargo xtask build --plugin=<plugin>`) and restart the dev server.

If port 5173 is already in use, the simplest fix is usually to stop the other process rather than reconfiguring both sides.

## Main Repository

The main project is [wrac-plugin-template](https://github.com/novonotes/wrac-plugin-template). It contains the template, setup guide, build workflow, and support channels.

This repository contains historical examples built from that template.
Please use the [wrac-plugin-template issues](https://github.com/novonotes/wrac-plugin-template/issues) or [discussions](https://github.com/novonotes/wrac-plugin-template/discussions) for questions, feedback, and compatibility reports.
