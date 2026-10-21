mod common;

use std::ffi::OsString;
use std::fs;

use acsc::catalog::Catalog;
use acsc::completion;
use acsc::config::Config;
use acsc::domain::Harness;
use acsc::resolve::Resolver;
use common::{binary, quoted};
use tempfile::TempDir;

#[test]
fn cache_merges_config_metadata_hides_hidden_entries_and_deduplicates() {
    let temp = TempDir::new().unwrap();
    let cache = temp.path().join("cache.json");
    fs::write(&cache, r#"{"unrelated":true,"models":[
        {"slug":"new","display_name":"New Name","priority":3,"default_reasoning_level":"ultra","supported_reasoning_levels":[{"effort":"ultra"},"low"]},
        {"slug":"hidden","visibility":"hide"},
        {"slug":"configured-hidden","visibility":"hide","display_name":"Cached Name","supported_reasoning_levels":[{"effort":"low"}]},
        {"slug":"gpt-5.6-sol","priority":2},
        {"slug":"new","priority":1,"default_reasoning_level":"low"},
        {"old_schema":true}, {"slug":42}, null
    ]}"#).unwrap();
    let config = format!(
        r#"
[harnesses.codex.models."configured-hidden"]
display_name = "User Name"
aliases = ["hidden-preset"]
native_efforts = ["custom"]
effort = "high"
[discovery.codex_cache]
path = {}
"#,
        quoted(&cache)
    );
    let resolver = Resolver::new(Config::parse(&config).unwrap()).unwrap();
    let catalog = Catalog::load(&resolver).unwrap();
    let codex: Vec<_> = catalog.for_harness(Some(Harness::Codex)).collect();
    assert_eq!(codex.iter().filter(|m| m.id == "gpt-5.6-sol").count(), 1);
    assert_eq!(codex.iter().filter(|m| m.id == "new").count(), 1);
    assert!(catalog.find(Harness::Codex, "hidden").is_none());
    let configured = catalog.find(Harness::Codex, "hidden-preset").unwrap();
    assert_eq!(configured.display_name, "User Name");
    assert_eq!(configured.native_efforts, ["custom"]);
    assert_eq!(configured.source, "user");
    let new = catalog.find(Harness::Codex, "new").unwrap();
    assert_eq!(new.priority, 1);
    assert_eq!(new.advertised_default.as_deref(), Some("low"));
    assert!(
        codex.iter().position(|m| m.id == "new").unwrap()
            < codex.iter().position(|m| m.id == "gpt-5.6-sol").unwrap()
    );
}

#[test]
fn optional_cache_failures_and_disabled_cache_do_not_block_resolution() {
    let temp = TempDir::new().unwrap();
    let cache = temp.path().join("cache.json");
    for contents in [
        None,
        Some("not json"),
        Some("{}"),
        Some(r#"{"models":[{"slug":"valid"}, {"slug":null}]}"#),
    ] {
        if let Some(contents) = contents {
            fs::write(&cache, contents).unwrap();
        }
        let config = format!("[discovery.codex_cache]\npath={}", quoted(&cache));
        let resolver = Resolver::new(Config::parse(&config).unwrap()).unwrap();
        assert!(
            Catalog::load(&resolver)
                .unwrap()
                .find(Harness::Codex, "spark")
                .is_some()
        );
    }
    let config = format!(
        "[discovery.codex_cache]\nenabled=false\npath={}",
        quoted(&cache)
    );
    assert!(
        Catalog::load(&Resolver::new(Config::parse(&config).unwrap()).unwrap())
            .unwrap()
            .find(Harness::Codex, "valid")
            .is_none()
    );
    // A directory in place of the file exercises an unreadable data source.
    let config = format!("[discovery.codex_cache]\npath={}", quoted(temp.path()));
    assert!(Catalog::load(&Resolver::new(Config::parse(&config).unwrap()).unwrap()).is_ok());
}

#[test]
fn discovery_uses_codex_home_and_never_changes_launch_defaults() {
    let temp = TempDir::new().unwrap();
    fs::create_dir(temp.path().join("codex")).unwrap();
    let cache = temp.path().join("codex/models_cache.json");
    let contents = r#"{"models":[{"slug":"new","display_name":"Latest","default_reasoning_level":"ultra","priority":1,"supported_reasoning_levels":[{"effort":"ultra"}]}]}"#;
    fs::write(&cache, contents).unwrap();
    let models = binary(temp.path())
        .args(["models", "--harness", "codex"])
        .output()
        .unwrap();
    assert!(
        models.status.success(),
        "{}",
        String::from_utf8_lossy(&models.stderr)
    );
    let output = String::from_utf8(models.stdout).unwrap();
    assert!(output.contains("new  [cache]"));
    assert!(output.contains("native efforts: ultra"));
    assert!(output.contains("cache advertised default (information only): ultra"));
    let preview = binary(temp.path())
        .args(["--harness", "codex", "-a", "--dry-run"])
        .output()
        .unwrap();
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["selections"]["model"]["value"], "gpt-5.6-sol");
    assert_eq!(preview["selections"]["native_effort"]["value"], "medium");
    let preview = binary(temp.path())
        .args([
            "--harness",
            "codex",
            "-a",
            "--model-size",
            "large",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(preview.status.success());
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["selections"]["model"]["value"], "gpt-6.1-sol");
    assert_eq!(preview["selections"]["model_size"]["value"], "large");
    assert_eq!(fs::read_to_string(cache).unwrap(), contents);
}

#[test]
fn model_sizes_are_listed_and_complete_native_efforts_for_the_mapped_model() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");
    fs::write(&config, "[harnesses.claude.model_sizes]\nlarge='size-target'\n[harnesses.claude.models.size-target]\nnative_efforts=['native-tier-effort']\n[discovery.codex_cache]\nenabled=false").unwrap();
    let output = binary(temp.path())
        .args(["--config"])
        .arg(&config)
        .args(["models", "--harness", "claude"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output.contains("size-target  [user]\n    model sizes: large"));
    let words: Vec<_> = [
        "acsc",
        "--config",
        config.to_str().unwrap(),
        "--harness",
        "claude",
        "--model-size",
        "large",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let mut command = completion::command_for(&words);
    let mut input = words;
    input.extend([OsString::from("--raw-effort"), OsString::from("native")]);
    let index = input.len() - 1;
    let candidates = clap_complete::engine::complete(&mut command, input, index, None).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].get_value(), "native-tier-effort");
    let output = binary(temp.path())
        .arg("--config")
        .arg(&config)
        .args([
            "--harness",
            "claude",
            "-s",
            "--model-size",
            "large",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let preview: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(preview["selections"]["model_size"]["source"], "cli");
    assert_eq!(preview["selections"]["model"]["value"], "size-target");
    assert_eq!(preview["selections"]["model"]["source"], "user_harness");
}

#[test]
fn completion_uses_final_harness_user_presets_and_native_efforts() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");
    fs::write(&config, "[harnesses.claude.models.special]\naliases=['mine']\nnative_efforts=['native-level']\n[discovery.codex_cache]\nenabled=false").unwrap();
    let words: Vec<_> = [
        "acsc",
        "--harness",
        "codex",
        "--config",
        config.to_str().unwrap(),
        "--harness",
        "claude",
        "--model",
        "mine",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let mut command = completion::command_for(&words);
    let mut input = words.clone();
    input.extend([OsString::from("--raw-effort"), OsString::from("native")]);
    let index = input.len() - 1;
    let candidates =
        clap_complete::engine::complete(&mut command, input, index, Some(temp.path())).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].get_value(), "native-level");
    let mut command = completion::command_for(&words);
    let mut input = words;
    input.extend([OsString::from("--model"), OsString::from("mi")]);
    let index = input.len() - 1;
    let candidates =
        clap_complete::engine::complete(&mut command, input, index, Some(temp.path())).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].get_value(), "mine");
}

#[test]
fn cached_ids_take_precedence_over_aliases_when_looking_up_a_resolved_native_id() {
    let temp = TempDir::new().unwrap();
    let cache = temp.path().join("cache.json");
    fs::write(&cache, r#"{"models":[{"slug":"native","priority":10,"supported_reasoning_levels":[{"effort":"native-effort"}]}]}"#).unwrap();
    let config = format!(
        "[harnesses.codex.models.other]\naliases=['native']\nnative_efforts=['other-effort']\n[discovery.codex_cache]\npath={}",
        quoted(&cache)
    );
    let resolver = Resolver::new(Config::parse(&config).unwrap()).unwrap();
    let catalog = Catalog::load(&resolver).unwrap();
    assert_eq!(
        catalog
            .find(Harness::Codex, "native")
            .unwrap()
            .native_efforts,
        ["native-effort"]
    );
}

#[test]
fn actual_dynamic_completion_is_quiet_with_incomplete_input_and_broken_data() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("config.toml");
    fs::write(&config, "malformed = [").unwrap();
    let output = binary(temp.path())
        .env("ACSC_COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", "7")
        .args([
            "--",
            "acsc",
            "--config",
            config.to_str().unwrap(),
            "--harness",
            "codex",
            "-a",
            "--model",
            "sp",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Cursor indices are zero based; the last word is index 7 in this request.
    assert!(String::from_utf8(output.stdout).unwrap().contains("spark"));
    for tail in [
        vec!["--harness"],
        vec!["--effort"],
        vec!["--harness", "co"],
        vec!["--config"],
    ] {
        let mut args = vec!["--", "acsc"];
        args.extend(tail);
        let output = binary(temp.path())
            .env("ACSC_COMPLETE", "bash")
            .env("_CLAP_COMPLETE_INDEX", (args.len() - 2).to_string())
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn completions_are_generated_for_all_requested_shells_without_loading_config() {
    let temp = TempDir::new().unwrap();
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let output = binary(temp.path())
            .args(["--config", "missing.toml", "completion", shell])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let script = String::from_utf8(output.stdout).unwrap();
        assert!(script.contains("ACSC_COMPLETE"));
        assert!(script.contains("acsc"));
    }
}
