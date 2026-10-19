use std::collections::BTreeMap;
use std::ffi::OsString;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::cli::Overrides;
use crate::config::{Config, HarnessConfig, ModelConfig, model_aliases, nonempty, parse_effort};
use crate::domain::{Effort, EffortSelection, Harness, Mode, ModelSelection, Selection, Source};

/// Fully resolved choices, independent of terminal rendering or process execution.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    pub harness: Selection<Harness>,
    pub mode: Selection<Mode>,
    pub skill: Selection<String>,
    pub model: Selection<Option<String>>,
    pub effort: Selection<EffortSelection>,
    pub native_effort: Selection<Option<String>>,
    pub yes: Selection<bool>,
    pub effective_yes: bool,
    pub push: Selection<bool>,
    pub fast: Selection<bool>,
    #[serde(serialize_with = "crate::preview::command")]
    pub command: Selection<Vec<OsString>>,
    pub base_args: Selection<Vec<String>>,
    pub native_settings: BTreeMap<String, serde_json::Value>,
    pub native_setting_sources: BTreeMap<String, Source>,
}

fn choose<T>(values: impl IntoIterator<Item = (Option<T>, Source)>) -> Option<Selection<T>> {
    values
        .into_iter()
        .find_map(|(value, source)| value.map(|value| Selection::new(value, source)))
}

pub struct Resolver {
    pub bundled: Config,
    pub user: Config,
}

impl Resolver {
    pub fn new(user: Config) -> Result<Self> {
        let resolver = Self {
            bundled: Config::bundled(),
            user,
        };
        for harness in Harness::ALL {
            let (bundled, user) = resolver.harness_configs(harness);
            model_aliases(&bundled, &user).with_context(|| format!("{harness} presets"))?;
        }
        Ok(resolver)
    }

    pub fn harness_configs(&self, harness: Harness) -> (HarnessConfig, HarnessConfig) {
        (
            self.bundled
                .harnesses
                .get(&harness)
                .cloned()
                .unwrap_or_default(),
            self.user
                .harnesses
                .get(&harness)
                .cloned()
                .unwrap_or_default(),
        )
    }

    pub fn selected_harness(&self, cli: &Overrides) -> Option<Selection<Harness>> {
        choose([
            (cli.harness, Source::Cli),
            (self.user.defaults.harness, Source::UserGlobal),
        ])
    }

    pub fn resolve(&self, cli: &Overrides) -> Result<Resolved> {
        let harness = self.selected_harness(cli)
            .context("select a harness with --harness antigravity|copilot|codex|claude or defaults.harness in config")?;
        let (bundled, user) = self.harness_configs(harness.value);
        let global = &self.user.defaults;
        let baseline = &self.bundled.defaults;
        let mode = choose([
            (cli.mode, Source::Cli),
            (user.mode, Source::UserHarness),
            (global.mode, Source::UserGlobal),
        ])
        .context("select a commit mode with --all, --staged, --ask, --mode, or mode in config")?;
        let skill = choose([
            (cli.skill.clone(), Source::Cli),
            (user.skill.clone(), Source::UserHarness),
            (global.skill.clone(), Source::UserGlobal),
            (baseline.skill.clone(), Source::BundledGlobal),
        ])
        .expect("bundled skill");
        nonempty(&skill.value, "skill")?;
        let model_choice = choose([
            (cli.model.clone(), Source::Cli),
            (
                user.model.clone().map(ModelSelection::Named),
                Source::UserHarness,
            ),
            (
                bundled.model.clone().map(ModelSelection::Named),
                Source::BundledHarness,
            ),
        ])
        .expect("bundled model selection");
        let aliases = model_aliases(&bundled, &user)?;
        let model_id = match model_choice.value {
            ModelSelection::Named(name) if name == "harness-default" => None,
            ModelSelection::Named(name) => Some(aliases.get(&name).cloned().unwrap_or(name)),
            ModelSelection::Literal(id) => Some(id),
        };
        if let Some(id) = &model_id {
            nonempty(id, "model")?;
        }
        let model = Selection::new(model_id, model_choice.source);
        let user_model = model
            .value
            .as_ref()
            .and_then(|id| user.models.get(id))
            .cloned()
            .unwrap_or_default();
        let bundled_model = model
            .value
            .as_ref()
            .and_then(|id| bundled.models.get(id))
            .cloned()
            .unwrap_or_default();
        let effort = choose([
            (cli.effort.clone(), Source::Cli),
            (model_effort(&user_model)?, Source::UserModel),
            (
                parse_effort(user.effort.as_deref(), user.raw_effort.as_deref())?,
                Source::UserHarness,
            ),
            (
                parse_effort(global.effort.as_deref(), global.raw_effort.as_deref())?,
                Source::UserGlobal,
            ),
            (model_effort(&bundled_model)?, Source::BundledModel),
            (
                parse_effort(bundled.effort.as_deref(), bundled.raw_effort.as_deref())?,
                Source::BundledHarness,
            ),
            (
                parse_effort(baseline.effort.as_deref(), baseline.raw_effort.as_deref())?,
                Source::BundledGlobal,
            ),
        ])
        .expect("bundled effort");
        let native_effort = match &effort.value {
            EffortSelection::HarnessDefault => Selection::new(None, effort.source),
            EffortSelection::Raw(value) => Selection::new(Some(value.clone()), effort.source),
            EffortSelection::Shared(level) => {
                let mapped = self.map_effort(*level, &bundled, &user, &bundled_model, &user_model);
                Selection::new(Some(mapped.value), mapped.source)
            }
        };
        let boolean = |cli, user, global, default| {
            choose([
                (cli, Source::Cli),
                (user, Source::UserHarness),
                (global, Source::UserGlobal),
                (default, Source::BundledGlobal),
            ])
            .expect("bundled boolean")
        };
        let yes = boolean(cli.yes, user.yes, global.yes, baseline.yes);
        let push = boolean(cli.push, user.push, global.push, baseline.push);
        let fast = boolean(cli.fast, user.fast, global.fast, baseline.fast);
        if fast.value && !matches!(harness.value, Harness::Codex | Harness::Claude) {
            bail!(
                "fast mode is unsupported for {}; use --no-fast to cancel the request",
                harness.value
            );
        }
        let command = choose([
            (
                cli.command.clone().map(|program| vec![program]),
                Source::Cli,
            ),
            (
                user.command
                    .clone()
                    .map(|args| args.into_iter().map(OsString::from).collect()),
                Source::UserHarness,
            ),
            (
                bundled
                    .command
                    .clone()
                    .map(|args| args.into_iter().map(OsString::from).collect()),
                Source::BundledHarness,
            ),
        ])
        .expect("bundled command");
        let program = command
            .value
            .first()
            .context("command must contain an executable")?;
        if program.is_empty() {
            bail!("command executable must not be empty");
        }
        let base_args = choose([
            (user.base_args.clone(), Source::UserHarness),
            (bundled.base_args.clone(), Source::BundledHarness),
        ])
        .expect("bundled base_args");
        let mut native_settings = bundled.native_settings.clone();
        let mut native_setting_sources = native_settings
            .keys()
            .cloned()
            .map(|key| (key, Source::BundledHarness))
            .collect::<BTreeMap<_, _>>();
        for (key, value) in &user.native_settings {
            native_settings.insert(key.clone(), value.clone());
            native_setting_sources.insert(key.clone(), Source::UserHarness);
        }
        if harness.value == Harness::Claude && fast.value {
            native_settings.insert("fastMode".into(), true.into());
            native_setting_sources.insert("fastMode".into(), fast.source);
        }
        Ok(Resolved {
            effective_yes: yes.value && mode.value != Mode::Ask,
            harness,
            mode,
            skill,
            model,
            effort,
            native_effort,
            yes,
            push,
            fast,
            command,
            base_args,
            native_settings,
            native_setting_sources,
        })
    }

    fn map_effort(
        &self,
        level: Effort,
        bundled: &HarnessConfig,
        user: &HarnessConfig,
        bundled_model: &ModelConfig,
        user_model: &ModelConfig,
    ) -> Selection<String> {
        choose([
            (
                user_model.effort_map.get(&level).cloned(),
                Source::UserModel,
            ),
            (user.effort_map.get(&level).cloned(), Source::UserHarness),
            (
                bundled_model.effort_map.get(&level).cloned(),
                Source::BundledModel,
            ),
            (
                bundled.effort_map.get(&level).cloned(),
                Source::BundledHarness,
            ),
            (Some(level.name().into()), Source::Identity),
        ])
        .expect("identity mapping")
    }
}

fn model_effort(model: &ModelConfig) -> Result<Option<EffortSelection>> {
    parse_effort(model.effort.as_deref(), model.raw_effort.as_deref())
}
