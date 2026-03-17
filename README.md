# placeholder_swap

A Rust-based CLI tool for bidirectional placeholder replacement in text files. Based on a TOML configuration file, placeholders like `[KEY]` can be replaced with their values — and vice versa.

## Features

- **Forward mode**: Replaces placeholders (`[KEY]`) with their configured values
- **Reverse mode**: Replaces values back into placeholder format (`[KEY]`)
- **Flexible I/O**: Reads from files or STDIN, writes to STDOUT
- **In-place editing**: Files can be modified directly with `-i`
- **Robust pattern matching**: Uses the Aho-Corasick algorithm for efficient matching of overlapping patterns
- **Safety checks**: Detects ambiguous configurations (duplicate values in reverse mode)

## Installation

```bash
cargo install --path .
```

This installs the binary to `~/.cargo/bin`, making it directly available in your `PATH`.

## Usage

### Configuration file

Create a TOML file with placeholder definitions:

```toml
[placeholders]
HOSTNAME = "server01.example.com"
DB_NAME = "production_db"
API_URL = "https://api.example.com/v2"
```

### Command-line options

```
placeholder_swap <forward|reverse> <config> [input_file] [-i]
```

| Argument      | Description                                | Required |
|---------------|--------------------------------------------|----------|
| `forward`     | Replace `[KEY]` placeholders with values   | Yes*     |
| `reverse`     | Replace values back to `[KEY]` placeholders| Yes*     |
| `<config>`    | Path to the TOML configuration file        | Yes      |
| `[input_file]`| Input file (default: STDIN)                | No       |
| `-i, --inplace`| Overwrite the input file directly          | No       |

*One of `forward` or `reverse` must be chosen.

### Examples

**Replace placeholders with values:**

```bash
placeholder_swap forward config.toml template.txt > result.txt
```

Turns `The server [HOSTNAME] uses [DB_NAME]` into `The server server01.example.com uses production_db`.

**Replace values back to placeholders:**

```bash
placeholder_swap reverse config.toml result.txt > template.txt
```

**In-place replacement:**

```bash
placeholder_swap forward config.toml file.txt -i
```

**With STDIN:**

```bash
cat template.txt | placeholder_swap forward config.toml > output.txt
```

## Dependencies

- [clap](https://crates.io/crates/clap) — Argument parsing
- [serde](https://crates.io/crates/serde) + [toml](https://crates.io/crates/toml) — TOML configuration
- [aho-corasick](https://crates.io/crates/aho-corasick) — Efficient multi-pattern matching
- [anyhow](https://crates.io/crates/anyhow) — Error handling
