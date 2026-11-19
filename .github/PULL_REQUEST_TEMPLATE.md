## Summary

<!-- Briefly describe what this pull request changes and why. -->

## Related Issue

<!-- Link any related issue, e.g. Closes #123 -->

## Type of Change

- [ ] `feat`: New DSP feature, UI capability, or plugin
- [ ] `fix`: Bug fix (DSP math, UI, DAW compatibility, installer)
- [ ] `perf`: Performance or CPU/memory optimization
- [ ] `refactor`: Structural change or file decomposition
- [ ] `docs`: Documentation or governance update
- [ ] `test` / `ci` / `chore`: Testing, CI/CD, or maintenance

## Testing & Verification

<!-- Describe the tests run and (if applicable) DAW hosts / sample rates tested. -->

- [ ] `cargo test -p phantom-dsp --release -- --nocapture`
- [ ] `cargo test -p phantom-launcher --release -- --nocapture`
- [ ] `cargo check --workspace`
- [ ] `cargo fmt --all -- --check`

## Engineering Standards Checklist

Please verify every applicable item before requesting review:

- [ ] **Modularity (<= 300 lines/file)**: Every modified/added `.rs` file in `crates/*/src/`, `plugins/*/src/`, and `apps/*/src/` is <= 300 lines.
- [ ] **Zero-Allocation Audio Thread**: No heap allocations (`Vec`, `Box`, `String`, `.collect()`, `.clone()` on heap types), blocking locks, or syscalls inside audio `process()` paths.
- [ ] **Real-Time Safety**: Guarded against NaN/Inf/denormals; host latency updated cleanly via `set_latency_samples` when applicable.
- [ ] **Clean-Room DSP**: All algorithms and assets are original / clean-room implementations (no proprietary reverse engineering).
- [ ] **Zero Telemetry & Offline-First**: Zero analytics, zero silent startup network requests, and no networking crates added to audio plugins.
- [ ] **Cross-Compilation Safe**: `assert_process_allocs` is NOT enabled in default `nih_plug` features; pure-Rust launcher dependencies remain compatible with `x86_64-pc-windows-gnu`.
- [ ] **Licensing**: Contributions are submitted under the repository's dual license (`MIT OR Apache-2.0`).
