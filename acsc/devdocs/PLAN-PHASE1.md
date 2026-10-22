# acsc Phase 1: CLI and shared configuration

`acsc` — agent commit skill command — unifies `agci`, `cpci`, `cxci`, and
`clci` into one Rust CLI. Phase 1 uses clap for argument parsing and establishes
a shared core for a future ratatui interface.

The initial delivery is this planning document only. Implementing the tool is
subsequent work; this delivery does not change installed launchers or create
`PLAN-PHASE2.md`.

## Purpose and scope

Phase 1 covers CLI selection, configuration, model presets, effort mapping,
model discovery, command previews, completion, and launching. It targets
macOS/Linux and Windows.

Existing `agyd`, `copilotd`, `codexd`, and `clauded` wrappers continue to own
their current launch/session behavior. The invoked commit skill continues to
own commit review, staging, Git operations, and final reporting.

Use the installed scripts in `~/.local/bin/` as the behavior baseline. The
2026-10-19 inspection found two differences from this repository's copies:

- Installed `agci` adds Pro/Flash selection, a high default effort for Pro,
  and a Pro-specific adjustment from medium to high effort.
- Installed `codexd` adds `--no-daemon` and keeps startup errors visible until
  the user presses Enter to close the pane.

The repository's existing launchers and wrappers remain in place. Phase 1
does not absorb tmux management, automatically replace legacy commands, or
change the commit workflow itself.

## CLI contract

The normal invocation is flag-oriented:

```text
acsc [options] [instructions...] [-- harness_args...]
```

| Selection | Interface |
|---|---|
| Harness | `--harness antigravity\|copilot\|codex\|claude` |
| Commit mode | `--mode all\|staged\|ask`; `-a/--all`, `-s/--staged`, `-k/--ask` |
| Preconfirmation | `-y/--yes`, `--no-yes` |
| Push | `-p/--push`, `--no-push` |
| Skill | `--skill NAME`; `-./--gdf` selects `gdf-commit` |
| Model | `-m/--model NAME`; `--model-id ID` bypasses preset lookup |
| Effort | `-e/--effort LEVEL`; `--raw-effort VALUE` |
| Fast mode | `-f/--fast`, `--no-fast` |
| Launch command | `--command PROGRAM` |
| Configuration | `--config-file PATH`, `--no-config` |
| Preview | `--dry-run` |
| Information | `-h/--help`, `--version` |

### Selection and overrides

- Harness and mode must be supplied by flags or configuration. Phase 1 does
  not open a picker when they are missing.
- Later selections override earlier selections, including different
  spellings for the same setting. This applies to harness, mode, skill,
  model, effort, command, and positive/negative flag pairs.
- Resolve the final harness and model before applying defaults and mappings.
  Changing models does not erase an explicit effort selection.
- `--model NAME` resolves a preset within the selected harness, otherwise
  passes through a literal model ID. `--model-id` and `--model` override each
  other by position.
- `--model harness-default` and `--effort harness-default` omit the
  corresponding harness argument. These override configured or earlier CLI
  selections. `--model-id` and `--raw-effort` remain literal escape hatches.
- Ask mode always remains interactive. A resolved `yes=true` has no effect
  while the final mode is `ask`; it remains applicable if the final mode
  becomes `all` or `staged`.
- `--no-fast` removes acsc's fast-mode request. It does not promise to disable
  a setting independently enabled by the harness or wrapper.
- Bare arguments become additional instructions, joined with spaces as in
  the existing scripts. Arguments after `--` are forwarded unchanged and
  appended last.
- Last-wins guarantees apply to acsc's options. Duplicate-option behavior
  inside forwarded arguments belongs to the wrapper/harness.
- Keep the shared short forms above. Retire the old model letters and
  repeated effort shortcuts; `-m` means model and `-h` means help.

### Informational commands

```text
acsc --models [--harness HARNESS]
acsc --completion SHELL
```

Model listing does not require a commit mode. Without a selected harness, it
groups results by harness. Completion supports Bash, Zsh, Fish, and PowerShell.
Management entry points require the `--` prefix, including
`acsc --config init|view|edit`. Bare `models`, `completion`, `config`, and `help` remain
additional instructions. Options belonging to a management subcommand stay
in that subcommand's help.

### Aliases

```sh
alias cxci='acsc --harness codex'
alias quickcommit='acsc --harness codex -a -y -p'

quickcommit --staged --no-push --effort high
```

The example's final selections are Codex, staged changes, preconfirmed, no
push, and high effort.

PowerShell uses forwarding functions:

```powershell
function cxci { acsc --harness codex @args }
function quickcommit { acsc --harness codex -a -y -p @args }

quickcommit --staged --no-push --effort high
```

Do not infer behavior from the executable's basename or automatically install
legacy command names. Alias prefixes should contain acsc options; putting
`--` in a prefix makes subsequent arguments harness arguments.

## Configuration, presets, and effort

### Configuration sources

Use optional TOML configuration:

- Unix: `$XDG_CONFIG_HOME/acsc/config.toml`, falling back to
  `~/.config/acsc/config.toml`.
- Windows: `%APPDATA%\acsc\config.toml`.
- `--config-file` selects an alternative file; `--no-config` disables user
  configuration. Their last occurrence wins.
- Missing default configuration is normal. Invalid configuration or an
  explicitly selected missing file produces an actionable error.
- No repository-local configuration or general named profiles in Phase 1.

Global defaults can select harness, mode, skill, effort, preconfirmation,
push, and fast mode. Harness sections can override applicable defaults and
configure model, launch command, policy arguments, and model entries. Model
entries are keyed by native model ID and can define aliases, default effort,
effort mappings, and native-effort suggestions.

For effort defaults, use this precedence, highest first:

```text
explicit CLI selection
user model default
user harness default
user global default
bundled model default
bundled harness default
bundled global default
```

Thus, a user's global effort overrides the scripts' packaged per-harness
defaults unless the user provides a more specific setting. Bundled global
effort is medium; the per-harness and per-model defaults below take precedence
over that fallback.

The skill defaults to `commit`. Preconfirmation, push, and an acsc fast-mode
request are disabled unless selected. There is no bundled default harness or
commit mode.

### Shared effort ramp

```text
low → medium → high → xhigh → max → ultra
```

`--effort` selects a shared level and translates it through the effective
mapping. `--raw-effort` bypasses translation. Configuration supports the same
distinction through mutually exclusive `effort` and `raw_effort` fields at
each scope. `effort = "harness-default"` omits the effort argument.

Initial mappings:

- Codex: identity.
- Copilot and Claude: identity through `max`; `ultra` maps to `max`.
- Antigravity: identity through `high`; `xhigh`, `max`, and `ultra` map to
  `high`.
- Preserve the installed Pro preset's `medium → high` adjustment as
  model-specific mapping data.

Mappings are configurable per harness and model. Resolve each entry using
user model → user harness → bundled model → bundled harness → identity.
These are translation rules, not a maintained capability validator; the
harness validates native combinations.

### Initial defaults and presets

Initial defaults preserve the installed launchers:

| Harness | Unix command | Model | Effort |
|---|---|---|---|
| Antigravity | `agyd` | Harness default | Medium |
| Copilot | `copilotd` | `gpt-5.6-terra` | Harness default |
| Codex | `codexd` | `gpt-5.6-sol` | Medium |
| Claude | `clauded` | `opus` | High |

Bundle the existing model selections as data:

| Harness | Preset name | Native model ID | Model-specific default effort |
|---|---|---|---|
| Antigravity | `pro` | `gemini-3.1-pro` | High |
| Antigravity | `flash` | `gemini-3.8-flash` | Inherit |
| Claude | `opus` | `opus` | Inherit |
| Claude | `fable` | `fable` | Inherit |
| Claude | `sonnet` | `sonnet` | Inherit |
| Claude | `haiku` | `haiku` | Inherit |

These are defaults derived from the inspected scripts, not claims about the
latest available models. They remain editable and do not automatically follow
discovered models.

### Example configuration

This example replaces the bundled Codex model with a user-defined `sol`
preset and demonstrates effort overrides and mapping data:

```toml
[defaults]
effort = "medium"

[harnesses.codex]
model = "sol"
command = ["codexd"]
base_args = ["--x-no-mcp", "--disable", "memories"]

[harnesses.codex.models."gpt-6.1-sol"]
aliases = ["sol"]
effort = "high"

[harnesses.antigravity.effort_map]
low = "low"
medium = "medium"
high = "high"
xhigh = "high"
max = "high"
ultra = "high"

[harnesses.antigravity.models."gemini-3.1-pro"]
aliases = ["pro"]
effort = "high"
effort_map = { medium = "high" }

[harnesses.claude.native_settings]
autoMemoryEnabled = false
```

Commands are executable-plus-prefix-argument arrays, without shell
evaluation. `--command PROGRAM` replaces that entire command array with a
single executable. Arrays replace their inherited values rather than
accumulating.

Preserve existing launch-policy arguments as editable `base_args`. Claude
additionally has a native-settings table so memory settings and an acsc
fast-mode request serialize into one JSON settings argument. An enabled acsc
fast-mode request sets `fastMode` to true in that argument. Without an acsc
fast-mode request, the native-settings table is left in control of that key.

## Shared core, discovery, and launching

### Resolution and preview

Organize Phase 1 around a reusable resolution pipeline:

```text
CLI/config → resolved selections → harness adapter → launch plan
```

The shared core owns defaults, preset resolution, effort translation,
validation, prompt construction, and catalog data. CLI rendering and process
execution remain separate. Retain the source of resolved selections so
previews and the later TUI can explain defaults and translations.

Use Rust with clap, serde/TOML, serde_json, and clap_complete. Add ratatui when
implementing Phase 2; Phase 1 establishes the reusable interfaces without
designing screens or widgets.

The launch plan contains the executable, argument vector, working directory,
resolved selections, and prompt. `--dry-run` displays these without running a
wrapper or harness.

### Harness adapters

Preserve prompt construction:

- Codex: `$NAME --MODE…`
- Claude: `/NAME --MODE…`
- Antigravity/Copilot: `use the 'NAME' skill with arguments --MODE…`

Arbitrary skill names are permitted and receive the same commit-mode
arguments. Compose the existing `-yes` and `-push` suffixes, with `-yes` only
applicable to all/staged modes. Append any additional instructions to the
skill invocation.

Preserve adapter behavior such as Antigravity's working-directory argument
and each harness's interactive prompt placement. Fast-mode translation exists
for Codex and Claude; requesting it for an unsupported adapter produces an
actionable error. Do not maintain model-specific fast-mode eligibility rules.

### Process launching

Launch with inherited working directory, environment, and terminal streams.
Preserve argument boundaries without interpreting instruction text or
forwarded arguments as shell code. On Unix, replace the process where
appropriate. On Windows, invoke the existing PowerShell wrappers through
`pwsh -NoProfile -File`, resolving the wrapper from PATH; wait for completion
and propagate status without changing execution policy.

Report launch failures and wrapper exit status accurately. A successful
wrapper launch is not evidence that a commit or push succeeded. In particular,
a wrapper that starts a tmux session can finish before the agent does.

### Model discovery and completion

- Merge configured model entries with the optional Codex model cache.
- Default to `$CODEX_HOME/models_cache.json`, or
  `~/.codex/models_cache.json`; allow a configured path or disabled discovery.
- Read IDs, display names, visibility, ordering, native effort lists, and
  advertised defaults. Ignore unrelated fields.
- Hide cache entries marked hidden unless explicitly configured.
- Configured entries take precedence; deduplicate by native ID.
- Missing, unreadable, malformed, or older caches do not block launching.
  Never refresh the cache or contact services.
- Cache metadata supplies suggestions and information only. It does not
  change effective defaults, mappings, or accepted explicit model IDs.
- Use the same catalog for model listing, dynamic completion, and future TUI
  choices. Completion remains quiet and useful with incomplete input or
  unavailable optional data.

## Validation and Phase 2 handoff

### Implementation tests

Use focused parser, resolver, adapter, and process-launch tests with fake
executables rather than live agents. Cover:

- Later overrides across repeated options, shorthand forms, and
  positive/negative pairs.
- Final-harness preset lookup and explicit effort surviving model changes.
- Global/harness/model precedence, raw effort, omission, and mapping
  overrides.
- Ask-mode interactivity with preconfirmation enabled.
- Prompt and argument construction for every harness, including push and
  additional instructions.
- Claude memory/fast settings combined into one argument.
- Spaces, quotes, Unicode, shell metacharacters, executable paths, and
  forwarded argument boundaries.
- Missing/broken optional caches, hidden entries, configured overrides, and
  completion with incomplete arguments.
- Dry-run and informational commands launching no child processes.
- Missing executables, nonzero wrapper status, Unix execution, and Windows
  PowerShell forwarding.

Implementation validation uses `cargo check`, `cargo build`,
`cargo nextest run`, formatting checks, and clippy, with platform-specific
launch tests on Unix and Windows.

### Phase 2 boundary

Phase 2 receives the shared selection model, catalog, resolver, and launch
preview. Picker layouts, interaction, saved TUI state, and automatic TUI
startup remain for `PLAN-PHASE2.md`.

### Document delivery

For the initial document-only delivery, verify that this Markdown captures
the agreed decisions and examples, check it for whitespace errors, and confirm
that only `acsc/devdocs/PLAN-PHASE1.md` was added. Tool implementation tests do
not apply to this document-only change.
