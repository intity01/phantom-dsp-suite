# Security Policy

## Privacy, Zero-Telemetry & Offline-First Guarantee

**Phantom DSP Suite** is engineered from the ground up as a **100% offline, zero-telemetry** audio software suite:

- **Zero Network Code in Audio Plugins**: `phantom-dsp`, `phantom-ui`, and `phantom-eq` contain **zero networking dependencies** and perform **zero network requests**. No license servers, no dongles, no analytics, no crash-report phone-home, and no background updaters.
- **Offline-First Desktop Hub**: `phantom-launcher` operates completely offline by default. It never initiates automatic network calls on startup or in the background. Any update/release lookup is strictly opt-in and user-triggered.
- **Cryptographic SHA-256 Verification**:
  - Official GitHub Releases publish a signed/verified `SHA256SUMS.txt` manifest covering every distributed archive.
  - `phantom-launcher` computes real SHA-256 digests (via pure-Rust `sha2`) locally to compare installed VST3/CLAP binaries against source artifacts (`phantom-launcher --verify phantom-eq`).
- **Memory Safety**: Core DSP algorithms, UI widgets, and installer tooling are written in safe Rust (`edition = "2021"`), minimizing buffer-overflow, use-after-free, and out-of-bounds memory corruption risks inside DAW host processes.

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| `0.1.x` | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in any Phantom DSP Suite crate, plugin, installer script, or CI/release workflow — including:

- Memory-safety issues or FFI boundary bugs inside VST3/CLAP exports
- Crafted DAW project / preset inputs causing panics, NaN/Inf audio blowups, or host crashes
- Path traversal, symlink attacks, or privilege-escalation risks in `phantom-launcher` or installer scripts
- Supply-chain, build reproducibility, or checksum-verification issues

**Please do not open a public GitHub Issue for undisclosed vulnerabilities.**

Instead, report it privately using **GitHub Security Advisories**:

1. Navigate to the repository's **Security** tab:
   `https://github.com/intity01/phantom-dsp-suite/security/advisories/new`
2. Provide:
   - Affected component (`phantom-dsp`, `phantom-ui`, `phantom-eq`, `phantom-launcher`, or build/scripts) and version/commit hash
   - Operating system, architecture, and DAW host (if applicable)
   - Clear reproduction steps or a minimal proof-of-concept
   - Potential impact assessment

### Response Timeline

- **Acknowledgment**: Within 72 hours of submission.
- **Triage & Assessment**: Within 7 days, including severity evaluation and remediation plan.
- **Fix & Coordinated Disclosure**: Security patches are prioritized and released with an advisory credit (if desired) once a fix is verified.

## Verifying Release Artifacts

After downloading a release archive and `SHA256SUMS.txt` from [GitHub Releases](https://github.com/intity01/phantom-dsp-suite/releases):

```bash
# Linux / macOS
sha256sum -c SHA256SUMS.txt --ignore-missing

# Windows (PowerShell)
Get-FileHash .\phantom-dsp-suite-*.zip -Algorithm SHA256
```

To verify an installed plugin binary offline against a local `dist/` build:

```bash
phantom-launcher --verify phantom-eq
```
