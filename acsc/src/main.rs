use std::io::{self, Write};

use anyhow::{Context, Result};

use acsc::catalog::{Catalog, effort_label};
use acsc::cli::{self, Overrides};
use acsc::completion;
use acsc::config::Config;
use acsc::domain::Harness;
use acsc::launch::LaunchPlan;
use acsc::resolve::Resolver;

fn run() -> Result<i32> {
    if completion::enabled() {
        // Completion is best effort and silent, including incomplete input.
        let _ = completion::complete();
        return Ok(0);
    }
    let matches = cli::command().get_matches();
    if let Some(("completion", subcommand)) = matches.subcommand() {
        completion::registration(
            subcommand
                .get_one::<String>("shell")
                .expect("required shell"),
            &mut io::stdout().lock(),
        )?;
        return Ok(0);
    }
    let overrides = Overrides::from_matches(&matches);
    let resolver = Resolver::new(Config::load(&overrides.config)?)?;
    if matches.subcommand_name() == Some("models") {
        let catalog = Catalog::load(&resolver)?;
        let selected = resolver
            .selected_harness(&overrides)
            .map(|value| value.value);
        let mut output = io::stdout().lock();
        for harness in Harness::ALL
            .into_iter()
            .filter(|h| selected.is_none_or(|selected| selected == *h))
        {
            writeln!(output, "{harness}:")?;
            for model in catalog.for_harness(Some(harness)) {
                writeln!(
                    output,
                    "  {}  [{}]{}",
                    model.id,
                    model.source,
                    if model.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" aliases: {}", model.aliases.join(", "))
                    }
                )?;
                if model.display_name != model.id {
                    writeln!(output, "    {}", model.display_name)?;
                }
                if let Some(effort) = &model.configured_effort {
                    writeln!(output, "    model effort: {}", effort_label(effort))?;
                }
                if !model.native_efforts.is_empty() {
                    writeln!(
                        output,
                        "    native efforts: {}",
                        model.native_efforts.join(", ")
                    )?;
                }
                if let Some(default) = &model.advertised_default {
                    writeln!(
                        output,
                        "    cache advertised default (information only): {default}"
                    )?;
                }
            }
        }
        return Ok(0);
    }
    let resolved = resolver.resolve(&overrides)?;
    let plan = LaunchPlan::build(
        resolved,
        &overrides,
        std::env::current_dir().context("cannot determine working directory")?,
    )?;
    if overrides.dry_run {
        let mut output = io::stdout().lock();
        serde_json::to_writer_pretty(&mut output, &plan)?;
        writeln!(output)?;
        Ok(0)
    } else {
        plan.execute()
    }
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe) => {}
        Err(error) => {
            eprintln!("acsc: {error:#}");
            std::process::exit(1);
        }
    }
}
