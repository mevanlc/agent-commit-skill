mod common;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use acsc::launch::windows_command;
use common::{binary, quoted};
use tempfile::TempDir;

fn recorder(root: &Path) -> PathBuf {
    let executable = root.join(format!(
        "wrapper 日本語 with spaces{}",
        std::env::consts::EXE_SUFFIX
    ));
    let status = Command::new("rustc")
        .arg("--edition=2024")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recorder.rs"))
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap();
    assert!(status.success());
    executable
}

#[test]
fn launch_preserves_argv_cwd_environment_streams_and_nonzero_status() {
    let temp = TempDir::new().unwrap();
    let executable = recorder(temp.path());
    let record = temp.path().join("record.args");
    let config = temp.path().join("config.toml");
    fs::write(&config, format!("[harnesses.codex]\ncommand=[{}, 'prefix arg']\nbase_args=[]\nmodel='harness-default'\neffort='harness-default'", quoted(&executable))).unwrap();
    let output = binary(temp.path())
        .current_dir(temp.path())
        .env("ACSC_TEST_RECORD", &record)
        .env("ACSC_TEST_ENV", "inherited 日本語")
        .env("ACSC_TEST_STATUS", "23")
        .args([
            "--config",
            config.to_str().unwrap(),
            "--harness",
            "codex",
            "-syp",
            "use 'quotes' \"日本語\"",
            "$(touch injected)",
            "--",
            "--native",
            "a b",
            "",
            "$HOME; `touch no`",
            "--harness",
            "claude",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(23));
    assert_eq!(output.stdout, b"wrapper stdout\n");
    assert_eq!(output.stderr, b"wrapper stderr\n");
    let args = fs::read(&record).unwrap();
    let args: Vec<_> = args
        .strip_suffix(&[0])
        .unwrap()
        .split(|byte| *byte == 0)
        .map(|arg| String::from_utf8(arg.to_vec()).unwrap())
        .collect();
    assert_eq!(
        args,
        [
            "prefix arg",
            "$commit --staged-yes-push use 'quotes' \"日本語\" $(touch injected)",
            "--native",
            "a b",
            "",
            "$HOME; `touch no`",
            "--harness",
            "claude"
        ]
    );
    assert_eq!(
        PathBuf::from(fs::read_to_string(record.with_extension("cwd")).unwrap())
            .canonicalize()
            .unwrap(),
        temp.path().canonicalize().unwrap()
    );
    assert_eq!(
        fs::read_to_string(record.with_extension("env")).unwrap(),
        "inherited 日本語"
    );
    assert!(!temp.path().join("injected").exists());
}

#[test]
fn dry_run_models_and_help_never_launch_the_configured_command() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");
    fs::write(&config, "[harnesses.codex]\ncommand=['definitely-missing-acsc-wrapper']\n[discovery.codex_cache]\nenabled=false").unwrap();
    for args in [
        vec!["--harness", "codex", "-a", "--dry-run"],
        vec!["models"],
        vec!["--help"],
        vec!["--version"],
    ] {
        let output = binary(temp.path())
            .args(["--config", config.to_str().unwrap()])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = binary(temp.path())
        .args([
            "--config",
            config.to_str().unwrap(),
            "--harness",
            "codex",
            "-a",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("could not launch")
    );
}

#[test]
fn default_config_paths_and_config_override_order_are_respected() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("config/acsc")).unwrap();
    fs::write(
        temp.path().join("config/acsc/config.toml"),
        "[defaults]\nharness='claude'\nmode='staged'",
    )
    .unwrap();
    let output = binary(temp.path()).arg("--dry-run").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan["prompt"], "/commit --staged");
    assert!(plan["executable"].is_string());
    assert_eq!(plan["args"][0], "--strict-mcp-config");
    assert!(plan["selections"]["command"]["value"][0].is_string());
    let output = binary(temp.path())
        .args([
            "--config",
            "missing",
            "--no-config",
            "--harness",
            "codex",
            "-a",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = binary(temp.path())
        .args([
            "--no-config",
            "--config",
            "missing",
            "--harness",
            "codex",
            "-a",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot read config")
    );
}

#[test]
fn powershell_command_resolves_path_without_interpreting_arguments() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().join("wrapper dir");
    fs::create_dir(&dir).unwrap();
    let script = dir.join("codexd.ps1");
    fs::write(&script, "unused").unwrap();
    let path = std::env::join_paths([&dir]).unwrap();
    let args: Vec<_> = ["prefix", "$commit --all ' \" ; 日本語", "--native", ""]
        .map(OsString::from)
        .into();
    let (program, forwarded) =
        windows_command("codexd.ps1".as_ref(), &args, Some(&path), temp.path()).unwrap();
    assert_eq!(program, "pwsh");
    assert_eq!(
        &forwarded[..3],
        [
            OsString::from("-NoProfile"),
            "-File".into(),
            script.clone().into_os_string()
        ]
    );
    assert_eq!(&forwarded[3..], args);
    let (_, absolute) = windows_command(script.as_os_str(), &args, None, temp.path()).unwrap();
    assert_eq!(forwarded, absolute);
    assert!(windows_command("missing.ps1".as_ref(), &args, Some(&path), temp.path()).is_err());
    assert_eq!(
        windows_command("custom.exe".as_ref(), &args, None, temp.path()).unwrap(),
        ("custom.exe".into(), args)
    );
}

#[cfg(unix)]
#[test]
fn unix_exec_preserves_signal_exit() {
    use std::os::unix::process::ExitStatusExt;
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");
    fs::write(
        &config,
        "[harnesses.codex]\ncommand=['/bin/sh', '-c', 'kill -TERM $$', 'wrapper']",
    )
    .unwrap();
    let output = binary(temp.path())
        .args([
            "--config",
            config.to_str().unwrap(),
            "--harness",
            "codex",
            "-a",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.signal(), Some(15));
}

#[cfg(unix)]
#[test]
fn unix_forwarded_arguments_can_contain_non_unicode_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let temp = TempDir::new().unwrap();
    let executable = recorder(temp.path());
    let record = temp.path().join("raw.args");
    let output = binary(temp.path())
        .env("ACSC_TEST_RECORD", &record)
        .env("ACSC_TEST_ENV", "ok")
        .env("ACSC_TEST_STATUS", "0")
        .args(["--no-config", "--harness", "codex", "-a", "--command"])
        .arg(executable)
        .arg("--")
        .arg(OsString::from_vec(vec![0x66, 0x80, 0x6f]))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(fs::read(record).unwrap().ends_with(&[0x66, 0x80, 0x6f, 0]));
}

#[cfg(windows)]
#[test]
fn windows_launches_powershell_wrapper_from_path_and_propagates_status() {
    let temp = TempDir::new().unwrap();
    let executable = recorder(temp.path());
    let script = temp.path().join("fake-wrapper.ps1");
    fs::write(
        &script,
        "& $env:ACSC_TEST_EXECUTABLE @args\nexit $LASTEXITCODE\n",
    )
    .unwrap();
    let config = temp.path().join("config.toml");
    fs::write(&config, "[harnesses.claude]\ncommand=['fake-wrapper.ps1', 'prefix with spaces']\nbase_args=[]\nmodel='harness-default'\neffort='harness-default'").unwrap();
    let path = std::env::join_paths(
        std::iter::once(temp.path().to_owned())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let record = temp.path().join("powershell.args");
    let output = binary(temp.path())
        .env("PATH", path)
        .env("ACSC_TEST_EXECUTABLE", executable)
        .env("ACSC_TEST_RECORD", &record)
        .env("ACSC_TEST_ENV", "ok")
        .env("ACSC_TEST_STATUS", "17")
        .args([
            "--config",
            config.to_str().unwrap(),
            "--harness",
            "claude",
            "-kyp",
            "say \"hello\" 日本語",
            "--",
            "",
            "a'b",
            "$HOME; hi",
            "--native",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(17),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(record).unwrap();
    let args: Vec<_> = bytes
        .strip_suffix(&[0])
        .unwrap()
        .split(|b| *b == 0)
        .map(|arg| std::str::from_utf8(arg).unwrap())
        .collect();
    assert_eq!(
        args,
        [
            "prefix with spaces",
            "--settings",
            "{\"autoMemoryEnabled\":false}",
            "/commit --ask-push say \"hello\" 日本語",
            "",
            "a'b",
            "$HOME; hi",
            "--native"
        ]
    );
}
