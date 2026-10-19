use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, builder::NonEmptyStringValueParser};

use crate::config::{ConfigChoice, parse_effort};
use crate::domain::{EffortSelection, Harness, Mode, ModelSelection};

#[derive(Debug, Clone, Default)]
pub struct Overrides {
    pub harness: Option<Harness>,
    pub mode: Option<Mode>,
    pub skill: Option<String>,
    pub model: Option<ModelSelection>,
    pub effort: Option<EffortSelection>,
    pub yes: Option<bool>,
    pub push: Option<bool>,
    pub fast: Option<bool>,
    pub command: Option<OsString>,
    pub config: ConfigChoice,
    pub dry_run: bool,
    pub instructions: Vec<String>,
    pub forwarded: Vec<OsString>,
}

fn value(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id)
        .long(id)
        .help(help)
        .value_parser(NonEmptyStringValueParser::new())
}

fn flag(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id).long(id).help(help).action(ArgAction::SetTrue)
}

pub fn command() -> Command {
    let mut cmd = Command::new("acsc")
        .version(env!("CARGO_PKG_VERSION"))
        .propagate_version(true)
        .about("Launch an agent's commit skill")
        .args_override_self(true)
        .after_help("Later options override earlier selections. Harness and mode are required for launching (flags or configuration).\nArguments after -- go unchanged to the harness.\nExample: acsc --harness codex -a -yp --staged --no-push -e high")
        .arg(value("harness", "Agent harness").value_parser(clap::value_parser!(Harness)))
        .arg(value("mode", "Commit scope").value_parser(clap::value_parser!(Mode)))
        .arg(flag("all", "Commit all changes").short('a'))
        .arg(flag("staged", "Commit staged changes").short('s'))
        .arg(flag("ask", "Ask which changes to commit").short('k'))
        .arg(flag("yes", "Preconfirm all/staged (ignored by ask)").short('y'))
        .arg(flag("no-yes", "Cancel preconfirmation"))
        .arg(flag("push", "Request push after committing").short('p'))
        .arg(flag("no-push", "Cancel the push request"))
        .arg(value("skill", "Skill name (default: commit)"))
        .arg(flag("gdf", "Select gdf-commit").short('.'))
        .arg(value("model", "Preset or native ID; harness-default omits model").short('m'))
        .arg(value("model-id", "Literal native model ID, bypassing presets"))
        .arg(value("effort", "Shared effort; harness-default omits effort").short('e').value_parser([
            "low", "medium", "high", "xhigh", "max", "ultra", "harness-default",
        ]))
        .arg(value("raw-effort", "Literal native effort, bypassing translation").value_parser(|value: &str| {
            if value.trim().is_empty() { Err("native effort must not be empty") } else { Ok(value.to_owned()) }
        }))
        .arg(flag("fast", "Request fast mode (Codex/Claude)").short('f'))
        .arg(flag("no-fast", "Cancel acsc's fast-mode request"))
        .arg(value("command", "Replace the entire command prefix with this executable")
            .value_parser(clap::value_parser!(OsString)).value_hint(ValueHint::ExecutablePath))
        .arg(value("config", "Read this TOML file").value_parser(clap::value_parser!(PathBuf)).value_hint(ValueHint::FilePath))
        .arg(flag("no-config", "Ignore user configuration"))
        .arg(flag("dry-run", "Print a JSON launch plan without starting a process"))
        .arg(Arg::new("instructions").num_args(0..).help("Additional instructions (joined with spaces)"))
        .arg(Arg::new("harness_args").last(true).num_args(0..).value_parser(clap::value_parser!(OsString)).help("Arguments forwarded unchanged"));

    for group in [
        &["mode", "all", "staged", "ask"][..],
        &["yes", "no-yes"],
        &["push", "no-push"],
        &["skill", "gdf"],
        &["model", "model-id"],
        &["effort", "raw-effort"],
        &["fast", "no-fast"],
        &["config", "no-config"],
    ] {
        for id in group {
            cmd = cmd.mut_arg(*id, |arg| {
                arg.overrides_with_all(group.iter().copied().filter(|other| other != id))
            });
        }
    }
    // Keep parent and subcommand matches separate. clap's global propagation
    // merges identical IDs, but does not resolve cross-ID overrides such as
    // `--config old models --no-config`. Overlaying the later segment preserves
    // alias ordering for all selection spellings.
    let options: Vec<_> = cmd
        .get_arguments()
        .filter(|arg| !arg.is_positional())
        .cloned()
        .collect();
    cmd.subcommand(
        Command::new("models")
            .about("List model presets and optional cached suggestions")
            .args_override_self(true)
            .args(options.clone()),
    )
    .subcommand(
        Command::new("completion")
            .about("Generate dynamic shell completion registration")
            .args_override_self(true)
            .args(options)
            .arg(Arg::new("shell").required(true).value_parser([
                "bash",
                "zsh",
                "fish",
                "powershell",
            ])),
    )
}

fn toggle(matches: &ArgMatches, positive: &str, negative: &str) -> Option<bool> {
    if matches.get_flag(positive) {
        Some(true)
    } else if matches.get_flag(negative) {
        Some(false)
    } else {
        None
    }
}

impl Overrides {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        let mut resolved = Self::from_level(matches);
        if let Some((_, child)) = matches.subcommand() {
            let later = Self::from_level(child);
            resolved.harness = later.harness.or(resolved.harness);
            resolved.mode = later.mode.or(resolved.mode);
            resolved.skill = later.skill.or(resolved.skill);
            resolved.model = later.model.or(resolved.model);
            resolved.effort = later.effort.or(resolved.effort);
            resolved.yes = later.yes.or(resolved.yes);
            resolved.push = later.push.or(resolved.push);
            resolved.fast = later.fast.or(resolved.fast);
            resolved.command = later.command.or(resolved.command);
            if later.config != ConfigChoice::Default {
                resolved.config = later.config;
            }
            resolved.dry_run |= later.dry_run;
        }
        resolved
    }

    fn from_level(matches: &ArgMatches) -> Self {
        let string = |id: &str| matches.get_one::<String>(id).cloned();
        Self {
            harness: matches.get_one::<Harness>("harness").copied(),
            mode: matches.get_one::<Mode>("mode").copied().or_else(|| {
                [
                    ("all", Mode::All),
                    ("staged", Mode::Staged),
                    ("ask", Mode::Ask),
                ]
                .into_iter()
                .find(|(id, _)| matches.get_flag(id))
                .map(|(_, mode)| mode)
            }),
            skill: string("skill").or_else(|| matches.get_flag("gdf").then(|| "gdf-commit".into())),
            model: string("model")
                .map(ModelSelection::Named)
                .or_else(|| string("model-id").map(ModelSelection::Literal)),
            effort: parse_effort(string("effort").as_deref(), string("raw-effort").as_deref())
                .expect("clap validates effort values"),
            yes: toggle(matches, "yes", "no-yes"),
            push: toggle(matches, "push", "no-push"),
            fast: toggle(matches, "fast", "no-fast"),
            command: matches.get_one::<OsString>("command").cloned(),
            config: matches
                .get_one::<PathBuf>("config")
                .cloned()
                .map(ConfigChoice::File)
                .unwrap_or_else(|| {
                    if matches.get_flag("no-config") {
                        ConfigChoice::Disabled
                    } else {
                        ConfigChoice::Default
                    }
                }),
            dry_run: matches.get_flag("dry-run"),
            instructions: matches
                .try_get_many::<String>("instructions")
                .ok()
                .flatten()
                .map(|args| args.cloned().collect())
                .unwrap_or_default(),
            forwarded: matches
                .try_get_many::<OsString>("harness_args")
                .ok()
                .flatten()
                .map(|args| args.cloned().collect())
                .unwrap_or_default(),
        }
    }
}
