//! File-oriented configuration commands, independent of config validity.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::Config;

/// Generate from the bundled data so the reference follows the installed binary
/// and its platform-specific wrapper names. Nothing in this file takes effect
/// until the user uncomments it.
pub fn starter() -> Result<String> {
    let defaults = toml::to_string_pretty(&Config::bundled())
        .context("cannot render bundled configuration")?;
    let mut text = String::from(
        "# acsc configuration\n\
         # All settings are commented out; bundled defaults remain in effect.\n\
         # Uncomment a table header and the settings you want to override.\n\
         # Values below show this version's bundled defaults.\n\
         # Leaving a setting commented lets it follow future bundled defaults.\n\
         # Global effort overrides all bundled harness/model effort defaults.\n\
         # effort and raw_effort are mutually exclusive within each table.\n\n",
    );
    for line in defaults.lines() {
        if line.is_empty() {
            text.push('\n');
        } else {
            text.push_str("# ");
            text.push_str(line);
            text.push('\n');
        }
        if line == "[defaults]" {
            text.push_str(
                "# No default harness or mode; these are optional examples:\n\
                 # harness = \"codex\"\n\
                 # mode = \"staged\"\n\
                 # Optional model strength (no default):\n\
                 # model_size = \"normal\"\n",
            );
        }
        if line == "[discovery.codex_cache]" {
            text.push_str(
                "# Omit path to use $CODEX_HOME/models_cache.json or ~/.codex/models_cache.json.\n\
                 # path = \"/custom/models_cache.json\"\n",
            );
        }
    }
    Ok(text)
}

/// Print bytes verbatim, including comments and invalid TOML, without creating
/// or parsing the file.
pub fn view(path: &Path, output: &mut dyn Write) -> Result<()> {
    let mut file = File::open(path).with_context(|| {
        format!(
            "cannot read config {}; use acsc --config init to create it if missing",
            path.display()
        )
    })?;
    io::copy(&mut file, output).context("cannot print config")?;
    Ok(())
}

/// create_new enforces no-overwrite even if another process creates the file
/// after parent-directory creation, or an existing path is a dangling symlink.
pub fn init(path: &Path, overwrite: bool) -> Result<()> {
    let text = starter()?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create config directory {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(!overwrite)
        .create(overwrite)
        .truncate(overwrite)
        .open(path)
        .with_context(|| {
            format!(
                "cannot initialize config {}{}",
                path.display(),
                if overwrite {
                    ""
                } else {
                    "; use --overwrite to replace an existing file"
                }
            )
        })?;
    file.write_all(text.as_bytes())
        .with_context(|| format!("cannot write config {}", path.display()))
}

fn editor() -> Result<Command> {
    let selected = ["VISUAL", "EDITOR"].into_iter().find_map(|name| {
        std::env::var_os(name)
            .filter(|value| !value.as_encoded_bytes().iter().all(u8::is_ascii_whitespace))
            .map(|value| (name, value))
    });
    let Some((name, value)) = selected else {
        return Ok(Command::new(if cfg!(windows) {
            "notepad.exe"
        } else {
            "vi"
        }));
    };
    // A literal existing path also supports unquoted executable paths with spaces
    // and Windows backslashes. Otherwise accept conventional quoted arguments.
    let words = if Path::new(&value).is_file() {
        vec![value]
    } else {
        shell_words::split(
            value
                .to_str()
                .with_context(|| format!("{name} must be valid Unicode or an executable path"))?,
        )
        .with_context(|| format!("cannot parse {name}; check its quoting"))?
        .into_iter()
        .map(OsString::from)
        .collect()
    };
    let program = words
        .first()
        .filter(|program| !program.is_empty())
        .with_context(|| format!("{name} must contain an editor executable"))?;
    let mut command = Command::new(program);
    command.args(&words[1..]);
    Ok(command)
}

pub fn edit(path: &Path) -> Result<i32> {
    let mut command = editor()?;
    if let Err(error) = init(path, false)
        && !error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::AlreadyExists)
    {
        return Err(error);
    }
    // Use an absolute final argument so a relative filename beginning with '-'
    // cannot be interpreted as an editor option. Do not invoke a shell.
    let path = std::path::absolute(path).context("cannot determine absolute config path")?;
    let status = command
        .arg(path)
        .status()
        .context("could not launch config editor; set VISUAL or EDITOR to an editor executable")?;
    Ok(status.code().unwrap_or(1))
}
