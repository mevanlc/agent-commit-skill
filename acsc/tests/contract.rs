use std::ffi::OsString;
use std::path::PathBuf;

use acsc::catalog::Catalog;
use acsc::cli::{self, Overrides};
use acsc::config::{Config, ConfigChoice};
use acsc::domain::{EffortSelection, Harness, Mode, Source};
use acsc::launch::LaunchPlan;
use acsc::resolve::Resolver;

fn cli(args: &[&str]) -> Overrides {
    let matches = cli::command()
        .try_get_matches_from(std::iter::once("acsc").chain(args.iter().copied()))
        .unwrap();
    Overrides::from_matches(&matches)
}

fn plan(config: &str, args: &[&str]) -> LaunchPlan {
    let cli = cli(args);
    let resolver = Resolver::new(Config::parse(config).unwrap()).unwrap();
    LaunchPlan::build(
        resolver.resolve(&cli).unwrap(),
        &cli,
        PathBuf::from("/a repo/日本語"),
    )
    .unwrap()
}

#[test]
fn clap_definition_is_consistent() {
    cli::command().debug_assert();
}

#[test]
fn alias_options_and_repeated_shorthand_are_last_wins() {
    let resolved = plan(
        "",
        &[
            "--harness",
            "claude",
            "--harness",
            "codex",
            "-a",
            "--mode",
            "ask",
            "-s",
            "--all",
            "--staged",
            "-y",
            "--no-yes",
            "-yy",
            "-p",
            "--no-push",
            "-pp",
            "--no-push",
            "--skill",
            "first",
            "-.",
            "--skill",
            "final",
            "-.",
            "-m",
            "spark",
            "--model-id",
            "literal",
            "--model",
            "spark",
            "-e",
            "low",
            "--raw-effort",
            "native",
            "--effort",
            "high",
            "--fast",
            "--no-fast",
            "-f",
            "--no-fast",
            "--command",
            "one",
            "--command",
            "two",
        ],
    )
    .selections;
    assert_eq!(resolved.harness.value, Harness::Codex);
    assert_eq!(resolved.mode.value, Mode::Staged);
    assert!(resolved.yes.value);
    assert!(!resolved.push.value);
    assert!(!resolved.fast.value);
    assert_eq!(resolved.skill.value, "gdf-commit");
    assert_eq!(resolved.model.value.as_deref(), Some("gpt-5.3-codex-spark"));
    assert_eq!(resolved.native_effort.value.as_deref(), Some("high"));
    assert_eq!(resolved.command.value, [OsString::from("two")]);
}

#[test]
fn all_override_directions_and_attached_short_values_work() {
    let resolved = plan(
        "",
        &[
            "--harness=codex",
            "-s",
            "-k",
            "--mode=all",
            "-.",
            "--skill=custom",
            "--no-yes",
            "-y",
            "--no-push",
            "-p",
            "--no-fast",
            "-f",
            "--model=spark",
            "--model-id=spark",
            "--effort=high",
            "--raw-effort=minimal",
        ],
    )
    .selections;
    assert_eq!(resolved.mode.value, Mode::All);
    assert_eq!(resolved.skill.value, "custom");
    assert_eq!(resolved.model.value.as_deref(), Some("spark"));
    assert_eq!(resolved.native_effort.value.as_deref(), Some("minimal"));
    assert!(resolved.yes.value && resolved.push.value && resolved.fast.value);
    let resolved = plan("", &["--harness=codex", "-as", "-munknown", "-elow"]).selections;
    assert_eq!(resolved.mode.value, Mode::Staged);
    assert_eq!(resolved.model.value.as_deref(), Some("unknown"));
    assert_eq!(resolved.native_effort.value.as_deref(), Some("low"));
}

#[test]
fn final_harness_resolves_presets_and_model_changes_keep_explicit_effort() {
    let config = r#"
[harnesses.codex.models."codex-id"]
aliases = ["shared"]
effort = "ultra"
[harnesses.claude.models."claude-id"]
aliases = ["shared"]
effort = "max"
"#;
    let resolved = plan(
        config,
        &[
            "--harness",
            "codex",
            "--model",
            "shared",
            "-e",
            "low",
            "--harness",
            "claude",
            "-a",
        ],
    )
    .selections;
    assert_eq!(resolved.model.value.as_deref(), Some("claude-id"));
    assert_eq!(resolved.native_effort.value.as_deref(), Some("low"));
    assert_eq!(resolved.effort.source, Source::Cli);
    let resolved = plan(
        "",
        &[
            "--harness",
            "codex",
            "-a",
            "-e",
            "high",
            "-m",
            "spark",
            "-m",
            "future",
        ],
    )
    .selections;
    assert_eq!(resolved.native_effort.value.as_deref(), Some("high"));
}

#[test]
fn user_defaults_take_priority_over_more_specific_bundled_defaults() {
    for (config, expected, source) in [
        ("", "xhigh", Source::BundledModel),
        ("[defaults]\neffort='low'", "low", Source::UserGlobal),
        (
            "[defaults]\neffort='low'\n[harnesses.codex]\neffort='max'",
            "max",
            Source::UserHarness,
        ),
        (
            "[defaults]\neffort='low'\n[harnesses.codex]\neffort='max'\n[harnesses.codex.models.'gpt-5.3-codex-spark']\neffort='ultra'",
            "ultra",
            Source::UserModel,
        ),
    ] {
        let resolved = plan(config, &["--harness", "codex", "-a", "-m", "spark"]).selections;
        assert_eq!(resolved.native_effort.value.as_deref(), Some(expected));
        assert_eq!(resolved.effort.source, source);
    }
}

#[test]
fn bundled_defaults_preserve_all_four_launchers() {
    for (harness, model, effort, args) in [
        (
            "antigravity",
            None,
            Some("medium"),
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                "/a repo/日本語",
                "--prompt-interactive",
                "use the 'commit' skill with arguments --all",
                "--effort",
                "medium",
            ],
        ),
        (
            "copilot",
            Some("gpt-5.6-terra"),
            None,
            vec![
                "--yolo",
                "--model=gpt-5.6-terra",
                "-i",
                "use the 'commit' skill with arguments --all",
            ],
        ),
        (
            "codex",
            Some("gpt-5.6-sol"),
            Some("medium"),
            vec![
                "--x-no-mcp",
                "--disable",
                "memories",
                "-m",
                "gpt-5.6-sol",
                "-c",
                "model_reasoning_effort=medium",
                "$commit --all",
            ],
        ),
        (
            "claude",
            Some("opus"),
            Some("high"),
            vec![
                "--strict-mcp-config",
                "--model",
                "opus",
                "--effort",
                "high",
                "--settings",
                "{\"autoMemoryEnabled\":false}",
                "/commit --all",
            ],
        ),
    ] {
        let plan = plan("", &["--harness", harness, "-a"]);
        assert_eq!(plan.selections.model.value.as_deref(), model);
        assert_eq!(plan.selections.native_effort.value.as_deref(), effort);
        assert_eq!(
            plan.args,
            args.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
    }
}

#[test]
fn presets_mapping_and_raw_escape_are_data_driven() {
    let config = r#"
[harnesses.antigravity.effort_map]
medium = "user-medium"
ultra = "user-ultra"
[harnesses.antigravity.models."gemini-3.1-pro"]
effort_map = { ultra = "model-ultra" }
"#;
    let resolved = plan(
        "",
        &[
            "--harness",
            "antigravity",
            "-a",
            "-m",
            "pro",
            "-e",
            "medium",
        ],
    )
    .selections;
    assert_eq!(resolved.native_effort.value.as_deref(), Some("high"));
    assert_eq!(resolved.native_effort.source, Source::BundledModel);
    for (args, expected, source) in [
        (vec!["-e", "medium"], "user-medium", Source::UserHarness),
        (vec!["-e", "ultra"], "model-ultra", Source::UserModel),
        (vec!["--raw-effort", "ultra"], "ultra", Source::Cli),
    ] {
        let mut all = vec!["--harness", "antigravity", "-a", "-m", "pro"];
        all.extend(args);
        let resolved = plan(config, &all).selections;
        assert_eq!(resolved.native_effort.value.as_deref(), Some(expected));
        assert_eq!(resolved.native_effort.source, source);
    }
    for harness in ["copilot", "claude"] {
        assert_eq!(
            plan("", &["--harness", harness, "-a", "-e", "ultra"])
                .selections
                .native_effort
                .value
                .as_deref(),
            Some("max")
        );
    }
    for effort in ["xhigh", "max", "ultra"] {
        assert_eq!(
            plan("", &["--harness", "antigravity", "-a", "-e", effort])
                .selections
                .native_effort
                .value
                .as_deref(),
            Some("high")
        );
    }
}

#[test]
fn configured_raw_effort_and_harness_default_override_shared_defaults() {
    let config = "[defaults]\nraw_effort='native'\n[harnesses.codex.models.'gpt-5.3-codex-spark']\neffort='harness-default'";
    let default = plan(config, &["--harness", "codex", "-a"]).selections;
    assert_eq!(default.native_effort.value.as_deref(), Some("native"));
    let model = plan(config, &["--harness", "codex", "-a", "-m", "spark"]).selections;
    assert_eq!(model.native_effort.value, None);
    assert_eq!(model.effort.source, Source::UserModel);
    let cli = plan(
        config,
        &[
            "--harness",
            "codex",
            "-a",
            "-m",
            "spark",
            "--raw-effort",
            "custom",
        ],
    )
    .selections;
    assert_eq!(cli.native_effort.value.as_deref(), Some("custom"));
}

#[test]
fn omission_overrides_config_and_literal_sentinels_are_forwarded() {
    let config = "[harnesses.codex]\nmodel='spark'\neffort='ultra'";
    let omitted = plan(
        config,
        &[
            "--harness",
            "codex",
            "-a",
            "--model",
            "harness-default",
            "--effort",
            "harness-default",
        ],
    );
    assert_eq!(omitted.selections.model.value, None);
    assert_eq!(omitted.selections.native_effort.value, None);
    assert_eq!(omitted.args.last().unwrap(), "$commit --all");
    let literal = plan(
        config,
        &[
            "--harness",
            "codex",
            "-a",
            "--model-id",
            "harness-default",
            "--raw-effort",
            "harness-default",
        ],
    );
    assert_eq!(
        literal.selections.model.value.as_deref(),
        Some("harness-default")
    );
    assert_eq!(
        literal.selections.native_effort.value.as_deref(),
        Some("harness-default")
    );
}

#[test]
fn ask_ignores_yes_without_erasing_it() {
    let ask = plan("", &["--harness", "codex", "-a", "-y", "-k", "-p"]);
    assert_eq!(ask.prompt, "$commit --ask-push");
    assert!(ask.selections.yes.value);
    assert!(!ask.selections.effective_yes);
    let all = plan("", &["--harness", "codex", "-k", "-y", "-a"]);
    assert_eq!(all.prompt, "$commit --all-yes");
}

#[test]
fn instructions_flags_and_forwarding_keep_argument_boundaries() {
    for (harness, prefix) in [
        ("codex", "$my-skill"),
        ("claude", "/my-skill"),
        ("copilot", "use the 'my-skill' skill with arguments"),
        ("antigravity", "use the 'my-skill' skill with arguments"),
    ] {
        let plan = plan(
            "",
            &[
                "--harness",
                harness,
                "-syp",
                "--skill",
                "my-skill",
                "use quotes ' \"",
                "日本語 $HOME `touch x` ;",
                "-e",
                "high",
                "--",
                "--model",
                "other model",
                "",
                "-k",
                "$(touch no)",
            ],
        );
        assert_eq!(
            plan.prompt,
            format!("{prefix} --staged-yes-push use quotes ' \" 日本語 $HOME `touch x` ;")
        );
        assert_eq!(
            &plan.args[plan.args.len() - 5..],
            ["--model", "other model", "", "-k", "$(touch no)"].map(OsString::from)
        );
    }
}

#[test]
fn arrays_replace_and_cli_command_replaces_the_entire_prefix() {
    let config = "[harnesses.codex]\ncommand=['custom wrapper', 'prefix arg']\nbase_args=['--custom-policy']";
    let configured = plan(config, &["--harness", "codex", "-a"]);
    assert_eq!(configured.executable, "custom wrapper");
    assert_eq!(configured.args[0], "prefix arg");
    assert_eq!(configured.args[1], "--custom-policy");
    assert!(!configured.args.contains(&OsString::from("--x-no-mcp")));
    let replaced = plan(
        config,
        &[
            "--harness",
            "codex",
            "-a",
            "--command",
            "literal program ; $X",
        ],
    );
    assert_eq!(replaced.executable, "literal program ; $X");
    assert_eq!(replaced.args[0], "--custom-policy");
    assert_eq!(replaced.selections.command.source, Source::Cli);
}

#[test]
fn claude_settings_merge_once_and_no_fast_leaves_native_settings_in_control() {
    let config = "[harnesses.claude.native_settings]\nautoMemoryEnabled=true\nfastMode=false\nother={key='value'}";
    let fast = plan(config, &["--harness", "claude", "-a", "-m", "sonnet", "-f"]);
    assert_eq!(
        fast.args.iter().filter(|arg| *arg == "--settings").count(),
        1
    );
    let i = fast
        .args
        .iter()
        .position(|arg| arg == "--settings")
        .unwrap();
    let settings: serde_json::Value =
        serde_json::from_str(fast.args[i + 1].to_str().unwrap()).unwrap();
    assert_eq!(
        settings,
        serde_json::json!({"autoMemoryEnabled":true,"fastMode":true,"other":{"key":"value"}})
    );
    let canceled = plan(
        "[harnesses.claude.native_settings]\nfastMode=true",
        &["--harness", "claude", "-a", "-f", "--no-fast"],
    );
    assert_eq!(canceled.selections.native_settings["fastMode"], true);
    assert!(!canceled.selections.fast.value);
}

#[test]
fn config_can_select_harness_mode_and_booleans() {
    let resolved = plan("[defaults]\nharness='copilot'\nmode='all'\nyes=true\npush=true\nskill='global'\n[harnesses.copilot]\nmode='ask'\nskill='harness'\npush=false", &[]).selections;
    assert_eq!(resolved.harness.source, Source::UserGlobal);
    assert_eq!(resolved.mode.value, Mode::Ask);
    assert_eq!(resolved.skill.value, "harness");
    assert!(resolved.yes.value);
    assert!(!resolved.push.value);
}

#[test]
fn config_selection_and_informational_flags_work_on_both_sides_of_subcommand() {
    let parsed = cli(&[
        "--config",
        "one",
        "--no-config",
        "--config=two",
        "--no-config",
    ]);
    assert_eq!(parsed.config, ConfigChoice::Disabled);
    let matches = cli::command()
        .try_get_matches_from([
            "acsc",
            "--harness",
            "codex",
            "models",
            "--harness",
            "claude",
            "--config",
            "config.toml",
        ])
        .unwrap();
    let parsed = Overrides::from_matches(&matches);
    assert_eq!(parsed.harness, Some(Harness::Claude));
    assert_eq!(parsed.config, ConfigChoice::File("config.toml".into()));
    let parsed = cli(&["--no-config", "--config", "final"]);
    assert_eq!(parsed.config, ConfigChoice::File("final".into()));
    for (args, expected) in [
        (
            vec!["--config", "old", "models", "--no-config"],
            ConfigChoice::Disabled,
        ),
        (
            vec!["--no-config", "models", "--config", "new"],
            ConfigChoice::File("new".into()),
        ),
    ] {
        assert_eq!(cli(&args).config, expected);
    }
    let parsed = cli(&[
        "-a",
        "-y",
        "-p",
        "-.",
        "-m",
        "spark",
        "-e",
        "high",
        "-f",
        "models",
        "--mode",
        "ask",
        "--no-yes",
        "--no-push",
        "--skill",
        "custom",
        "--model-id",
        "literal",
        "--raw-effort",
        "native",
        "--no-fast",
    ]);
    assert_eq!(parsed.mode, Some(Mode::Ask));
    assert_eq!(parsed.yes, Some(false));
    assert_eq!(parsed.push, Some(false));
    assert_eq!(parsed.fast, Some(false));
    assert_eq!(parsed.skill.as_deref(), Some("custom"));
    assert_eq!(parsed.effort, Some(EffortSelection::Raw("native".into())));
}

#[test]
fn invalid_config_reports_actionable_errors() {
    for config in [
        "[defaults]\neffort='tiny'",
        "[defaults]\neffort='low'\nraw_effort='native'",
        "[harnesses.codex]\ncommand=[]",
        "[harnesses.codex]\ncommand=['']",
        "[harnesses.codex]\nunknown=true",
        "[harnesses.typo]",
        "broken = [",
        "[harnesses.codex.models.x]\neffort='low'\nraw_effort='native'",
        "[harnesses.codex]\neffort_map={low=''}",
        "[defaults]\nskill=''",
    ] {
        assert!(Config::parse(config).is_err(), "{config}");
    }
    assert!(
        Config::load(&ConfigChoice::File(PathBuf::from("/not/a/config.toml")))
            .unwrap_err()
            .to_string()
            .contains("cannot read config")
    );
}

#[test]
fn ambiguous_presets_are_rejected_and_user_alias_arrays_replace_defaults() {
    for config in [
        "[harnesses.codex.models.one]\naliases=['same']\n[harnesses.codex.models.two]\naliases=['same']",
        "[harnesses.codex.models.one]\naliases=['gpt-5.6-sol']",
        "[harnesses.codex.models.one]\naliases=['harness-default']",
    ] {
        assert!(Resolver::new(Config::parse(config).unwrap()).is_err());
    }
    let resolver = Resolver::new(Config::parse("[harnesses.codex.models.'gpt-5.3-codex-spark']\naliases=['quick']\n[discovery.codex_cache]\nenabled=false").unwrap()).unwrap();
    let catalog = Catalog::load(&resolver).unwrap();
    assert!(catalog.find(Harness::Codex, "spark").is_none());
    assert!(catalog.find(Harness::Codex, "quick").is_some());
}

#[test]
fn missing_required_selections_and_unsupported_fast_are_errors() {
    let resolver = Resolver::new(Config::default()).unwrap();
    assert!(
        resolver
            .resolve(&cli(&["-a"]))
            .unwrap_err()
            .to_string()
            .contains("select a harness")
    );
    assert!(
        resolver
            .resolve(&cli(&["--harness", "codex"]))
            .unwrap_err()
            .to_string()
            .contains("select a commit mode")
    );
    for harness in ["antigravity", "copilot"] {
        assert!(
            resolver
                .resolve(&cli(&["--harness", harness, "-a", "-f"]))
                .unwrap_err()
                .to_string()
                .contains("--no-fast")
        );
        assert!(
            !plan("", &["--harness", harness, "-a", "-f", "--no-fast"])
                .selections
                .fast
                .value
        );
    }
}

#[test]
fn shared_effort_is_distinct_from_raw_and_unknown_models_are_accepted() {
    let resolved = plan(
        "",
        &[
            "--harness",
            "codex",
            "-a",
            "--model-id",
            "brand-new-id",
            "--raw-effort",
            "future-effort",
        ],
    )
    .selections;
    assert_eq!(
        resolved.effort.value,
        EffortSelection::Raw("future-effort".into())
    );
    assert_eq!(resolved.model.value.as_deref(), Some("brand-new-id"));
    assert!(
        cli::command()
            .try_get_matches_from(["acsc", "--effort", "unknown"])
            .is_err()
    );
    assert!(
        cli::command()
            .try_get_matches_from(["acsc", "--raw-effort", " "])
            .is_err()
    );
}
