use std::ffi::OsString;
use std::path::PathBuf;

use acsc::catalog::Catalog;
use acsc::cli::{self, Overrides};
use acsc::completion;
use acsc::config::Config;
use acsc::domain::{Harness, ModelSize, Source};
use acsc::launch::LaunchPlan;
use acsc::resolve::{Resolved, Resolver};

fn options(args: &[&str]) -> Overrides {
    let matches = cli::command()
        .try_get_matches_from(std::iter::once("acsc").chain(args.iter().copied()))
        .unwrap();
    Overrides::from_matches(&matches)
}

fn resolve(config: &str, args: &[&str]) -> Resolved {
    Resolver::new(Config::parse(config).unwrap())
        .unwrap()
        .resolve(&options(args))
        .unwrap()
}

#[test]
fn bundled_sizes_select_pinned_models_without_changing_explicit_effort() {
    for (harness, ids) in [
        (
            "antigravity",
            ["gemini-3.8-flash", "gemini-4-argon", "gemini-4-argon"],
        ),
        ("codex", ["gpt-5.6-terra", "gpt-6.1-sol", "gpt-6-astra"]),
        (
            "claude",
            ["claude-sonnet-5-5", "claude-opus-5-5", "claude-fable-5-1"],
        ),
    ] {
        for (size, id) in ModelSize::ALL.into_iter().zip(ids) {
            let cli = options(&[
                "--harness",
                harness,
                "-s",
                "--effort",
                "low",
                "--model-size",
                size.name(),
            ]);
            let resolved = Resolver::new(Config::default())
                .unwrap()
                .resolve(&cli)
                .unwrap();
            assert_eq!(resolved.model.value.as_deref(), Some(id));
            assert_eq!(resolved.model.source, Source::BundledHarness);
            let selected_size = resolved.model_size.as_ref().unwrap();
            assert_eq!(selected_size.value, size);
            assert_eq!(selected_size.source, Source::Cli);
            assert_eq!(resolved.native_effort.value.as_deref(), Some("low"));
            assert_eq!(resolved.effort.source, Source::Cli);
            let plan = LaunchPlan::build(resolved, &cli, PathBuf::from("/repo")).unwrap();
            assert!(
                plan.args
                    .windows(2)
                    .any(|args| { (args[0] == "--model" || args[0] == "-m") && args[1] == id })
            );
        }
    }
}

#[test]
fn model_and_size_options_override_in_both_directions_and_across_subcommands() {
    for tail in [
        vec!["--model-size", "normal", "--model", "spark"],
        vec![
            "--model-size",
            "xlarge",
            "--model-id",
            "gpt-5.3-codex-spark",
        ],
    ] {
        let mut args = vec!["--harness", "codex", "-s"];
        args.extend(tail);
        let resolved = resolve("", &args);
        assert_eq!(resolved.model.value.as_deref(), Some("gpt-5.3-codex-spark"));
        assert!(resolved.model_size.is_none());
    }
    for prefix in ["--model", "--model-id", "--model-size"] {
        let resolved = resolve(
            "",
            &[
                "--harness",
                "codex",
                "-s",
                prefix,
                "normal",
                "--model-size",
                "large",
            ],
        );
        assert_eq!(resolved.model.value.as_deref(), Some("gpt-6.1-sol"));
        assert_eq!(resolved.model_size.unwrap().value, ModelSize::Large);
    }
    let resolved = resolve(
        "",
        &[
            "--harness",
            "codex",
            "-s",
            "--model-size",
            "xlarge",
            "models",
            "--model-id",
            "literal",
        ],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("literal"));
    assert!(resolved.model_size.is_none());
    let resolved = resolve(
        "",
        &[
            "--harness",
            "codex",
            "-s",
            "--model",
            "spark",
            "models",
            "--model-size",
            "large",
            "--harness",
            "claude",
        ],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(resolved.model_size.unwrap().value, ModelSize::Large);
    let resolved = resolve(
        "",
        &[
            "--harness",
            "codex",
            "--model-size",
            "normal",
            "--harness",
            "antigravity",
            "-s",
        ],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("gemini-3.8-flash"));
}

#[test]
fn configured_size_defaults_follow_cli_harness_global_precedence() {
    let config = "[defaults]\nmodel_size='normal'\n[harnesses.claude]\nmodel_size='xlarge'";
    let resolved = resolve(config, &["--harness", "claude", "-s"]);
    assert_eq!(resolved.model.value.as_deref(), Some("claude-fable-5-1"));
    assert_eq!(resolved.model_size.unwrap().source, Source::UserHarness);
    let resolved = resolve(config, &["--harness", "codex", "-s"]);
    assert_eq!(resolved.model.value.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(resolved.model_size.unwrap().source, Source::UserGlobal);
    let resolved = resolve(
        config,
        &["--harness", "claude", "-s", "--model-size", "large"],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(resolved.model_size.unwrap().source, Source::Cli);
    let resolved = resolve(
        config,
        &["--harness", "claude", "-s", "--model", "harness-default"],
    );
    assert!(resolved.model.value.is_none());
    assert!(resolved.model_size.is_none());
    let resolved = resolve(
        "[defaults]\nmodel_size='xlarge'\n[harnesses.codex]\nmodel='spark'",
        &["--harness", "codex", "-s"],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("gpt-5.3-codex-spark"));
    assert!(resolved.model_size.is_none());
}

#[test]
fn user_mappings_override_one_size_and_use_native_metadata_without_alias_resolution() {
    let config = r#"
[harnesses.codex.model_sizes]
large = "custom"
[harnesses.codex.models.custom]
effort = "high"
effort_map = { high = "custom-high" }
"#;
    let resolved = resolve(
        config,
        &["--harness", "codex", "-s", "--model-size", "large"],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("custom"));
    assert_eq!(resolved.model.source, Source::UserHarness);
    assert_eq!(resolved.native_effort.value.as_deref(), Some("custom-high"));
    let resolved = resolve(
        config,
        &["--harness", "codex", "-s", "--model-size", "normal"],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("gpt-5.6-terra"));
    let config = "[harnesses.codex.model_sizes]\nlarge='literal'\n[harnesses.codex.models.other]\naliases=['literal']";
    let resolved = resolve(
        config,
        &["--harness", "codex", "-s", "--model-size", "large"],
    );
    assert_eq!(resolved.model.value.as_deref(), Some("literal"));
}

#[test]
fn missing_mappings_are_actionable_and_can_be_added_for_copilot() {
    let cli = options(&["--harness", "copilot", "-s", "--model-size", "normal"]);
    let error = Resolver::new(Config::default())
        .unwrap()
        .resolve(&cli)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("harnesses.copilot.model_sizes.normal")
    );
    let config = Config::parse("[harnesses.copilot.model_sizes]\nnormal='custom-copilot'").unwrap();
    let resolved = Resolver::new(config).unwrap().resolve(&cli).unwrap();
    assert_eq!(resolved.model.value.as_deref(), Some("custom-copilot"));
}

#[test]
fn ambiguous_or_malformed_size_configuration_is_rejected() {
    for config in [
        "[defaults]\nmodel_size='giant'",
        "[harnesses.codex]\nmodel='sol'\nmodel_size='normal'",
        "[harnesses.codex.model_sizes]\nnormal=' '",
        "[harnesses.codex.model_sizes]\ngiant='native'",
    ] {
        assert!(Config::parse(config).is_err(), "{config}");
    }
}

#[test]
fn catalog_includes_shared_and_user_targets_and_completion_offers_sizes() {
    let resolver = Resolver::new(
        Config::parse("[harnesses.codex.model_sizes]\nxlarge='custom-astra'").unwrap(),
    )
    .unwrap();
    let catalog = Catalog::load(&resolver).unwrap();
    assert_eq!(
        catalog
            .find(Harness::Antigravity, "gemini-4-argon")
            .unwrap()
            .model_sizes,
        [ModelSize::Large, ModelSize::Xlarge]
    );
    let custom = catalog.find(Harness::Codex, "custom-astra").unwrap();
    assert_eq!(custom.model_sizes, [ModelSize::Xlarge]);
    assert_eq!(custom.source, "user");
    let words: Vec<_> = ["acsc", "--harness", "codex"].map(OsString::from).into();
    let mut command = completion::command_for(&words);
    let mut input = words;
    input.extend([OsString::from("--model-size"), OsString::from("")]);
    let index = input.len() - 1;
    let candidates = clap_complete::engine::complete(&mut command, input, index, None).unwrap();
    let values: Vec<_> = candidates
        .iter()
        .map(|value| value.get_value().to_str().unwrap())
        .collect();
    assert_eq!(values, ["normal", "large", "xlarge"]);
}
