use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::io::Write;

use anyhow::{Context, Result};
use clap::Command;
use clap_complete::{ArgValueCompleter, CompleteEnv, CompletionCandidate};

use crate::catalog::Catalog;
use crate::cli::{self, Overrides};
use crate::config::Config;
use crate::domain::Mode;
use crate::resolve::Resolver;

pub const ENV: &str = "ACSC_COMPLETE";

/// Parse only completed words. Partial or invalid input and optional files must
/// never print diagnostics while the shell is completing.
pub fn command_for(words: &[OsString]) -> Command {
    // Everything after the delimiter belongs to the harness. The completion
    // engine otherwise still recognizes subcommand names in this tail.
    if words.iter().any(|word| word == "--") {
        return Command::new("acsc")
            .disable_help_flag(true)
            .disable_help_subcommand(true);
    }
    let parsed = cli::command()
        .ignore_errors(true)
        .try_get_matches_from(words);
    let mut overrides = parsed
        .as_ref()
        .map(Overrides::from_matches)
        .unwrap_or_default();
    let resolver = Config::load(&overrides.config)
        .and_then(Resolver::new)
        .unwrap_or_else(|_| Resolver::new(Config::default()).expect("bundled defaults"));
    let catalog = Catalog::load(&resolver).unwrap_or_default();
    let harness = resolver
        .selected_harness(&overrides)
        .map(|value| value.value);
    let mut named = BTreeSet::from(["harness-default".to_owned()]);
    let mut ids = BTreeSet::new();
    for model in catalog.for_harness(harness) {
        named.insert(model.id.clone());
        named.extend(model.aliases.clone());
        ids.insert(model.id.clone());
    }
    overrides.mode.get_or_insert(Mode::All);
    let native_efforts: BTreeSet<_> = resolver
        .resolve(&overrides)
        .ok()
        .and_then(|resolved| {
            let model = resolved.model.value?;
            catalog
                .find(resolved.harness.value, &model)
                .map(|model| model.native_efforts.clone())
        })
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut command = cli::command()
        .mut_subcommand("--models", |command| {
            with_values(command, &named, &ids, &native_efforts)
        })
        .mut_subcommand("--completion", |command| {
            with_values(command, &named, &ids, &native_efforts)
        });
    // clap_complete stops offering subcommands after entering variadic
    // instructions, although flag subcommands still parse there. Offer those
    // same candidates through the positional completer; matching IDs deduplicate
    // them when the engine also offers subcommands.
    let commands: Vec<_> = command
        .get_subcommands()
        .filter(|command| command.get_name().starts_with("--"))
        .map(|command| (command.get_name().to_owned(), command.get_about().cloned()))
        .collect();
    command = command.mut_arg("instructions", |arg| {
        arg.add(ArgValueCompleter::new(move |current: &OsStr| {
            let Some(prefix) = current.to_str() else {
                return Vec::new();
            };
            commands
                .iter()
                .filter(|(name, _)| name.starts_with(prefix))
                .map(|(name, help)| {
                    CompletionCandidate::new(name.clone())
                        .id(Some(format!("command::{name}")))
                        .tag(Some("Commands".into()))
                        .help(help.clone())
                })
                .collect::<Vec<_>>()
        }))
    });
    with_values(command, &named, &ids, &native_efforts)
}

fn with_values(
    command: Command,
    named: &BTreeSet<String>,
    ids: &BTreeSet<String>,
    native_efforts: &BTreeSet<String>,
) -> Command {
    command
        .mut_arg("model", |arg| arg.add(completer(named.clone())))
        .mut_arg("model-id", |arg| arg.add(completer(ids.clone())))
        .mut_arg("raw-effort", |arg| {
            arg.add(completer(native_efforts.clone()))
        })
}

fn completer(values: BTreeSet<String>) -> ArgValueCompleter {
    ArgValueCompleter::new(move |current: &OsStr| {
        let Some(prefix) = current.to_str() else {
            return Vec::new();
        };
        values
            .iter()
            .filter(|value| value.starts_with(prefix))
            .map(|value| CompletionCandidate::new(value.clone()))
            .collect()
    })
}

pub fn enabled() -> bool {
    std::env::var_os(ENV).is_some_and(|value| !value.is_empty() && value != "0")
}

/// Called at startup only for an environment-activated completion request.
pub fn complete() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let words = args
        .iter()
        .position(|arg| arg == "--")
        .map(|index| &args[index + 1..])
        .unwrap_or(&[]);
    let cursor = std::env::var("_CLAP_COMPLETE_INDEX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| words.len().saturating_sub(1))
        .min(words.len());
    let command = command_for(&words[..cursor]);
    CompleteEnv::with_factory(|| command.clone())
        .var(ENV)
        .try_complete(args, std::env::current_dir().ok().as_deref())?;
    Ok(())
}

pub fn registration(shell: &str, writer: &mut dyn Write) -> Result<()> {
    let executable =
        std::env::current_exe().context("cannot locate acsc for completion registration")?;
    let executable = executable
        .to_str()
        .context("completion registration requires a Unicode executable path")?;
    if shell == "powershell" {
        // Use parsed argument values instead of re-evaluating the user's command
        // line as PowerShell source. Single quotes escape by doubling in pwsh.
        let executable = executable.replace('\'', "''");
        writeln!(
            writer,
            r#"Register-ArgumentCompleter -Native -CommandName acsc -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)
    $words = @($commandAst.CommandElements | Where-Object {{ $_.Extent.StartOffset -lt $cursorPosition }} | ForEach-Object {{
        if ($_ -is [System.Management.Automation.Language.StringConstantExpressionAst]) {{ $_.Value }}
        else {{ $_.Extent.Text }}
    }})
    if ($wordToComplete -eq '') {{ $words += '' }}
    $previous = $env:ACSC_COMPLETE
    try {{
        $env:ACSC_COMPLETE = 'powershell'
        & '{executable}' -- @words | ForEach-Object {{
            $parts = $_ -split "`t", 2
            $value = $parts[0]
            $quoted = "'" + $value.Replace("'", "''") + "'"
            [System.Management.Automation.CompletionResult]::new($quoted, $value, 'ParameterValue', $value)
        }}
    }} finally {{
        if ($null -eq $previous) {{ Remove-Item Env:\ACSC_COMPLETE -ErrorAction SilentlyContinue }}
        else {{ $env:ACSC_COMPLETE = $previous }}
    }}
}}"#
        )?;
    } else {
        let shells = clap_complete::env::Shells::builtins();
        let completer = shells
            .completer(shell)
            .context("unsupported completion shell")?;
        completer.write_registration(ENV, "acsc", "acsc", executable, writer)?;
    }
    Ok(())
}
