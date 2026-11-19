# Contributing to Phantom DSP Suite

Thank you for your interest in contributing to **Phantom DSP Suite**! We are building a modern, open-source, mastering-grade audio plugin suite in Rust — with zero telemetry, zero DRM, and uncompromising real-time DSP quality.

All contributors are expected to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Workspace Architecture & Module Boundaries

The repository is organized as a single Cargo workspace with strict separation of concerns:

```text
phantom-dsp-suite/
├── crates/
│   ├── phantom-dsp/        # Pure audio DSP math (filters, FIR, dynamics, FFT, M/S, oversampling)
│   └── phantom-ui/         # Reusable Dark Obsidian egui widgets, theme & Lucide vector icons
├── plugins/
│   └── phantom-eq/         # Phantom EQ VST3/CLAP plugin (NIH-plug adapter + editor wiring)
└── apps/
    └── phantom-launcher/   # Cross-platform offline plugin hub & 1-click installer (GUI + CLI)
```

### Boundary Rules

1. **`crates/phantom-dsp` must remain 100% UI-free and host-free.**
   - Pure signal processing only (no `nih_plug`, no `egui`, no GUI or OS dependencies).
   - Must be independently testable with `cargo test -p phantom-dsp --release`.
2. **`crates/phantom-ui` owns reusable visual components.**
   - Theme tokens, vector icons, spectrum/curve widgets, and controls shared across plugins.
   - Do not put plugin-specific parameter state here.
3. **`plugins/<name>` binds DSP + UI to VST3/CLAP via `nih_plug`.**
   - Keep `lib.rs`, `params.rs`, and `editor/` cleanly decomposed.
4. **`apps/phantom-launcher` owns plugin discovery, installation, and integrity verification.**
   - Must stay strictly **offline-first** (zero automatic network requests on startup) and use pure-Rust dependencies that cross-compile cleanly to Windows.

---

## Core Engineering Standards

Every pull request is reviewed against these non-negotiable standards:

### 1. Strict Modularity (<= 300 Lines per File)

- Every Rust source file under `crates/*/src/`, `plugins/*/src/`, and `apps/*/src/` **must stay at or below 300 lines**.
- If a module approaches 300 lines, split it into cohesive submodules (e.g., `editor/mod.rs`, `editor/header.rs`, `editor/display.rs`, `editor/inspector.rs`).
- CI automatically audits file line counts on every push and pull request.

### 2. Zero-Allocation Audio Thread

The audio `process()` callback runs on a real-time thread with strict deadlines:

- **NEVER** allocate on the heap inside audio processing loops (`Vec::new`, `Vec::push`, `.collect()`, `Box`, `String`, `format!`, `.clone()` on heap types).
- **NEVER** acquire blocking locks (`Mutex::lock`), perform file I/O, or invoke syscalls on the audio thread.
- Pre-allocate all FFT scratch buffers, delay lines, and ring buffers inside `new()` or `initialize()`.
- Use fixed-size stack arrays (e.g., `[BandSnapshot; NUM_BANDS]`) for parameter change detection.
- Guard against NaNs, Infs, and denormals in every DSP stage.
- **Cross-compilation note**: Do **not** add `assert_process_allocs` to `nih_plug` default features in `Cargo.toml` — it breaks `x86_64-pc-windows-gnu` cross-compilation.

### 3. Clean-Room DSP Policy

Phantom DSP Suite is a **100% clean-room** open-source implementation:

- All filters, oversamplers, linear-phase FIR engines, dynamic detectors, and UI metaphors must be written from first-principles audio DSP mathematics and public academic/industry literature (e.g., Robert Bristow-Johnson's Audio EQ Cookbook, Zölzer's *DAFX*, Smith's *Spectral Audio Signal Processing*, Oppenheim & Schafer).
- **NEVER** reverse engineer, disassemble, decompile, copy coefficients from, or inspect proprietary commercial plugin binaries.
- **NEVER** copy third-party copyrighted graphics, icons, presets, or source code from incompatible licenses.

### 4. Zero Telemetry & Offline-First

- No analytics, crash trackers, license check-ins, or hidden network traffic.
- Any network feature in `phantom-launcher` must be strictly opt-in and triggered explicitly by the user.

---

## Development Setup

### Prerequisites

- **Rust**: Stable toolchain (`edition = "2021"`) via [rustup](https://rustup.rs/)
- **Linux system libraries** (Ubuntu / Debian):
  ```bash
  sudo apt-get update
  sudo apt-get install -y libxcb1-dev libgl1-mesa-dev pkg-config \
    libx11-dev libxcursor-dev libxrandr-dev libxi-dev libxkbcommon-dev
  ```
- **Windows cross-compilation from Linux (optional)**:
  - `rustup target add x86_64-pc-windows-gnu`
  - [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) + `zig`

### Build, Format & Test Commands

```bash
# Run DSP accuracy, symmetry, zero-NaN & throughput tests
cargo test -p phantom-dsp --release -- --nocapture

# Run Launcher manifest, SHA-256 & install/uninstall tests
cargo test -p phantom-launcher --release -- --nocapture

# Type-check the full workspace
cargo check --workspace --all-targets

# Verify formatting (uses workspace rustfmt.toml)
cargo fmt --all -- --check
```

### Cross-Compiling Windows VST3/CLAP & Launcher from Linux

```bash
# Build Phantom EQ DLL with explicit VST3 + CLAP exports
RUSTFLAGS="-C link-arg=$(pwd)/plugins/phantom-eq/exports.def" \
  cargo zigbuild --target x86_64-pc-windows-gnu -p phantom-eq --release

# Build Phantom Launcher Windows executable
cargo zigbuild --target x86_64-pc-windows-gnu -p phantom-launcher --release
```

---

## Commit Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/):

| Prefix      | Purpose                                                          |
| ----------- | ---------------------------------------------------------------- |
| `feat:`     | New user-facing feature, DSP mode, or plugin                     |
| `fix:`      | Bug fix (DSP math, UI interaction, DAW compatibility, installer) |
| `perf:`     | Performance optimization (SIMD, FFT scratch reuse, CPU reduction)|
| `refactor:` | Code restructuring or file-splitting with no behavior change     |
| `docs:`     | Documentation, README, changelog, or comments                    |
| `test:`     | Adding or improving unit/integration/DSP accuracy tests          |
| `ci:`       | GitHub Actions workflows or build scripts                        |
| `chore:`    | Maintenance tasks, dependency updates, tooling config            |

Example: `feat(phantom-dsp): add zero-padded overlap-save linear phase FIR engine`

---

## Pull Request Process

1. Fork the repository and create a topic branch from `main` (e.g., `feat/band-solo-monitor`).
2. Keep PRs focused on a single concern and ensure all modified files remain `<= 300` lines.
3. Add or update unit tests for any DSP math, routing, or installer change.
4. Run `cargo test -p phantom-dsp --release`, `cargo test -p phantom-launcher --release`, `cargo check --workspace`, and `cargo fmt --all -- --check`.
5. Fill out the Pull Request template checklist honestly and completely.

## Licensing of Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Phantom DSP Suite by you, as defined in the Apache-2.0 license, shall be dual-licensed under **MIT OR Apache-2.0** (see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE)), without any additional terms or conditions.
