# autopkg

`autopkg` is a small Rust CLI tool that checks for and installs updates for applications defined in a YAML configuration file. It is designed around a composable architecture with:

- **Fetchers** – where updates come from (e.g. GitHub releases)
- **Installers** – how updates are installed (e.g. `.deb` packages)
- **Main orchestrator** – loops over configured applications and drives fetchers/installers

Currently implemented:

- `GitHubFetcher` – uses the GitHub Releases API
- `DebInstaller` – installs `.deb` packages using `dpkg` (with optional `sudo`)

The design aims to make it easy to add more fetchers (e.g., other APIs) and installers (e.g., AppImage, tarballs, etc.).

---

## Features

- Read configuration from a YAML file (supports shorthand and explicit syntax)
- Check for and (optionally) install updates for multiple applications
- Installer-driven update policy (`Installer::should_check_for_update`)
- Fetcher-driven retrieval (`Fetcher::fetch_if_newer`)
- `--dry-run` support (download but don’t install)
- Logging to `stderr` (compatible with console and systemd)
- Basic semantic version comparison
- Error isolation: failure on one application does not prevent others from being processed
- Subcommand-based CLI using `clap`:
  - `autopkg run` – run update flow
  - `autopkg show-config` – show parsed configuration

---

## Installation

### Prerequisites

- Rust toolchain (1.70+ recommended): [Install Rust](https://www.rust-lang.org/tools/install)
- Debian-based system (for `DebInstaller`), with:
  - `dpkg` in `PATH`
  - `sudo` (optional, but preferred for privilege escalation)

### Build from source

Clone the repository and build:

```bash
git clone https://github.com/tvanderpool/autopkg-g.git
cd autopkg-g
cargo build --release
```

The resulting binary will be at:

```bash
target/release/autopkg
```

You can either run it from there, or install it somewhere in your `PATH`.

---

## Usage

`autopkg` uses a YAML configuration file that defines a list of applications, each with a fetcher and installer configuration.

By default, it looks for `autopkg.yml` in the current working directory. You can override this with `--config`.

### CLI overview

```bash
autopkg --help
```

Example help (simplified):

```text
autopkg
Auto-updater tool for applications defined in a YAML config.

Usage: autopkg [OPTIONS] <COMMAND>

Options:
  --log-level <LEVEL>  Log level (error, warn, info, debug, trace) [default: info]
  -h, --help           Print help
  -V, --version        Print version

Commands:
  run          Run update checks (and installs, unless --dry-run)
  show-config  Show the parsed configuration
  help         Print this message or the help of the given subcommand(s)
```

### `run` subcommand

Run update checks for all configured applications:

```bash
autopkg run
```

Options:

- `--config <PATH>` – Path to config file (default: `autopkg.yml`)
- `--dry-run` – Check for updates and download, but **do not** install anything

Examples:

```bash
# Use default config (autopkg.yml) with default log level (info)
autopkg run

# Use a custom configuration file
autopkg run --config /etc/autopkg.yml

# Dry-run (no installation), with more verbose logging
autopkg --log-level debug run --dry-run
```

### `show-config` subcommand

Parse and print the configuration (useful for debugging):

```bash
autopkg show-config
autopkg show-config --config /etc/autopkg.yml
```

This will log that the configuration was parsed and print the YAML representation to `stdout`.

---

## Configuration

`autopkg` configuration is a YAML file with the following top-level structure:

```yaml
applications:
  - name: ...
    fetcher: ...
    installer: ...
    package_name: ... # optional
    pinned: ...       # optional
```

### Application fields

- `name` (string, required): Logical name of the application.
- `fetcher` (object, required): Configuration for the fetcher.
- `installer` (string or object, required): Configuration for the installer.
- `package_name` (string, optional): Name used by the installer to query installed version (for `dpkg`, this is the package name).
  - Defaults to `name` if omitted.
- `pinned` (bool, optional): If `true`, the installer will **skip update checks** for this app.

### Fetchers

Currently supported: **GitHub releases**.

```yaml
fetcher:
  type: github
  repo: owner/repo
  file_pattern: "*.deb"
```

Fields:

- `type` (string, required): Must be `github` for the `GitHubFetcher`.
- `repo` (string, required): `owner/repo` on GitHub (e.g., `obsidianmd/obsidian-releases`).
- `file_pattern` (string, optional): Glob pattern to match assets in the latest release.
  - If omitted, defaults to `"*"`.

Behavior:

- Uses the GitHub API endpoint:  
  `https://api.github.com/repos/{owner}/{repo}/releases/latest`
- Matches assets against `file_pattern`.
- Downloads matched asset to the system temp directory with a unique filename.
- Compares the latest release version (from `tag_name`) to the installed version.
- Returns:
  - `None` if current version is up to date.
  - `Some(path)` if a newer asset was downloaded.

### Installers

Currently supported: **Debian `.deb`**.

`installer` supports both explicit and shorthand forms:

```yaml
# Full form
installer:
  type: deb

# Shorthand
installer: deb
```

Fields (full form):

- `type` (string, required): Must be `deb` for `DebInstaller`.

Behavior:

- Determines installed version using:

  ```bash
  dpkg -s <package_name>
  ```

- Reads the `Version:` field from `dpkg` output.
- If package is not installed, it treats the current version as `0.0.0`.
- If `pinned: true` is set on the application, the installer returns `UpdateCheck::No` and **skips** update checks.
- When installing:
  - Prefer `sudo dpkg -i <file>` if `sudo` is present.
  - Otherwise, use `dpkg -i <file>` directly.
  - Returns an error if the command exits with a non-zero status.

---

## Example configuration

The following is a complete example `autopkg.yml`:

```yaml
applications:
  - name: obsidian
    fetcher:
      type: github
      repo: obsidianmd/obsidian-releases
      file_pattern: "*.deb"
    installer:
      type: deb
    # Optional:
    # package_name: obsidian
    # pinned: false

  - name: some-app
    fetcher:
      type: github
      repo: owner/repo
      file_pattern: "*amd64.deb"
    installer: deb  # shorthand format
    # package_name: some-app
    # pinned: true
```

---

## Architecture

The project is structured around clear interfaces and composition.

### Configuration (`config.rs`)

Defines the configuration structures:

- `Config` – the root configuration with `applications: Vec<ApplicationConfig>`.
- `ApplicationConfig` – per-application configuration.
- `FetcherConfig` – configuration for fetchers.
- `InstallerConfig` – configuration for installers.
- Custom deserialization to support both full and shorthand installer syntax.

### Types (`types.rs`)

Defines common types:

- `enum UpdateCheck` – indicates whether and how to check for updates:
  - `UpdateCheck::No`
  - `UpdateCheck::Yes(String)` – includes the current installed version
- `FetchResult` – common result type for fetch operations.

### Fetchers (`fetcher` module)

- `trait Fetcher` – one method:

  ```rust
  fn fetch_if_newer(&self, current_version: &str) -> Result<Option<PathBuf>>;
  ```

- `create_fetcher` – factory that returns `Box<dyn Fetcher>`:

  - Currently supported:
    - `type = "github"` → `GitHubFetcher`

- `GitHubFetcher` (in `fetcher/github.rs`):

  - Calls GitHub Releases API to get `latest` release.
  - Extracts version from `tag_name` (supports `v1.2.3` style tags).
  - Performs simple semantic version comparison.
  - Finds an asset matching `file_pattern`.
  - Downloads the asset to `/tmp` (or equivalent temp dir).
  - Returns `Some(path)` if the latest version is newer; otherwise `None`.

### Installers (`installer` module)

- `trait Installer`:

  ```rust
  fn should_check_for_update(&self) -> Result<UpdateCheck>;
  fn install(&self, file_path: &Path) -> Result<()>;
  ```

- `create_installer` – factory that returns `Box<dyn Installer>`:

  - Currently supported:
    - `type = "deb"` → `DebInstaller`

- `DebInstaller` (in `installer/deb.rs`):

  - Uses `dpkg -s <package_name>` to discover installed version.
  - Respects `pinned` flag in the app config.
  - Treats missing packages as version `0.0.0`.
  - Runs installation using `sudo dpkg -i` or `dpkg -i`.

### Main flow (`main.rs`)

1. Parse CLI using `clap` with subcommands.
2. Initialize logging to `stderr` with `env_logger`.
3. Load and parse the YAML configuration.
4. For `run`:
   - Loop over applications:
     - Create installer and fetcher.
     - Ask installer whether and how to check for updates (`should_check_for_update`).
     - If `Yes(current_version)`:
       - Ask fetcher if there is a newer version (`fetch_if_newer`).
       - If a new file is returned:
         - Either log (when `--dry-run`), or call `installer.install`.
   - Errors for one app are logged but do not stop the others.
5. For `show-config`:
   - Load config and pretty-print it to stdout.

---

## Logging

Logging uses the `log` and `env_logger` crates, and all log output goes to `stderr`.

You can control log verbosity with `--log-level`:

- `error`
- `warn`
- `info` (default)
- `debug`
- `trace`

Examples:

```bash
autopkg --log-level debug run
autopkg --log-level trace show-config
```

You can also override via the standard `RUST_LOG` environment variable; `autopkg` will only set it if not already set.

---

## Extending `autopkg`

The architecture is intentionally modular:

### Adding a new fetcher

1. Create a new file in `src/fetcher/`, e.g. `http.rs`.
2. Implement the `Fetcher` trait.
3. Update `create_fetcher` in `src/fetcher/mod.rs` to dispatch on a new `type` string (e.g., `http`).
4. Extend `FetcherConfig` with any additional fields needed.

### Adding a new installer

1. Create a new file in `src/installer/`, e.g. `appimage.rs`.
2. Implement the `Installer` trait.
3. Update `create_installer` in `src/installer/mod.rs` to dispatch on a new `type` string (e.g., `appimage`).
4. Extend `ApplicationConfig` or define new installer-specific configuration fields if necessary.

---

## Safety and limitations

- Installation currently assumes `.deb` packages and uses `dpkg`. Use caution, as updating system packages can break software if used incorrectly.
- No rollback mechanism is implemented.
- Version comparison is minimal and may not handle complex tagging schemes.
- GitHub API calls are currently unauthenticated; heavy usage may run into rate limits.
  - In future, a `GITHUB_TOKEN` environment variable or config option can be added.

---

## Development

Run tests (once you add them):

```bash
cargo test
```

Run with debug logging:

```bash
RUST_LOG=autopkg=debug cargo run -- run --dry-run
```

Format code:

```bash
cargo fmt
```

Lint (if you’ve added `clippy`):

```bash
cargo clippy --all-targets --all-features
```

---

## License

TBD. (Add your preferred license here, e.g. MIT/Apache-2.0.)

---

## Future ideas

- Add `AppImageInstaller` / `TarballInstaller`.
- Add more fetchers (custom APIs, direct URLs, etc.).
- Support authenticated GitHub requests via tokens.
- Cache downloaded assets.
- More advanced version comparison (e.g., using `semver` crate).
- Systemd unit for periodic checks and updates.