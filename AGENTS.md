# AGENTS

## Project quick start
- Build the binary: `cargo build --release`
- Basic check: `cargo check --all`

## Tests
- Unit/regular tests: `cargo test` (if needed for local changes)
- Integration tests live under `tests/` and are exercised via `cargo xwin` commands in CI.
- CI integration tests reference: `.github/workflows/CI.yml` job `Test Suite`, steps like `xwin build - x86_64`, `xwin run - x86_64`, and `xwin test - x86_64`.

## Formatting and linting
- Format: `cargo fmt --all -- --check`
- Lint: `cargo clippy --all-features`

## Notes
- The integration test crates are in `tests/*` and are built/run through `cargo run --release xwin ...` in CI.
