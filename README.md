# rustmt5

A command-line tool for compiling MQL5 files and running MT5 strategy tests on macOS — without touching the MetaTrader GUI.

MT5 on macOS runs inside a bundled Wine wrapper. `rustmt5` finds the relevant binaries automatically, translates file paths to Wine format, and invokes the compiler or strategy tester for you.

## Prerequisites

- **MetaTrader 5** installed on macOS (official installer from MetaQuotes)
- **Rust toolchain** (`rustup`, `cargo`) for building from source

## Installation

```bash
git clone https://github.com/felixLandlord/rustmt5.git
cd rustmt5
cargo install --path .
```

The `rustmt5` binary is installed to `~/.cargo/bin/rustmt5`. If that directory is not on your `PATH`, add it to your shell profile:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Reload your shell (or `source ~/.zshrc`) and verify:

```bash
which rustmt5
rustmt5 --version
```

## Usage

### Help and version

```bash
rustmt5 --help              # list subcommands and global options
rustmt5 --version           # print package version (from Cargo.toml)
rustmt5 compile --help      # compile subcommand options
rustmt5 test --help         # test subcommand options
rustmt5 metrics --help      # metrics subcommand options
rustmt5 score --help        # score subcommand options
```

### Compile an MQL5 file

```bash
rustmt5 compile MyEA.mq5
```

MetaEditor writes `.ex5` and `.log` next to the source during compile; after every run (success or failure) `rustmt5` moves them into **`output/compile/`** next to the `.mq5` (e.g. `examples/output/compile/strategy.log`). The log is still printed to the terminal and used to detect errors.

**Encoding note:** MetaEditor commonly writes `.log` files as **UTF‑16LE with a BOM**. `rustmt5` detects and decodes this automatically.

#### Deploy to MT5 Experts with `--output`

Use `--output` (or `-o`) to **also** copy the `.ex5` to MT5's Experts folder (or another path) for testing — separate from `output/compile/`:

```bash
# Copy to MT5's Experts folder (default Wine install path)
rustmt5 compile MyEA.mq5 --output

# Copy to a custom directory
rustmt5 compile MyEA.mq5 --output ./build
```

With `--output` alone (no path), the destination is `RUSTMT5_EXPERTS_DIR` if set, otherwise:

`$HOME/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5/MQL5/Experts/rustmt5_ea/`

The `rustmt5_ea/` subfolder keeps your compiled EAs organised separately from MT5's built-in examples.

The Experts directory is created automatically if it does not exist.

### Run the strategy tester

```bash
rustmt5 test backtest.ini
```

This launches MT5's strategy tester headlessly. When the test finishes, report files (`.htm` + `.png`) are copied from the MT5 install directory into **`output/test/`** next to the `.ini` (e.g. `examples/output/test/strategy_report.htm`).

```bash
rustmt5 test examples/backtest.ini

# Copy reports somewhere else instead
rustmt5 test examples/backtest.ini --input ./reports
```

`--input` without a path uses `output/test/` (same as the default). The directory is created if it does not exist.

MT5 always writes reports to its install directory (next to `terminal64.exe`). The report name and subfolder come from the `Report=` key in your `.ini`. With `Report=rustmt5_report/strategy_report`, MT5 writes:

```
…/MetaTrader 5/rustmt5_report/strategy_report.htm
…/MetaTrader 5/rustmt5_report/strategy_report.png   (+ chart images)
```

`rustmt5` copies all of those files to the destination after a successful run.

Wine and MoltenVK noise (Vulkan extension lists, toolbar/HID messages) is suppressed via `WINEDEBUG=-all`, `MVK_CONFIG_LOG_LEVEL=0`, and output filtering.

Before running a test, `rustmt5 test` checks that the Expert from your `.ini` exists under `MQL5/Experts/` (e.g. `Expert=rustmt5_ea\strategy` → `…/MQL5/Experts/rustmt5_ea/strategy.ex5`). If it is missing, the test exits with an error before launching MT5.

Also ensure:

- MT5 is **not already open** (only one terminal instance at a time)
- The compiled EA exists at the right path.
- Historical data exists for the `Symbol` and `Period` in your `.ini`

### Extract metrics from a report

```bash
rustmt5 metrics examples/output/test/strategy_report.htm
```

Parses the MT5 HTML report (UTF-8 or UTF-16LE), validates 65 result metrics plus settings, and writes JSON to **`output/metrics/{report_name}.json`** by default.

```bash
rustmt5 metrics report.htm -o ./my_metrics.json
rustmt5 metrics report2.htm --append ./my_metrics.json   # adds report id = max + 1
```

### Score metrics

```bash
rustmt5 score examples/score.toml output/metrics/strategy_report.json
```

Loads a TOML config (`weighted_average`, `geometric_mean`, `harmonic_mean`, `exponential_weighted`, or `weighted_sum`), normalizes metrics to 0–100, prints a breakdown, and reports PASS/FAIL against `pass_threshold`.

Typical workflow: `compile` → `test` → `metrics` → `score`.

### Example `.ini` config

```ini
[Tester]
Expert=rustmt5_ea/MyEA
Symbol=EURUSD
Period=H1
FromDate=2024.01.01
ToDate=2024.12.31
Model=1
Deposit=10000
Currency=USD
Leverage=100
Optimization=0
Report=rustmt5_report/backtest_result
ReplaceReport=1
ShutdownTerminal=1
```

`Expert=rustmt5_ea/MyEA` matches the `rustmt5_ea/` subfolder that `compile --output` targets. `Report=rustmt5_report/backtest_result` keeps reports in their own subfolder inside the MT5 install directory.

### Example `.mq5` file

```mql5
void OnStart()
{
   Print("Hello from MQL5!");
}
```

## How it works

1. **Path discovery** — `rustmt5` locates Wine inside `MetaTrader 5.app` (`Contents/SharedSupport/wine/bin/wine64`) and MT5 binaries in the Wine prefix at `~/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5/`.
2. **Path translation** — macOS paths are converted to Wine's `Z:\` drive format (e.g. `/Users/you/ea.mq5` becomes `Z:\Users\you\ea.mq5`).
3. **Execution** — Wine runs with `WINEPREFIX` set to the MetaQuotes prefix, then invokes MetaEditor or the terminal with the translated path.

## Environment variables

Auto-discovery works on a standard Mac MT5 install. Override any path if yours differs (CrossOver, multiple terminals, etc.):

| Variable | Description |
|---|---|
| `RUSTMT5_WINEPREFIX` | Wine prefix directory (`net.metaquotes.wine.metatrader5`) |
| `RUSTMT5_WINE` | Path to the `wine64` binary |
| `RUSTMT5_EDITOR` | Path to `MetaEditor64.exe` |
| `RUSTMT5_TERMINAL` | Path to `terminal64.exe` |
| `RUSTMT5_EXPERTS_DIR` | Directory for `compile --output` (no path). Defaults to `…/MQL5/Experts/` under the Wine prefix |

### Manual path overrides (example)

If auto-discovery fails, set these in your shell (adjust `$HOME` if needed):

```bash
export RUSTMT5_WINE="/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64"
export RUSTMT5_TERMINAL="$HOME/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5/terminal64.exe"
export RUSTMT5_EDITOR="$HOME/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5/MetaEditor64.exe"
```

Optional: pin the Wine prefix explicitly (usually inferred automatically):

```bash
export RUSTMT5_WINEPREFIX="$HOME/Library/Application Support/net.metaquotes.wine.metatrader5"
```

Default Experts folder used by `compile --output` (optional override):

```bash
export RUSTMT5_EXPERTS_DIR="$HOME/Library/Application Support/net.metaquotes.wine.metatrader5/drive_c/Program Files/MetaTrader 5/MQL5/Experts/"
```

## Project structure

```
src/
├── main.rs         # Entry point
├── cli.rs          # CLI (compile, test, metrics, score)
├── error.rs        # Error types
├── text_decode.rs  # UTF-8 / UTF-16LE text files
├── mt5.rs          # MT5 binary discovery
├── wine.rs         # Mac-to-Wine path translation
├── wine_output.rs  # Wine stderr filtering
├── compile.rs      # Compile subcommand
├── test.rs         # Test subcommand
├── metrics/        # HTML → JSON extraction
└── score/          # TOML config scoring
```

## Distribution

### Install from source

```bash
cargo install --path .
```

### Build a release binary

```bash
cargo build --release
```

The optimized binary will be at `target/release/rustmt5`. Copy it anywhere on your `$PATH`.

### Run tests

```bash
cargo test
```

### Publishing to crates.io

Before publishing, ensure your `Cargo.toml` has the required metadata:

```toml
[package]
name = "rustmt5"
version = "0.1.0"
edition = "2021"
description = "CLI tool for compiling MQL5 and running MT5 strategy tester on macOS"
license = "MIT"
repository = "https://github.com/felixLandlord/rustmt5"
keywords = ["mt5", "mql5", "metatrader", "trading", "cli"]
categories = ["command-line-utilities", "development-tools"]
```

Then publish:

```bash
cargo login  # authenticate with your crates.io API token
cargo publish --dry-run  # verify everything is in order
cargo publish
```

Once published, anyone can install it with:

```bash
cargo install rustmt5
```

## Example Usage of Commands (compile, test, metrics, score)

```cli
# 1. Compile (deploys .ex5 to MT5 Experts; artifacts → examples/output/compile/)
rustmt5 compile examples/strategy.mq5 --output

# 2. Backtest (reports → examples/output/test/ by default)
rustmt5 test examples/backtest.ini

# 3. Extract metrics → examples/output/metrics/
rustmt5 metrics examples/output/test/strategy_report.htm \
  -o examples/output/metrics/strategy_report.json

# 4. Score (prints to terminal; reads config + metrics from examples/)
rustmt5 score examples/score.toml examples/output/metrics/strategy_report.json
```

## Troubleshooting

**"MT5 installation not found"**
MT5 is not installed at the expected location. Use environment variables to point to your installation.

**"failed to convert path to Wine format"**
The file path could not be canonicalized. Make sure the file exists and the path is valid.

**Compile log vs Wine exit code**
MetaEditor writes a `.log` file next to your `.mq5` (e.g. `strategy.log`) when using `/log`. `rustmt5` reads that file and treats `Result: 0 errors, ...` as success even if Wine exits with a non-zero status. Wine/HID messages on stderr are suppressed when a log file is present so you see the compiler output instead.

**Compiler runs but produces no output**
The Wine path translation may be incorrect. Double-check that `wine64` exists and runs:

```bash
"/Applications/MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64" --version
export WINEPREFIX="$HOME/Library/Application Support/net.metaquotes.wine.metatrader5"
```

**"MT5 must not already be running"**
Only one instance of MT5 can run at a time. Quit any running MT5 instance before using `rustmt5 test`.

**`rustmt5 test` fails with a non-zero exit code (e.g. 189)**
The strategy tester did not complete successfully. Check that the EA is installed under `MQL5/Experts/`, the `.ini` `[Tester]` settings are valid, and you have history for the symbol/timeframe. Wine GUI spam in the terminal is harmless and is filtered by `rustmt5`.

## References

- [MQL5 Language Reference](https://www.mql5.com/en/docs) — full MQL5 API documentation
- [MQL5 Programming for Traders](https://www.mql5.com/en/book) — comprehensive MQL5 programming book
- [MT5 Command Line Backtest Discussion](https://www.mql5.com/en/forum/499821) — community thread on running backtests from the command line
- [MQL Clangd (VS Code Extension)](https://marketplace.visualstudio.com/items?itemName=ngSoftware.mql-clangd) — MQL4/MQL5 IntelliSense and compilation in VS Code, including Wine/macOS support
- [MT5 Platform Start & Configuration Files](https://www.metatrader5.com/en/terminal/help/start_advanced/start#configuration_file) — official documentation on `.ini` config file parameters for the strategy tester

## License

MIT
