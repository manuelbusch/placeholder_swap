use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
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

#[derive(Clone, Debug, ValueEnum)]
enum Mode {
    /// [Key] -> Value
    Forward,
    /// Value -> [Key]
    Reverse,
}

#[derive(Parser, Debug)]
#[command(name = "placeholder_swap")]
#[command(about = "Ersetzt [Platzhalter] <-> Werte basierend auf einer TOML-Config.")]
struct Args {
    /// Pfad zur TOML-Konfiguration
    #[arg(short, long)]
    config: PathBuf,

    /// Input-Datei (wenn nicht gesetzt: STDIN)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output-Datei (wenn nicht gesetzt: STDOUT)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Überschreibt die Input-Datei (nur möglich mit --input; nicht kombinierbar mit --output)
    #[arg(long)]
    inplace: bool,

    /// Modus: forward ([Key]->Value) oder reverse (Value->[Key])
    #[arg(short, long, value_enum)]
    mode: Mode,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.inplace && args.output.is_some() {
        bail!("--inplace kann nicht zusammen mit --output verwendet werden.");
    }
    if args.inplace && args.input.is_none() {
        bail!("--inplace erfordert --input (Datei muss bekannt sein, um sie zu überschreiben).");
    }

    // Config lesen
    let cfg_str = fs::read_to_string(&args.config)
        .with_context(|| format!("Konnte Config nicht lesen: {:?}", args.config))?;
    let cfg: Config = toml::from_str(&cfg_str).context("TOML konnte nicht geparst werden")?;

    if cfg.placeholders.is_empty() {
        bail!("Config enthält keine Einträge unter [placeholders].");
    }

    // Input lesen: Datei oder STDIN
    let input_text = match &args.input {
        Some(path) => fs::read_to_string(path)
            .with_context(|| format!("Konnte Input-Datei nicht lesen: {:?}", path))?,
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .context("Konnte STDIN nicht lesen")?;
            buf
        }
    };

    // Transformieren
    let output_text = match args.mode {
        Mode::Forward => replace_placeholders_with_values(&input_text, &cfg),
        Mode::Reverse => replace_values_with_placeholders(&input_text, &cfg)?,
    };

    // Output schreiben: inplace, Datei oder STDOUT
    if args.inplace {
        let in_path = args.input.as_ref().expect("checked above");
        fs::write(in_path, output_text)
            .with_context(|| format!("Konnte Input-Datei nicht überschreiben: {:?}", in_path))?;
        return Ok(());
    }

    match &args.output {
        Some(path) => {
            fs::write(path, output_text)
                .with_context(|| format!("Konnte Output-Datei nicht schreiben: {:?}", path))?;
        }
        None => {
            let mut out = io::stdout().lock();
            out.write_all(output_text.as_bytes())
                .context("Konnte nicht auf STDOUT schreiben")?;
        }
    }

    Ok(())
}

/// Forward: Ersetzt alle Vorkommen von `[Key]` durch den dazugehörigen Wert.
/// Zur Sicherheit sortieren wir nach Placeholder-Länge absteigend, falls Keys
/// sich als Teilstrings überlappen (selten, aber möglich).
fn replace_placeholders_with_values(input: &str, cfg: &Config) -> String {
    let mut pairs: Vec<(String, String)> = cfg
        .placeholders
        .iter()
        .map(|(k, v)| (format!("[{}]", k), v.clone()))
        .collect();

    pairs.sort_by_key(|(ph, _)| std::cmp::Reverse(ph.len()));

    let mut out = input.to_owned();
    for (ph, val) in pairs {
        out = out.replace(&ph, &val);
    }
    out
}

/// Reverse: Ersetzt alle Werte durch ihre Platzhalter `[Key]`.
///
/// Robust gegen Overlaps/Teilstrings mit Aho-Corasick "leftmost-longest":
/// - nimmt an der frühesten Position stets den längsten Treffer
/// - verhindert dadurch, dass kürzere Werte, die in längeren vorkommen, zuerst ersetzt werden
fn replace_values_with_placeholders(input: &str, cfg: &Config) -> Result<String> {
    // Mehrdeutigkeit verhindern: identische Werte für verschiedene Keys wären nicht eindeutig.
    let mut seen_values: BTreeMap<&str, &str> = BTreeMap::new();
    for (k, v) in &cfg.placeholders {
        if let Some(existing_key) = seen_values.insert(v.as_str(), k.as_str()) {
            bail!(
                "Mehrdeutige Reverse-Ersetzung: Der Wert {:?} ist sowohl für Key {:?} als auch {:?} definiert.",
                v,
                existing_key,
                k
            );
        }
    }

    // Patterns = Werte
    let patterns: Vec<&str> = cfg.placeholders.values().map(|s| s.as_str()).collect();

    // Replacements müssen in exakt derselben Reihenfolge sein wie patterns.
    // Da wir oben values() verwenden, nehmen wir hier keys() auf derselben Map in derselben Iterationsreihenfolge.
    let replacements: Vec<String> = cfg
        .placeholders
        .keys()
        .map(|k| format!("[{}]", k))
        .collect();

    let ac: AhoCorasick = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(patterns)
        .context("Aho-Corasick Build fehlgeschlagen")?;

    let replacement_refs: Vec<&str> = replacements.iter().map(|s| s.as_str()).collect();

    Ok(ac.replace_all(input, &replacement_refs))
}
