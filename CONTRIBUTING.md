# Contributing to llmusage

Thanks for your interest in contributing to **llmusage**! This guide will help you get up and running.

## Getting Started

### Prerequisites

- **Rust** (stable, latest) via [rustup](https://rustup.rs/)
- **Git**

### Fork, Clone, and Build

```bash
# Fork on GitHub, then:
git clone https://github.com/<your-username>/llmusage.git
cd llmusage

# Build
cargo build

# Run locally
cargo run -- summary --days 7
cargo run -- sync --provider claude_code
```

The project uses SQLite (bundled via `rusqlite`), so no external database setup is needed.

## Development Workflow

### Branch Naming

Create branches from `main` using these prefixes:

- `feature/` -- new functionality (e.g., `feature/aws-bedrock-collector`)
- `fix/` -- bug fixes (e.g., `fix/daily-json-output`)
- `docs/` -- documentation changes

### Code Style

Before submitting, make sure your code passes:

```bash
cargo fmt --check    # formatting
cargo clippy         # lints
cargo test           # tests
```

We use default `rustfmt` and `clippy` settings. Fix all warnings before submitting.

### Project Structure

```
src/
  main.rs           # CLI entry point (clap commands)
  lib.rs            # Public module declarations
  collectors/       # Provider-specific data collectors
    mod.rs          # Collector trait + registration
    claude_code.rs  # Local log-based collector
    anthropic.rs    # API-based collector
    ollama.rs       # API-based collector
    ...
  config.rs         # Configuration management
  costs.rs          # Model pricing (LiteLLM integration)
  db.rs             # SQLite storage layer
  display.rs        # Table/output formatting
  models.rs         # Shared data types (UsageRecord, etc.)
```

## Adding a New Provider/Collector

This is the most common type of contribution. Each collector implements the `Collector` trait:

```rust
#[async_trait]
pub trait Collector: Send + Sync {
    fn name(&self) -> &str;
    async fn collect(&self) -> Result<Vec<UsageRecord>>;
}
```

### Steps

1. **Create** `src/collectors/your_provider.rs` with a struct that implements `Collector`.
2. **Return** a `Vec<UsageRecord>` from `collect()`. Each record needs at minimum: `provider`, `model`, `input_tokens`, `output_tokens`, `recorded_at`, and `collected_at`.
3. **Register** your collector in `src/collectors/mod.rs`:
   - Add `pub mod your_provider;` at the top.
   - Add a block in `get_collectors()` that instantiates it (conditionally, based on config or auto-detection).
4. **Add config fields** in `src/config.rs` if your provider needs API keys or settings.
5. **Add pricing** entries in `src/costs.rs` if the provider's models aren't already covered by LiteLLM.

Look at `ollama.rs` for a simple API-based example, or `claude_code.rs` for a local log-based example.

### Collector Types

- **API-based** (e.g., Anthropic, OpenAI, Gemini): Require an API key in config. Only instantiated when the key is present.
- **Local log-based** (e.g., Claude Code, Codex, Gemini CLI): Read from local files/databases. Auto-detected based on whether the expected paths exist.

## Testing

```bash
cargo test              # run all tests
cargo test -- --nocapture  # see println output
```

When adding new code, please:

- Add unit tests for parsing logic and data transformations.
- Use `#[cfg(test)]` modules within the same file.
- If your collector parses a specific file format, include a small fixture or inline test data.

## Submitting Changes

### Pull Request Process

1. Ensure `cargo fmt`, `cargo clippy`, and `cargo test` all pass.
2. Open a PR against `main` with a clear title and description.
3. Describe **what** changed and **why**. If it closes an issue, reference it (`Closes #N`).
4. Keep PRs focused -- one feature or fix per PR.

### Commit Messages

Use conventional-style messages:

```
feat: add AWS Bedrock collector
fix: handle empty session files in claude_code collector
docs: update README with new provider list
```

Keep the first line under 72 characters. Add a blank line and more detail in the body if needed.

## Reporting Issues

### Bug Reports

Open an issue with:

- What you expected vs. what happened
- Steps to reproduce
- Your OS, Rust version (`rustc --version`), and llmusage version (`llmusage --version`)
- Relevant error output

### Feature Requests

Open an issue describing:

- The use case or problem you're trying to solve
- Your proposed solution (if any)
- Which providers or commands are affected

### Security Vulnerabilities

Do **not** open a public issue. Instead, email [rijal.it@gmail.com](mailto:rijal.it@gmail.com) with details. We will respond within 48 hours.

## Code of Conduct

This project follows the [Contributor Covenant v2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). By participating, you agree to uphold a welcoming, inclusive, and harassment-free environment for everyone.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
