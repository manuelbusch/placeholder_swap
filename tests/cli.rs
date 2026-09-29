use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn write_config(dir: &TempDir, content: &str) -> std::path::PathBuf {
    let path = dir.path().join("config.toml");
    fs::write(&path, content).unwrap();
    path
}

fn write_input(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path
}

const CONFIG: &str = r#"
[placeholders]
HOST = "server01.example.com"
DB = "production_db"
"#;

#[test]
fn forward_from_file_to_stdout() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir, CONFIG);
    let input = write_input(&dir, "input.txt", "connect to [HOST]/[DB]");

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["forward", cfg.to_str().unwrap(), input.to_str().unwrap()])
        .assert()
        .success()
        .stdout("connect to server01.example.com/production_db");
}

#[test]
fn reverse_from_file_to_stdout() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir, CONFIG);
    let input = write_input(
        &dir,
        "input.txt",
        "connect to server01.example.com/production_db",
    );

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["reverse", cfg.to_str().unwrap(), input.to_str().unwrap()])
        .assert()
        .success()
        .stdout("connect to [HOST]/[DB]");
}

#[test]
fn forward_from_stdin() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir, CONFIG);

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["forward", cfg.to_str().unwrap()])
        .write_stdin("[HOST]:[DB]")
        .assert()
        .success()
        .stdout("server01.example.com:production_db");
}

#[test]
fn inplace_forward() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir, CONFIG);
    let input = write_input(&dir, "file.txt", "[HOST]/[DB]");

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args([
            "forward",
            cfg.to_str().unwrap(),
            input.to_str().unwrap(),
            "-i",
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&input).unwrap();
    assert_eq!(content, "server01.example.com/production_db");
}

#[test]
fn inplace_without_input_fails() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir, CONFIG);

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["forward", cfg.to_str().unwrap(), "-i"])
        .write_stdin("ignored")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--inplace requires an input file"));
}

#[test]
fn missing_config_fails() {
    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["forward", "/nonexistent/config.toml"])
        .write_stdin("hello")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Could not read config"));
}

#[test]
fn empty_config_fails() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir, "[placeholders]\n");

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["forward", cfg.to_str().unwrap()])
        .write_stdin("hello")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no entries"));
}

#[test]
fn reverse_duplicate_values_fails() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(
        &dir,
        r#"
[placeholders]
A = "same"
B = "same"
"#,
    );

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["reverse", cfg.to_str().unwrap()])
        .write_stdin("same")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Ambiguous"));
}

#[test]
fn roundtrip_via_cli() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir, CONFIG);
    let template = "[HOST] serves [DB]";
    let input = write_input(&dir, "template.txt", template);

    // Forward
    let forward = Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["forward", cfg.to_str().unwrap(), input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(forward.status.success());
    let expanded = String::from_utf8(forward.stdout).unwrap();

    // Reverse via stdin
    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["reverse", cfg.to_str().unwrap()])
        .write_stdin(expanded)
        .assert()
        .success()
        .stdout(template);
}

#[test]
fn reverse_empty_value_fails() {
    let dir = TempDir::new().unwrap();
    let cfg = write_config(
        &dir,
        r#"
[placeholders]
E = ""
"#,
    );

    Command::cargo_bin("placeholder_swap")
        .unwrap()
        .args(["reverse", cfg.to_str().unwrap()])
        .write_stdin("abc")
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty value"));
}
