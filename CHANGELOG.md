# Changelog

All notable changes to **Phantom DSP Suite** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Phase B — Pro-Q 4+ Flagship UI, Intelligent DSP & State Engine (`phantom-dsp`, `phantom-ui`, `phantom-eq`)**:
  - **Pro-Q 4 Floating Band Popover HUD & Precision Rotary Knobs**: On-canvas popover anchored to the selected node with 4 rotary knobs (`FREQ`, `GAIN`, `Q`, `DYN`), filter shape vector icons, slope/routing pills, Spectral Dynamics (`SPEC`) toggle, Headphone Solo latch/momentary audition, and `[HUD / DOCK / HUD+DOCK]` layout switch.
  - **Resizable Plugin Window**: `ResizableWindow` wrapper (`860×500` minimum, `1100×640` default) with responsive EQ canvas and output meter scaling.
  - **Interactive Piano Roll & Musical Note Snapping**: 20px `C1..C8` Piano Roll bar with active band note dots, live note + cents readout, click-key-to-snap frequency, and `[SNAP]` / `Ctrl`-drag semitone snapping.
  - **Spectrum Grab, EQ Match & Multi-Instance Collision Detection**: Automatic resonant peak detection (`detect_spectrum_peaks`) with click-and-drag Bell creation, `EqMatchEngine` reference vs. current track 1/3-octave curve fitting (up to 12 bands), and 16-slot lock-free `CollisionRegistry` cross-track masking overlay.
  - **Per-Band Spectral Dynamics (`SpectralBandProcessor`)**: Zero-latency 9-subband selective resonance compressor/expander within each band's target bandwidth.
  - **Preset Manager, A/B State Bank & 64-Step Undo/Redo**: 12 built-in Mastering/Mixing/Creative Factory Presets + offline `.peq` User Presets (`~/.config/phantom-dsp/presets/phantom-eq/`), `<` / `>` preset navigation, `[A / B]` comparison slots with `[Copy]`, and 64-step Undo/Redo (`Ctrl+Z` / `Ctrl+Y`).

### Planned
- **Phantom Comp** — Mastering & bus compressor with visual transfer curve and sidechain filtering.
- **Phantom Limiter** — True Peak brickwall mastering limiter with EBU R128 loudness metering.
- **Phantom MB** — Linear-phase multiband dynamic processor with per-band soloing.
- **Phantom Saturn** — Multiband tape and tube harmonic saturation engine.

## [0.1.0] - 2026-11-19

### Added
- **Workspace Architecture (`phantom-dsp-suite`)**:
  - Modular Rust Cargo workspace separating pure DSP (`crates/phantom-dsp`), shared UI (`crates/phantom-ui`), plugins (`plugins/phantom-eq`), and desktop tooling (`apps/phantom-launcher`).
- **Pure DSP Engine (`phantom-dsp`)**:
  - Transposed Direct Form II RBJ biquad filters (`Bell`, `LowCut`, `HighCut`, `LowShelf`, `HighShelf`, `Notch`, `BandPass`).
  - Zero-padded Overlap-Save **Linear Phase FIR Engine** (`FFT_SIZE = 8192`, `KERNEL_LEN = 4095` odd Type-I symmetric Kaiser β=7.5 kernel, `HOP_SIZE = 1024`, exact buffer-size-invariant `LATENCY_SAMPLES = 3071`), strict real DC/Nyquist enforcement, exact 2×2 Stereo/Left/Right/Mid/Side transfer matrix, C1 raised-cosine click-free kernel crossfading, and zero-allocation scratch buffers verified via `#[global_allocator]`.
  - Per-band dynamic sidechain envelope detection (`DynamicDetector`).
  - 2x anti-cramping oversampling (`Oversampler2x`).
  - Mid/Side encoding/decoding and per-band channel routing (`ChannelTarget`).
  - Lock-free real-time FFT spectrum analyzer bridge (`SharedSpectrum`, `SpectrumProcessor`).
  - 9 automated DSP tests covering impulse symmetry, flat 0 dB identity across varied chunk sizes, frequency-response accuracy, zero NaN/Inf, zero-allocation guarantee, and throughput (>100M smp/s biquad, >43M smp/s linear phase).
- **Dark Obsidian UI Toolkit (`phantom-ui`)**:
  - Reusable dark-obsidian palette (`#101117` / `#161820` / `#38BDF8`), typography, Lucide-inspired vector icons, and gradient-mesh frequency-response/spectrum rendering views.
- **Phantom EQ Plugin (`phantom-eq` — VST3 & CLAP)**:
  - 8-band dynamic parametric equalizer with Zero Latency (`0` samples) and Linear Phase (`3071` samples) modes and dynamic NIH-plug host latency reporting (`set_latency_samples`).
  - Per-band Solo (Audition) monitoring (lock-free `AtomicU8` + dedicated bandpass/cut monitor respecting Stereo/L/R/M/S placement) and isolated selected-band curve overlay.
  - Pro-Q-style canvas ergonomics (double-click node gain reset, double-click empty canvas band activation, right-click bypass toggle, scroll-wheel Q control).
  - Explicit Windows DLL export definition (`exports.def`) exporting `GetPluginFactory` and `clap_entry`.
- **Phantom Launcher (`phantom-launcher`)**:
  - Cross-platform offline-first Desktop GUI (`eframe`/`egui 0.31`) and headless CLI (`--cli`, `--list`, `--install`, `--uninstall`, `--verify`, `--user`/`--system`/`--vst3-dir`/`--clap-dir`/`--source`).
  - 1-click installation to User, System, or Custom VST3 & CLAP directories on Windows, macOS, and Linux, plus non-blocking Windows Cubase process detection.
  - Pure-Rust streaming 64 KiB SHA-256 verification (`sha2`) with post-copy verification, editable manual GitHub Releases link (`intity01/phantom-dsp-suite`), zero automatic startup network requests, and 12 automated unit/roundtrip tests.
- **Modularity & Hygiene**:
  - All 45 `.rs` files across the workspace strictly ≤ 300 lines; 21/21 automated tests passing with 0 workspace compiler warnings.
- **Windows Cubase Installers**:
  - `install_to_cubase.ps1` and `install_to_cubase.bat` helper scripts.
- **Open-Source Governance & CI/CD**:
  - Dual licensing (`MIT OR Apache-2.0`), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, GitHub issue/PR templates, automated multi-OS CI (`ci.yml`), and tagged multi-platform release automation with `SHA256SUMS.txt` (`release.yml`).

[Unreleased]: https://github.com/intity01/phantom-dsp-suite/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/intity01/phantom-dsp-suite/releases/tag/v0.1.0
