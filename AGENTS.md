# Repository Guidelines

## Project Structure & Module Organization

This is a Rust 2024 Cargo project that builds the `clockify` binary from `src/main.rs`. Core API, config, models, output, time parsing, resolution, and status cache logic live in top-level `src/*.rs` modules. Command handlers are grouped under `src/commands/` by CLI subcommand, and the terminal UI is under `src/tui/`. Static project assets, such as the README demo image, live in `assets/`. The bundled agent skill is in `skill/SKILL.md`. Release packaging is configured through `dist-workspace.toml` and the generated GitHub workflow in `.github/workflows/release.yml`.

## Build, Test, and Development Commands

- `cargo build`: compile the local debug binary.
- `cargo run -- <command>`: run the CLI from source, for example `cargo run -- status --json`.
- `cargo run`: open the TUI when attached to a terminal.
- `cargo test`: run unit tests embedded in source modules.
- `cargo clippy --all-targets -- -D warnings`: run lint checks and fail on warnings.
- `cargo fmt`: format all Rust code with rustfmt defaults.
- `cargo install --path .`: install the local `clockify` binary for manual testing.

## Coding Style & Naming Conventions

Follow standard rustfmt formatting with 4-space indentation. Use `snake_case` for functions, modules, variables, and command files; use `PascalCase` for structs, enums, and enum variants. Keep command-specific behavior in the matching file under `src/commands/`, shared Clockify API shapes in `src/models.rs`, and reusable parsing or lookup logic in `src/time.rs` or `src/resolve.rs`. Prefer `anyhow::Result` for fallible command flows, matching the existing code.

## Testing Guidelines

Tests currently live inline in `#[cfg(test)] mod tests` blocks, especially for parsing, config, output, and resolution behavior. Add focused unit tests beside the logic being changed. Name tests after the behavior under test, such as `parses_yesterday_time` or `resolves_unique_suffix`. Run `cargo test` before submitting, and run clippy for changes that touch command flow, API calls, or TUI state.

## Commit & Pull Request Guidelines

Recent history uses short, imperative, lowercase commit subjects such as `streamlined README` and `added agent skills`. Keep commits focused and describe the user-visible change. Pull requests should include a brief summary, test results, linked issues when applicable, and screenshots or terminal output for visible CLI/TUI changes.

## Security & Configuration Tips

Do not commit API keys, local config, or cache files. The CLI reads configuration from `~/.config/clockify/config.toml`, cache data from `~/.cache/clockify/`, and can resolve credentials from `CLOCKIFY_API_KEY` or a 1Password reference.
