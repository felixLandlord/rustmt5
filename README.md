# rustmt5

A command-line tool for compiling MQL5 files and running MT5 strategy tests on macOS — without touching the MetaTrader GUI.

MT5 on macOS runs inside a bundled Wine wrapper. `rustmt5` finds the relevant binaries automatically, translates file paths to Wine format, and invokes the compiler or strategy tester for you.

## Prerequisites

- **MetaTrader 5** installed on macOS (official installer from MetaQuotes)
- **Rust toolchain** (`rustup`, `cargo`) for building from source

## Installation

```bash
git clone https://github.com/youruser/rustmt5.git
cd rustmt5
cargo install --path .
```

The `rustmt5` binary will be available in your `$PATH`.

## Usage

### Compile an MQL5 file

```bash
rustmt5 compile MyEA.mq5
```

The compiler will produce a `MyEA.ex5` file alongside the source. Errors and warnings from MetaEditor are printed to the terminal.

### Run the strategy tester

```bash
rustmt5 test backtest.ini
```

This launches MT5's strategy tester headlessly using the provided configuration. Results are written to MT5's reports directory.

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
```

### Example `.mq5` file

```mql5
void OnStart()
{
   Print("Hello from MQL5!");
}
```

## How it works

1. **Path discovery** — `rustmt5` searches `/Applications/MetaTrader 5.app` and `~/Applications/MetaTrader 5.app` for `wine64`, `metaeditor64.exe`, and `terminal64.exe`.
2. **Path translation** — macOS paths are converted to Wine's `Z:\` drive format (e.g. `/Users/you/ea.mq5` becomes `Z:\Users\you\ea.mq5`).
3. **Execution** — the appropriate binary is invoked through Wine with the translated path.

## Environment variables

Override auto-discovery by setting any of these:

| Variable | Description |
|---|---|
| `RUSTMT5_WINE` | Path to the `wine64` binary |
| `RUSTMT5_EDITOR` | Path to `metaeditor64.exe` |
| `RUSTMT5_TERMINAL` | Path to `terminal64.exe` |

This is useful if you have a non-standard MT5 installation (e.g. via CrossOver) or multiple versions installed.

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

## Troubleshooting

**"MT5 installation not found"**
MT5 is not installed at the expected location. Use environment variables to point to your installation.

**"failed to convert path to Wine format"**
The file path could not be canonicalized. Make sure the file exists and the path is valid.

**Compiler runs but produces no output**
The Wine path translation may be incorrect. Double-check that `wine64` can be invoked directly:
```bash
/Applications/MetaTrader\ 5.app/Contents/MacOS/wine64 --version
```

**"MT5 must not already be running"**
Only one instance of MT5 can run at a time. Quit any running MT5 instance before using `rustmt5 test`.

## License

MIT
