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

The `rustmt5` binary will be available in your `$PATH`.

## Usage

### Help and version

```bash
rustmt5 --help              # list subcommands and global options
rustmt5 --version           # print package version (from Cargo.toml)
rustmt5 compile --help      # compile subcommand options
rustmt5 test --help         # test subcommand options
```

### Compile an MQL5 file

```bash
rustmt5 compile MyEA.mq5
```

The compiler will produce a `MyEA.ex5` file alongside the source. Errors and warnings from MetaEditor are printed to the terminal.

#### Copy output to a specific directory

Use `--output` (or `-o`) to copy the compiled `.ex5` to a directory of your choice:

```bash
rustmt5 compile MyEA.mq5 --output ./build
```

The `.ex5` is still produced next to the source (MetaEditor's behavior), then copied to the specified directory.

### Run the strategy tester

```bash
rustmt5 test backtest.ini
```

This launches MT5's strategy tester headlessly using the provided configuration. Results are written to MT5's reports directory.

> **Note:** There is no `--output` flag for `test` because the `.ini` file's `Report` field already controls where MT5 writes its report. Set `Report=my_report` in your `.ini` to control the output filename.

### Example `.ini` config

```ini
[Tester]
Expert=MyEA
Symbol=EURUSD
Period=H1
FromDate=2024.01.01
ToDate=2024.12.31
Model=1
Deposit=10000
Currency=USD
Leverage=100
Optimization=0
Report=backtest_result
ReplaceReport=1
ShutdownTerminal=1
```

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

## Project structure

```
src/
├── main.rs       # Entry point
├── cli.rs        # CLI argument parsing (clap)
├── error.rs      # Error types
├── mt5.rs        # MT5 binary discovery
├── wine.rs       # Mac-to-Wine path translation
├── compile.rs    # Compile subcommand
└── test.rs       # Test subcommand
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

## References

- [MQL5 Language Reference](https://www.mql5.com/en/docs) — full MQL5 API documentation
- [MQL5 Programming for Traders](https://www.mql5.com/en/book) — comprehensive MQL5 programming book
- [MT5 Command Line Backtest Discussion](https://www.mql5.com/en/forum/499821) — community thread on running backtests from the command line
- [MQL Clangd (VS Code Extension)](https://marketplace.visualstudio.com/items?itemName=ngSoftware.mql-clangd) — MQL4/MQL5 IntelliSense and compilation in VS Code, including Wine/macOS support
- [MT5 Platform Start & Configuration Files](https://www.metatrader5.com/en/terminal/help/start_advanced/start#configuration_file) — official documentation on `.ini` config file parameters for the strategy tester

## License

MIT
