# Repository Guidelines

## Project Structure & Module Organization

EasyCut is a Rust 2024 Cargo workspace with three members:

- `easycut-core/` contains the binary entry point (`src/main.rs`).
- `easycut-media/` contains video decoding, metadata, and frame-buffer types.
- `easycut-ui/` contains the `eframe`/`egui` desktop interface.
- `README.md` documents supported platforms and basic usage; `QUESTION.md` records design decisions and resolved development questions.
- `frame1.png` and `sample_file.mp4` are local sample assets for manual media/UI checks. Build output is generated under `target/` and should not be committed.

Keep crate-specific logic in its owning member and use path dependencies for cross-crate APIs. Add unit tests beside the implementation they exercise; add integration tests under that crate's `tests/` directory when a public API needs end-to-end coverage.

## Build, Test, and Development Commands

Run commands from the workspace root:

```bash
cargo check                 # Fast workspace type and dependency check
cargo build                 # Debug build
cargo build --release       # Optimized build
cargo test                  # Run all workspace tests
cargo run -p easycut-core   # Launch the desktop application
cargo fmt --all -- --check  # Verify formatting
cargo clippy --workspace --all-targets -- -D warnings  # Lint all targets
```

Use `cargo test -p <crate>` or `cargo check -p <crate>` when iterating on one member. Video playback and file dialogs require a desktop environment and the native dependencies used by `video-rs`.

## Coding Style & Naming Conventions

Use `rustfmt` defaults, four-space indentation, and idiomatic Rust naming: `snake_case` for functions, variables, and modules; `UpperCamelCase` for types and traits; `SCREAMING_SNAKE_CASE` for constants. Prefer `Result`/`Option` propagation over panics in library code, document public types and non-obvious media conversions, and keep UI-only behavior in `easycut-ui`.

## Testing Guidelines

There is currently no committed test suite. New media parsing, timestamp, and frame-conversion behavior should receive focused unit tests, with fixtures kept small and deterministic. Run `cargo test` before opening a PR; manually verify the UI with `cargo run -p easycut-core` and the sample MP4 when changing playback or rendering.

## Commit & Pull Request Guidelines

Existing commits use short, imperative, uppercase prefixes such as `MOD:`, `REFACTOR:`, and `DOC:` (for example, `MOD: optimize`). Follow that pattern, keep the subject focused, and avoid mixing unrelated changes. Pull requests should explain the behavior changed, identify affected crates, list validation commands, link an issue or design note when applicable, and include a screenshot or short recording for visible UI changes.
