# Changelog

All notable changes to **Phantom DSP Suite** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Phantom Vox Windows packaging**: `package_windows.ps1` builds the Windows plugins and lays them out under `dist/windows/` (VST3 bundle + CLAP), so the launcher can install Phantom Vox. The Windows release archive now ships Phantom Vox too, and its DLL exports are verified alongside Phantom EQ.

### Planned
- **Phantom Comp** — Mastering & bus compressor with visual transfer curve and sidechain filtering.
- **Phantom Limiter** — True Peak brickwall mastering limiter with EBU R128 loudness metering.
- **Phantom MB** — Linear-phase multiband dynamic processor with per-band soloing.
- **Phantom Saturn** — Multiband tape and tube harmonic saturation engine.

## [0.1.0] - 2026-11-20

### Added
- **Workspace Architecture (`phantom-dsp-suite`)**:
  - Modular Rust Cargo workspace strictly enforcing `<= 300` lines per `.rs` file, separating pure DSP (`crates/phantom-dsp`), shared UI (`crates/phantom-ui`), plugins (`plugins/phantom-eq`), and desktop tooling (`apps/phantom-launcher`).
- **Mastering DSP Engine (`phantom-dsp` & `phantom-eq`)**:
  - **24 Dynamic & Spectral Bands** (clean Pro-Q 4 empty startup) with 10 filter shapes (`Bell`, `Low Cut`, `High Cut`, `Low Shelf`, `High Shelf`, `Notch`, `Band Pass`, `Tilt Shelf`, `Flat Tilt`, `All Pass`) and 8 cascaded slopes (`6 dB/oct` to `96 dB/oct` + `Brickwall`).
  - **Triple Processing Engine** defaulting to low-CPU **`Zero Latency + 1x (Off)`** oversampling:
    - **Zero Latency** (`0` samples) and **Natural Phase** (analog-matched Nyquist IIR) with optional `2x HQ` / `4x Mastering` polyphase half-band oversampling.
    - **Linear Phase FIR** (`FFT_SIZE = 8192`, `KERNEL_LEN = 4095` odd Type-I symmetric Kaiser β=7.5 kernel, `HOP_SIZE = 1024`, exact `LATENCY_SAMPLES = 3071` reported to the DAW via PDC, C1 raised-cosine kernel crossfading, 2×2 Stereo/L/R/M/S transfer matrix).
  - **Zero-Click Parameter Smoothing**: per-band `BandParamSmoother` (`SMOOTH_SUBBLOCK = 16`, log freq/Q + linear gain, 15 ms) and `OnePoleSmoother` for output gain and global Gain Scale — zero zipper noise under rapid mouse drags or DAW automation.
  - **Click-Free 10 ms PDC-Aligned Bypass**: `BypassCrossfade` (10 ms smooth wet/dry ramp) paired with `LatencyBypassDelay` (continuously updated `3071`-sample stereo delay line in Linear Phase mode) so toggling Bypass never clicks or shifts DAW PDC timing while keeping meters and spectrum live.
  - **Native Mono + Stereo Track Support**: declares both Stereo (`2->2` + 2-ch `Sidechain`) and Mono (`1->1` + 1-ch `Sidechain`) `AUDIO_IO_LAYOUTS`, processing mono tracks with fixed 256-sample stack buffers and zero heap allocations.
  - **Global Gain Scale (`0%–200%`)**: DAW-automatable parameter scaling static band gain, dynamic range, and visual EQ curves for gain-bearing shapes (`Bell`, `LowShelf`, `HighShelf`, `TiltShelf`, `FlatTilt`, `BandPass`) while leaving cuts/notch/all-pass unscaled.
  - **Dynamic EQ, Spectral Dynamics & Pro-Q Tools**: soft-knee dynamic EQ with Attack/Release/Auto-Threshold/External Sidechain, 9-subband zero-latency `SpectralBandProcessor`, 1/3-octave `EqMatchEngine`, interactive Spectrum Grab peak handles, 16-slot lock-free `CollisionRegistry`, Character saturation (`Clean` / `Subtle Analog` / `Warm Tube`), gated RMS Auto-Gain, EBU R128 Momentary/Short-Term LUFS + True Peak metering, and 10 ms crossfaded Headphone Solo audition.
- **Pro-Q 4 Minimal Dark Obsidian UI (`phantom-ui` & `phantom-eq/src/editor`)**:
  - **Single-Row Top Bar & Slim Bottom Strip**: uncluttered Dark Obsidian canvas (`#101117` / `#38BDF8`) with lock-free `LiveParamMirror` (0 ms GUI feedback even when host audio is suspended), fixed proportional window size presets (`S 960×560`, `M 1100×640`, `L 1260×720`, `XL 1420×800`), isolated right-hand LUFS/True Peak output meter, and `Mode / Engine` & `Analyzer & Tools` popovers.
  - **Floating Band HUD & Direct Keyboard Knob Entry**: anchored `296×98 px` floating HUD with 4 precision rotary knobs (`FREQ`, `GAIN`, `Q`, `DYN`); clicking any knob's value strip enables direct inline keyboard typing including 12-TET musical notes (`A4`, `C#3`, `Db4`, `2.5k`, etc.), while double-clicking the dial resets to default.
  - **Right-Click Band Node Context Menu & Tactile Gestures**: right-click any node for Bypass, Solo, all 10 shapes, all 8 slopes, all 5 placements, Invert Gain, Dynamic (-4 dB) / Clear Dynamics, Spectral Dynamics, and Delete; middle-click any node for instant 1-click Band Bypass; `Shift + Drag` for `0.15x` fine precision with zero jump; `Ctrl` / Piano Roll `Snap` for semitone quantization; keyboard-focus safety via `ctx.wants_keyboard_input()`.
  - **Custom User Preset Naming, Deletion, A/B & Undo/Redo**: 12 built-in Factory Presets + named offline `.peq` User Presets with inline custom naming, filename sanitization, per-preset `×` deletion, backwards-compatible `.peq` parsing (`gain_scale_pct` defaults to `100%`), `[A / B]` comparison bank with `[Copy]`, and 64-step drag-coalesced Undo/Redo (`Ctrl+Z` / `Ctrl+Y`).
- **Phantom Launcher (`phantom-launcher`)**:
  - Cross-platform offline-first Desktop GUI and headless CLI (`--cli`, `--list`, `--install`, `--uninstall`, `--verify`) with streaming 64 KiB SHA-256 verification and non-blocking Windows Cubase detection.
- **CI/CD & Packaging**:
  - Dual-licensed (`MIT OR Apache-2.0`), Windows Cubase installer scripts (`install_to_cubase.ps1` / `.bat`), explicit Windows DLL export table (`exports.def`), automated multi-OS CI (`ci.yml`), and multi-platform release workflow (`release.yml`) with automated Windows PE export verification (`GetPluginFactory` & `clap_entry`) and `SHA256SUMS.txt`.

[Unreleased]: https://github.com/intity01/phantom-dsp-suite/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/intity01/phantom-dsp-suite/releases/tag/v0.1.0
