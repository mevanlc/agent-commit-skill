#[allow(
    dead_code,
    reason = "This integration target only needs common::binary"
)]
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use acsc::config::Config;
use common::binary;
use tempfile::TempDir;

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn init_creates_commented_defaults_and_requires_explicit_overwrite() {
    let temp = TempDir::new().unwrap();
    success(
        binary(temp.path())
            .args(["--config", "init"])
            .output()
            .unwrap(),
    );
    let path = temp.path().join("config/acsc/config.toml");
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.lines()
            .all(|line| line.is_empty() || line.starts_with('#'))
    );
    assert!(text.contains("# [defaults]"));
    assert!(text.contains("# [discovery.codex_cache]"));
    assert!(text.contains("# [harnesses.codex.model_sizes]"));
    assert!(text.contains("# large = \"gpt-6.1-sol\""));
    assert!(text.contains("# aliases = [\"pro\"]"));
    assert!(text.contains(if cfg!(windows) {
        "# command = [\"codexd.ps1\"]"
    } else {
        "# command = [\"codexd\"]"
    }));
    let config = Config::parse(&text).unwrap();
    assert!(config.defaults.harness.is_none());
    assert!(config.defaults.mode.is_none());
    assert!(config.defaults.effort.is_none());
    assert!(config.harnesses.is_empty());

    fs::write(&path, "existing user content").unwrap();
    let output = binary(temp.path())
        .args(["--config", "init"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--overwrite")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "existing user content");
    success(
        binary(temp.path())
            .args(["--config", "init", "--overwrite"])
            .output()
            .unwrap(),
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn nested_config_selections_are_last_wins_and_relative_paths_work() {
    let temp = TempDir::new().unwrap();
    success(
        binary(temp.path())
            .current_dir(temp.path())
            .args([
                "--config-file",
                "first",
                "--config",
                "--config-file",
                "second",
                "--no-config",
                "init",
                "--config-file",
                "nested/日本語 config.toml",
            ])
            .output()
            .unwrap(),
    );
    let path = temp.path().join("nested/日本語 config.toml");
    assert!(path.is_file());
    assert!(!temp.path().join("first").exists());
    assert!(!temp.path().join("second").exists());
    assert!(!temp.path().join("config").exists());
    let viewed = success(
        binary(temp.path())
            .current_dir(temp.path())
            .args([
                "--no-config",
                "--config",
                "--config-file",
                "nested/日本語 config.toml",
                "view",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(viewed, fs::read(path).unwrap());
}

#[test]
fn view_preserves_exact_contents_even_when_invalid_and_does_not_create_missing_files() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("raw.toml");
    let content = b"# comments\r\n[broken TOML\n\xff no final newline";
    fs::write(&path, content).unwrap();
    let output = success(
        binary(temp.path())
            .args(["--config", "view", "--config-file"])
            .arg(&path)
            .output()
            .unwrap(),
    );
    assert_eq!(output, content);
    assert_eq!(fs::read(&path).unwrap(), content);
    let output = binary(temp.path())
        .args(["--config", "view"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("config init")
    );
    assert!(!temp.path().join("config").exists());
}

#[test]
fn disabled_config_commands_report_errors_without_mutating_files() {
    let temp = TempDir::new().unwrap();
    for command in ["init", "view", "edit"] {
        let output = binary(temp.path())
            .args(["--config", command, "--no-config"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("disabled by --no-config")
        );
    }
    assert!(fs::read_dir(temp.path()).unwrap().next().is_none());
}

fn editor(root: &Path) -> PathBuf {
    let path = root.join(format!(
        "editor 日本語 with spaces{}",
        std::env::consts::EXE_SUFFIX
    ));
    let status = Command::new("rustc")
        .arg("--edition=2024")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor.rs"))
        .arg("-o")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    path
}

#[test]
fn edit_uses_editor_precedence_literal_argv_and_status_without_overwriting_existing_content() {
    let temp = TempDir::new().unwrap();
    let editor = editor(temp.path());
    let record = temp.path().join("editor.args");
    let path = temp.path().join("-config 日本語.toml");
    let mut command = binary(temp.path());
    command
        .current_dir(temp.path())
        .args(["--config", "edit", "--config-file=-config 日本語.toml"])
        .env("ACSC_TEST_RECORD", &record)
        .env(
            "ACSC_TEST_EDITOR_CONTENT",
            "[defaults]\nharness='claude'\nmode='staged'\n",
        )
        .env("ACSC_TEST_EDITOR_STATUS", "0")
        .env(
            "VISUAL",
            format!(
                "{} --wait 'a b' '$(touch injected)'",
                shell_words::quote(editor.to_str().unwrap())
            ),
        )
        .env("EDITOR", "definitely-missing-acsc-editor");
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"editor stdout\n");
    assert_eq!(output.stderr, b"editor stderr\n");
    let args = fs::read(&record).unwrap();
    let args: Vec<_> = args
        .strip_suffix(&[0])
        .unwrap()
        .split(|byte| *byte == 0)
        .map(|arg| String::from_utf8(arg.to_vec()).unwrap())
        .collect();
    assert_eq!(&args[..3], ["--wait", "a b", "$(touch injected)"]);
    assert_eq!(
        Path::new(&args[3]).canonicalize().unwrap(),
        path.canonicalize().unwrap()
    );
    let before = fs::read_to_string(record.with_extension("before")).unwrap();
    assert!(before.starts_with("# acsc configuration\n"));
    assert!(Config::parse(&before).unwrap().defaults.harness.is_none());
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("harness='claude'")
    );
    assert!(!temp.path().join("injected").exists());

    // Existing malformed TOML must reach the editor intact. Blank VISUAL falls
    // through to EDITOR, and a literal path with spaces needs no quote parsing.
    fs::write(&path, "[malformed\n").unwrap();
    command
        .env("VISUAL", " \t ")
        .env("EDITOR", &editor)
        .env_remove("ACSC_TEST_EDITOR_CONTENT")
        .env("ACSC_TEST_EDITOR_STATUS", "19");
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(19));
    assert_eq!(
        fs::read_to_string(record.with_extension("before")).unwrap(),
        "[malformed\n"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "[malformed\n");

    // Windows searches system directories before the inherited PATH, so a
    // fake Notepad there would not reliably prevent opening the real editor.
    #[cfg(unix)]
    {
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::copy(&editor, bin.join("vi")).unwrap();
        command
            .env_remove("VISUAL")
            .env_remove("EDITOR")
            .env(
                "PATH",
                std::env::join_paths(
                    std::iter::once(bin)
                        .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
                )
                .unwrap(),
            )
            .env("ACSC_TEST_EDITOR_STATUS", "0");
        success(command.output().unwrap());
    }
}

#[test]
fn invalid_editor_configuration_fails_before_creating_a_file() {
    let temp = TempDir::new().unwrap();
    let output = binary(temp.path())
        .args(["--config", "edit"])
        .env("VISUAL", "'unmatched")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot parse VISUAL")
    );
    assert!(!temp.path().join("config").exists());
}

#[test]
fn config_subcommands_are_in_help_and_dynamic_completion() {
    let temp = TempDir::new().unwrap();
    let help = success(
        binary(temp.path())
            .args(["--config", "--help"])
            .output()
            .unwrap(),
    );
    let help = String::from_utf8(help).unwrap();
    for command in ["view", "edit", "init"] {
        assert!(help.contains(command));
    }
    let completion = success(
        binary(temp.path())
            .env("ACSC_COMPLETE", "bash")
            .env("_CLAP_COMPLETE_INDEX", "2")
            .args(["--", "acsc", "--config", "i"])
            .output()
            .unwrap(),
    );
    assert!(String::from_utf8(completion).unwrap().contains("init"));
}

#[cfg(unix)]
#[test]
fn init_refuses_dangling_symlinks_without_overwrite() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    let target = temp.path().join("missing-target");
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let output = binary(temp.path())
        .args(["--config", "init", "--config-file"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(path.is_symlink());
    assert!(!target.exists());
}
