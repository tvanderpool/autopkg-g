# Copilot Instructions for autopkg

This document provides guidelines and instructions for GitHub Copilot when working on the `autopkg` project.

## Project Overview

`autopkg` is a Rust CLI tool that checks for and installs updates for applications defined in a YAML configuration file. The project follows a composable architecture with clear separation between **Fetchers** (where updates come from) and **Installers** (how updates are installed).

## Architecture

The project is organized into the following modules:

- **config.rs** - Configuration structures and YAML parsing
- **types.rs** - Common types (UpdateCheck, FetchResult)
- **fetcher/** - Fetcher implementations
  - `mod.rs` - Fetcher trait and factory
  - `github.rs` - GitHub Releases fetcher
- **installer/** - Installer implementations
  - `mod.rs` - Installer trait and factory
  - `deb.rs` - Debian package installer
- **main.rs** - CLI entry point and orchestration

### Design Principles

1. **Modularity**: Fetchers and installers are pluggable via traits
2. **Error isolation**: Failure on one application doesn't prevent processing others
3. **Clear interfaces**: Traits define minimal, focused contracts
4. **Factory pattern**: Use `create_fetcher()` and `create_installer()` to instantiate implementations

## Development Guidelines

### Building and Testing

```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run tests (when available)
cargo test

# Run with debug logging
RUST_LOG=autopkg=debug cargo run -- run --dry-run
```

### Code Style

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features
```

**Always run `cargo fmt` before committing code.**

### Code Conventions

- Use `anyhow::Result<T>` for functions that return errors
- Log to `stderr` using the `log` crate (info!, warn!, error!, debug!)
- Follow Rust naming conventions (snake_case for functions/variables, CamelCase for types)
- Keep functions focused and small
- Add documentation comments (`///`) for public APIs
- Prefer explicit error messages over generic ones

### Configuration Format

The tool uses YAML configuration with support for both shorthand and explicit syntax. See `config.rs` for custom deserializers.

Example:
```yaml
applications:
  - name: my-app
    fetcher:
      type: github
      repo: owner/repo
      file_pattern: "*.deb"
    installer: deb  # shorthand
    # or installer:
    #   type: deb    # explicit
```

### Adding New Fetchers

1. Create a new file in `src/fetcher/` (e.g., `http.rs`)
2. Implement the `Fetcher` trait
3. Update `create_fetcher()` in `src/fetcher/mod.rs` to dispatch to your implementation
4. Extend `FetcherConfig` in `config.rs` if additional fields are needed
5. Document the new fetcher in README.md

### Adding New Installers

1. Create a new file in `src/installer/` (e.g., `appimage.rs`)
2. Implement the `Installer` trait with:
   - `should_check_for_update()` - returns UpdateCheck
   - `install()` - performs installation
3. Update `create_installer()` in `src/installer/mod.rs`
4. Extend `InstallerConfig` in `config.rs` if needed
5. Document the new installer in README.md

## Dependencies

Key dependencies:
- `clap` - CLI argument parsing
- `serde` / `serde_yaml` - Configuration parsing
- `reqwest` - HTTP requests (blocking API)
- `anyhow` - Error handling
- `log` / `env_logger` - Logging
- `regex` - Pattern matching
- `glob` - File pattern matching

## Testing

Currently, the project has minimal test infrastructure. When adding tests:
- Place unit tests in the same file as the code using `#[cfg(test)]` modules
- Use `cargo test` to run tests
- Mock external dependencies where practical

## Common Tasks

### Check code compiles
```bash
cargo check
```

### Run the tool
```bash
cargo run -- run --config autopkg.yml
cargo run -- run --dry-run
cargo run -- show-config
```

### Update dependencies
```bash
cargo update
```

## Important Notes

- The project targets Debian-based systems for the `DebInstaller`
- `dpkg` and optionally `sudo` must be available in PATH
- Logging goes to `stderr` to be compatible with systemd and console
- The tool supports `--log-level` flag (error, warn, info, debug, trace)
- Version comparison is basic - complex version schemes may not work correctly
- GitHub API calls are unauthenticated and subject to rate limits

## Pull Request Guidelines

When submitting changes:
1. Ensure code compiles without warnings (`cargo build`)
2. Run `cargo fmt` to format code
3. Run `cargo clippy` and address any warnings
4. Test manually with sample configurations
5. Update README.md if adding new features
6. Keep changes focused and minimal
7. Write clear commit messages describing what and why

## Troubleshooting

- **Build errors**: Ensure Rust toolchain 1.70+ is installed
- **dpkg errors**: Ensure running on Debian-based system with dpkg available
- **Permission errors**: DebInstaller prefers `sudo` for privilege escalation
- **GitHub rate limits**: Consider adding authentication token support (future enhancement)
