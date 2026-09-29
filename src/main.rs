use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

#[derive(Debug, Deserialize)]
struct Config {
    placeholders: BTreeMap<String, String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Replace [Key] placeholders with their values
    Forward(CommonArgs),
    /// Replace values back to [Key] placeholders
    Reverse(CommonArgs),
}

#[derive(Parser, Debug)]
struct CommonArgs {
    /// Path to the TOML configuration file
    config: PathBuf,

    /// Input file (reads from STDIN if omitted)
    input: Option<PathBuf>,

    /// Overwrite the input file in-place
    #[arg(short, long)]
    inplace: bool,
}

#[derive(Parser, Debug)]
#[command(name = "placeholder_swap")]
#[command(about = "Replaces [placeholders] <-> values based on a TOML config.")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let common = match &args.command {
        Command::Forward(a) | Command::Reverse(a) => a,
    };

    if common.inplace && common.input.is_none() {
        bail!("--inplace requires an input file.");
    }

    // Read config
    let cfg_str = fs::read_to_string(&common.config)
        .with_context(|| format!("Could not read config: {}", common.config.display()))?;
    let cfg: Config = toml::from_str(&cfg_str).context("Could not parse TOML")?;

    if cfg.placeholders.is_empty() {
        bail!("Config contains no entries under [placeholders].");
    }

    // Read input: file or STDIN
    let input_text = if let Some(path) = &common.input {
        fs::read_to_string(path)
            .with_context(|| format!("Could not read input file: {}", path.display()))?
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("Could not read STDIN")?;
        buf
    };

    // Transform
    let output_text = match &args.command {
        Command::Forward(_) => replace_placeholders_with_values(&input_text, &cfg),
        Command::Reverse(_) => replace_values_with_placeholders(&input_text, &cfg)?,
    };

    // Write output: inplace or STDOUT
    if common.inplace {
        let in_path = common.input.as_ref().expect("checked above");
        fs::write(in_path, output_text)
            .with_context(|| format!("Could not overwrite input file: {}", in_path.display()))?;
    } else {
        let mut out = io::stdout().lock();
        out.write_all(output_text.as_bytes())
            .context("Could not write to STDOUT")?;
    }

    Ok(())
}

/// Forward: Replaces all occurrences of `[Key]` with the corresponding value.
/// Sorts by placeholder length descending to handle overlapping keys safely.
fn replace_placeholders_with_values(input: &str, cfg: &Config) -> String {
    let mut pairs: Vec<(String, String)> = cfg
        .placeholders
        .iter()
        .map(|(k, v)| (format!("[{k}]"), v.clone()))
        .collect();

    pairs.sort_by_key(|(ph, _)| std::cmp::Reverse(ph.len()));

    let mut out = input.to_owned();
    for (ph, val) in pairs {
        out = out.replace(&ph, &val);
    }
    out
}

/// Reverse: Replaces all values with their placeholders `[Key]`.
///
/// Uses Aho-Corasick "leftmost-longest" for robust handling of overlapping values.
fn replace_values_with_placeholders(input: &str, cfg: &Config) -> Result<String> {
    let mut seen_values: BTreeMap<&str, &str> = BTreeMap::new();
    for (k, v) in &cfg.placeholders {
        if let Some(existing_key) = seen_values.insert(v.as_str(), k.as_str()) {
            bail!(
                "Ambiguous reverse replacement: value {v:?} is defined for both key {existing_key:?} and {k:?}."
            );
        }
    }

    let patterns: Vec<&str> = cfg.placeholders.values().map(String::as_str).collect();

    let replacements: Vec<String> = cfg.placeholders.keys().map(|k| format!("[{k}]")).collect();

    let ac: AhoCorasick = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(patterns)
        .context("Aho-Corasick build failed")?;

    let replacement_refs: Vec<&str> = replacements.iter().map(String::as_str).collect();

    Ok(ac.replace_all(input, &replacement_refs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(entries: &[(&str, &str)]) -> Config {
        Config {
            placeholders: entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    // --- Forward tests ---

    #[test]
    fn forward_simple() {
        let cfg = make_config(&[("HOST", "example.com"), ("PORT", "8080")]);
        let result = replace_placeholders_with_values("connect to [HOST]:[PORT]", &cfg);
        assert_eq!(result, "connect to example.com:8080");
    }

    #[test]
    fn forward_no_placeholders_in_input() {
        let cfg = make_config(&[("HOST", "example.com")]);
        let result = replace_placeholders_with_values("no placeholders here", &cfg);
        assert_eq!(result, "no placeholders here");
    }

    #[test]
    fn forward_multiple_occurrences() {
        let cfg = make_config(&[("X", "hello")]);
        let result = replace_placeholders_with_values("[X] and [X]", &cfg);
        assert_eq!(result, "hello and hello");
    }

    #[test]
    fn forward_overlapping_keys() {
        let cfg = make_config(&[("A", "short"), ("AB", "long")]);
        let result = replace_placeholders_with_values("[AB] [A]", &cfg);
        assert_eq!(result, "long short");
    }

    #[test]
    fn forward_empty_input() {
        let cfg = make_config(&[("X", "val")]);
        let result = replace_placeholders_with_values("", &cfg);
        assert_eq!(result, "");
    }

    #[test]
    fn forward_preserves_unrelated_brackets() {
        let cfg = make_config(&[("HOST", "example.com")]);
        let result = replace_placeholders_with_values("[HOST] and [OTHER]", &cfg);
        assert_eq!(result, "example.com and [OTHER]");
    }

    // --- Reverse tests ---

    #[test]
    fn reverse_simple() {
        let cfg = make_config(&[("HOST", "example.com"), ("PORT", "8080")]);
        let result = replace_values_with_placeholders("connect to example.com:8080", &cfg).unwrap();
        assert_eq!(result, "connect to [HOST]:[PORT]");
    }

    #[test]
    fn reverse_no_values_in_input() {
        let cfg = make_config(&[("HOST", "example.com")]);
        let result = replace_values_with_placeholders("nothing to replace", &cfg).unwrap();
        assert_eq!(result, "nothing to replace");
    }

    #[test]
    fn reverse_multiple_occurrences() {
        let cfg = make_config(&[("X", "hello")]);
        let result = replace_values_with_placeholders("hello and hello", &cfg).unwrap();
        assert_eq!(result, "[X] and [X]");
    }

    #[test]
    fn reverse_overlapping_values() {
        let cfg = make_config(&[("SHORT", "ab"), ("LONG", "abc")]);
        let result = replace_values_with_placeholders("abc ab", &cfg).unwrap();
        assert_eq!(result, "[LONG] [SHORT]");
    }

    #[test]
    fn reverse_duplicate_values_error() {
        let cfg = make_config(&[("A", "same"), ("B", "same")]);
        let result = replace_values_with_placeholders("same", &cfg);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Ambiguous"));
    }

    #[test]
    fn reverse_empty_input() {
        let cfg = make_config(&[("X", "val")]);
        let result = replace_values_with_placeholders("", &cfg).unwrap();
        assert_eq!(result, "");
    }

    // --- Roundtrip tests ---

    #[test]
    fn roundtrip_forward_then_reverse() {
        let cfg = make_config(&[
            ("HOST", "server01.example.com"),
            ("DB", "production_db"),
            ("PORT", "5432"),
        ]);
        let template = "connect to [HOST]:[PORT]/[DB]";
        let expanded = replace_placeholders_with_values(template, &cfg);
        let restored = replace_values_with_placeholders(&expanded, &cfg).unwrap();
        assert_eq!(restored, template);
    }

    #[test]
    fn roundtrip_reverse_then_forward() {
        let cfg = make_config(&[("HOST", "server01.example.com"), ("DB", "production_db")]);
        let concrete = "server01.example.com/production_db";
        let templatized = replace_values_with_placeholders(concrete, &cfg).unwrap();
        let expanded = replace_placeholders_with_values(&templatized, &cfg);
        assert_eq!(expanded, concrete);
    }
}
