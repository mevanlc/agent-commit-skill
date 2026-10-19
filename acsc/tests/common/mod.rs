use std::path::Path;
use std::process::Command;

pub fn binary(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_acsc"));
    command
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("APPDATA", root.join("config"))
        .env("CODEX_HOME", root.join("codex"))
        .env_remove("ACSC_COMPLETE");
    command
}

pub fn quoted(path: &Path) -> String {
    serde_json::to_string(path.to_str().unwrap()).unwrap()
}
