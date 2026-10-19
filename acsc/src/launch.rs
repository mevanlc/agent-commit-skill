use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::cli::Overrides;
use crate::domain::Harness;
use crate::resolve::Resolved;

#[derive(Debug, Clone, Serialize)]
pub struct LaunchPlan {
    #[serde(serialize_with = "crate::preview::os_string")]
    pub executable: OsString,
    #[serde(serialize_with = "crate::preview::args")]
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub selections: Resolved,
    pub prompt: String,
}

impl LaunchPlan {
    pub fn build(selections: Resolved, cli: &Overrides, cwd: PathBuf) -> Result<Self> {
        let mut invocation = selections
            .mode
            .value
            .argument(selections.yes.value, selections.push.value);
        if !cli.instructions.is_empty() {
            invocation.push(' ');
            invocation.push_str(&cli.instructions.join(" "));
        }
        let prompt = match selections.harness.value {
            Harness::Codex => format!("${} {invocation}", selections.skill.value),
            Harness::Claude => format!("/{} {invocation}", selections.skill.value),
            Harness::Antigravity | Harness::Copilot => format!(
                "use the '{}' skill with arguments {invocation}",
                selections.skill.value
            ),
        };
        let mut args: Vec<OsString> = selections.command.value[1..].to_vec();
        args.extend(selections.base_args.value.iter().map(OsString::from));
        let mut add =
            |values: &[&OsStr]| args.extend(values.iter().map(|value| value.to_os_string()));
        let model = selections.model.value.as_deref();
        let effort = selections.native_effort.value.as_deref();
        match selections.harness.value {
            Harness::Antigravity => {
                add(&[
                    "--add-dir".as_ref(),
                    cwd.as_os_str(),
                    "--prompt-interactive".as_ref(),
                    prompt.as_ref(),
                ]);
                if let Some(effort) = effort {
                    add(&["--effort".as_ref(), effort.as_ref()]);
                }
                if let Some(model) = model {
                    add(&["--model".as_ref(), model.as_ref()]);
                }
            }
            Harness::Copilot => {
                if let Some(model) = model {
                    add(&[format!("--model={model}").as_ref()]);
                }
                if let Some(effort) = effort {
                    add(&["--effort".as_ref(), effort.as_ref()]);
                }
                add(&["-i".as_ref(), prompt.as_ref()]);
            }
            Harness::Codex => {
                if let Some(model) = model {
                    add(&["-m".as_ref(), model.as_ref()]);
                }
                if let Some(effort) = effort {
                    add(&[
                        "-c".as_ref(),
                        format!("model_reasoning_effort={effort}").as_ref(),
                    ]);
                }
                if selections.fast.value {
                    add(&["-c".as_ref(), "service_tier=fast".as_ref()]);
                }
                add(&[prompt.as_ref()]);
            }
            Harness::Claude => {
                if let Some(model) = model {
                    add(&["--model".as_ref(), model.as_ref()]);
                }
                if let Some(effort) = effort {
                    add(&["--effort".as_ref(), effort.as_ref()]);
                }
                let settings = serde_json::to_string(&selections.native_settings)?;
                add(&["--settings".as_ref(), settings.as_ref(), prompt.as_ref()]);
            }
        }
        args.extend(cli.forwarded.clone());
        let executable = selections.command.value[0].clone();
        Ok(Self {
            executable,
            args,
            cwd,
            selections,
            prompt,
        })
    }

    pub fn process(&self) -> Result<Command> {
        #[cfg(windows)]
        let (executable, args) = windows_command(
            &self.executable,
            &self.args,
            std::env::var_os("PATH").as_deref(),
            &self.cwd,
        )?;
        #[cfg(not(windows))]
        let (executable, args) = (self.executable.clone(), self.args.clone());

        let mut command = Command::new(executable);
        command.args(args).current_dir(&self.cwd);
        Ok(command)
    }

    /// On Unix the wrapper inherits this PID, terminal, signals, and exit status.
    /// Windows waits for the wrapper and returns its status, not a commit outcome.
    pub fn execute(&self) -> Result<i32> {
        let mut command = self.process()?;
        let context = || {
            format!(
                "could not launch {:?}; check --command or the harness command in config",
                self.executable
            )
        };
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            Err(command.exec()).with_context(context)
        }
        #[cfg(not(unix))]
        {
            let status = command.status().with_context(context)?;
            Ok(status.code().unwrap_or(1))
        }
    }
}

/// Kept platform-independent so PATH lookup and PowerShell argv can be tested
/// on every host. Only .ps1 commands are handed to PowerShell.
pub fn windows_command(
    program: &OsStr,
    args: &[OsString],
    path: Option<&OsStr>,
    cwd: &Path,
) -> Result<(OsString, Vec<OsString>)> {
    if !Path::new(program)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
    {
        return Ok((program.to_owned(), args.to_vec()));
    }
    let script = Path::new(program);
    let resolved = if script.is_absolute() || script.components().count() > 1 {
        let full = cwd.join(script);
        full.is_file().then_some(full)
    } else {
        path.into_iter()
            .flat_map(std::env::split_paths)
            .map(|dir| cwd.join(dir).join(script))
            .find(|candidate| candidate.is_file())
    }
    .with_context(|| {
        format!(
            "PowerShell wrapper {program:?} not found; add its directory to PATH or set --command"
        )
    })?;
    let mut forwarded = vec![
        "-NoProfile".into(),
        "-File".into(),
        resolved.into_os_string(),
    ];
    forwarded.extend_from_slice(args);
    Ok(("pwsh".into(), forwarded))
}
