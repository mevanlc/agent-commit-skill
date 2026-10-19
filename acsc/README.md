# acsc

`acsc` (agent commit skill command) launches the commit skill in Antigravity,
Copilot, Codex, or Claude. It replaces the flag-selection work in `agci`,
`cpci`, `cxci`, and `clci`, while the existing `*d` wrappers handle terminal
sessions and the skill handles commit review and Git operations.

## Build and install

From the repository root:

```sh
cargo build --manifest-path acsc/Cargo.toml
cargo install --path acsc --locked
```

The binary installs to Cargo's bin directory. Install the commit skills using
the [repository installer](../README.md#install), and put the appropriate
wrappers from `utils/` on PATH: `agyd`, `copilotd`, `codexd`, `clauded` on
macOS/Linux; the corresponding `.ps1` scripts on Windows. Windows requires
`pwsh` for PowerShell wrappers. Existing installed wrappers are also usable.
Use `--command PROGRAM` or configuration to select another executable.

## Usage

```sh
acsc --harness codex --staged
acsc --harness codex -a -y -p -m spark -e high
acsc --harness claude -k -. --model sonnet
acsc --harness antigravity -a --model pro
acsc --harness copilot -s use imperative mood -- --native-option value
acsc --harness codex -a --dry-run
acsc models --harness codex
```

Harness and mode must come from flags or configuration. Additional instruction
words are joined with spaces. Every argument after `--` goes unchanged to the
wrapper at the end of its argument vector. `--dry-run` prints JSON containing
the command, argument vector, working directory, prompt, resolved values, and
their sources without starting a child process.

| Selection | Options |
|---|---|
| Harness | `--harness antigravity\|copilot\|codex\|claude` |
| Mode | `--mode all\|staged\|ask`; `-a/--all`, `-s/--staged`, `-k/--ask` |
| Preconfirm | `-y/--yes`, `--no-yes` |
| Push | `-p/--push`, `--no-push` |
| Skill | `--skill NAME`; `-./--gdf` selects `gdf-commit` |
| Model | `-m/--model NAME`, `--model-id ID` |
| Effort | `-e/--effort LEVEL`, `--raw-effort VALUE` |
| Fast | `-f/--fast`, `--no-fast` (Codex/Claude) |
| Executable | `--command PROGRAM` |
| Config | `--config PATH`, `--no-config` |
| Preview | `--dry-run` |

Later selections win, including different spellings for the same selection.
The final harness determines how presets resolve, and changing models keeps
an explicit effort. This makes shell aliases useful:

```sh
alias cxci='acsc --harness codex'
alias quickcommit='acsc --harness codex -a -y -p'
quickcommit --staged --no-push --effort high
```

PowerShell uses functions:

```powershell
function cxci { acsc --harness codex @args }
function quickcommit { acsc --harness codex -a -y -p @args }
quickcommit --staged --no-push --effort high
```

The example commits staged changes, preconfirms the review gate, and requests
high effort. Ask mode ignores `--yes` and always remains interactive. Skill
arguments use the existing `--all-yes-push` / `--staged-push` / `--ask-push`
suffixes. Arbitrary skill names receive the same arguments.

`-h` means help and `-m` means model. Legacy model letters and repeated effort
shortcuts are replaced by named values. The `models` and `completion` words
are subcommands; use another opening word when writing instructions that
would otherwise start with one of these names.

## Configuration and defaults

User configuration is optional TOML at:

- Unix: `$XDG_CONFIG_HOME/acsc/config.toml`, otherwise `~/.config/acsc/config.toml`.
- Windows: `%APPDATA%\acsc\config.toml`.

`--config PATH` selects another file; `--no-config` disables user configuration.
Their final occurrence wins. Missing default files are normal; malformed
files and explicitly selected missing files report errors. There is no
repository-local config or named-profile layer.

See [config.example.toml](config.example.toml) for a working example and
[src/defaults.toml](src/defaults.toml) for the complete bundled data. Both are
editable TOML rather than model capability claims.

| Harness | Default wrapper (Unix) | Default model | Default effort |
|---|---|---|---|
| Antigravity | `agyd` | Harness default | medium |
| Copilot | `copilotd` | `gpt-5.6-terra` | Harness default |
| Codex | `codexd` | `gpt-5.6-sol` | medium |
| Claude | `clauded` | `opus` | high |

Windows uses `.ps1` wrapper names. Bundled presets include Antigravity `pro`
and `flash`, Codex `spark`, and Claude `opus`, `fable`, `sonnet`, and `haiku`.
`pro` defaults to high effort, and `spark` to xhigh.

`--model` resolves aliases within the final harness and otherwise accepts a
literal model ID. `--model-id` always uses its literal ID. `--model
harness-default` omits the native model option; the literal escape still
allows an ID spelled `harness-default`.

The shared effort ramp is `low`, `medium`, `high`, `xhigh`, `max`, `ultra`.
Codex uses these unchanged. Copilot and Claude map ultra to max; Antigravity
maps levels above high to high. The Pro model maps medium to high.
`--raw-effort` bypasses mapping. `--effort harness-default` omits the native
effort argument. The harness validates native model/effort combinations.

Effort defaults resolve in this order:

1. Explicit CLI selection.
2. User model, harness, then global default.
3. Bundled model, harness, then global default.

A user global effort therefore overrides bundled per-model defaults. At any
scope, use either `effort` or `raw_effort`. Mapping entries resolve individually
through user model, user harness, bundled model, bundled harness, then identity.
Model metadata is keyed by native ID:

```toml
[defaults]
harness = "codex"
mode = "staged"
effort = "medium"

[harnesses.codex]
model = "sol"

[harnesses.codex.models."gpt-6.1-sol"]
aliases = ["sol"]
effort = "high"
effort_map = { ultra = "max" }
native_efforts = ["low", "high", "max"]
```

Global defaults accept `harness`, `mode`, `skill`, `effort`/`raw_effort`, `yes`,
`push`, and `fast`. Harness sections accept those applicable selections plus
`model`, `command`, `base_args`, `effort_map`, `native_settings`, and `models`.
Model entries accept `aliases`, `display_name`, `effort`/`raw_effort`,
`effort_map`, and `native_efforts`. Duplicate aliases or aliases that shadow a
different configured ID produce an error.

`command` is an executable-plus-prefix-argument array. Arrays replace inherited
arrays. `--command PROGRAM` replaces the entire command array with one
executable. Arguments and instruction text are passed without shell evaluation.
Policy flags such as MCP and permissions settings live in `base_args`; the
wrappers retain their own policies.

Claude's `native_settings` entries merge by key with bundled settings and
serialize as one `--settings` JSON argument. Fast mode sets `fastMode=true`.
`--no-fast` cancels acsc's request while leaving a separately configured native
fast setting in control. Fast requests for other adapters report an error.

## Model discovery and completion

`acsc models` groups presets and suggestions by harness. `--harness` narrows
the listing. Codex suggestions additionally read `$CODEX_HOME/models_cache.json`
or `~/.codex/models_cache.json`. Configure discovery with:

```toml
[discovery.codex_cache]
enabled = true
# path = "/another/models_cache.json"
```

Hidden cache entries appear only when explicitly present in the model tables.
Configured metadata takes precedence. Cache names, native effort suggestions,
and advertised defaults are informational: they never change launch defaults
or validate an explicit selection. Missing or broken caches are ignored. acsc
does not refresh the cache or contact a service.

Generate dynamic completion registration on shell startup so it follows the
installed binary and current configuration:

```sh
# Bash (~/.bashrc) or Zsh (~/.zshrc)
source <(acsc completion bash)
source <(acsc completion zsh)

# Fish (config.fish)
acsc completion fish | source
```

```powershell
# PowerShell ($PROFILE)
acsc completion powershell | Out-String | Invoke-Expression
```

The registration calls acsc to complete options, harnesses, modes, model
IDs/presets, and efforts. Model suggestions use the final selected harness and
config; native effort suggestions follow the selected model. Incomplete input
and unavailable optional data remain quiet. The same catalog and resolver are
available through the Rust library for the future TUI.

## Launch status and development

Unix replaces the acsc process with the wrapper. Windows resolves `.ps1`
wrappers from PATH and invokes `pwsh -NoProfile -File`, without changing
execution policy. Both inherit cwd, environment, and terminal streams and
preserve the wrapper's status. A wrapper that launches tmux may finish before
the agent; its status does not establish whether a commit or push succeeded.

From `acsc/`:

```sh
cargo check
cargo build
cargo nextest run
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Tests use fake executables and temporary configuration/cache files. The CI
matrix exercises macOS, Linux, and Windows, including PowerShell forwarding
on Windows. Phase 2 will add ratatui interaction using the shared core; no TUI
starts automatically in Phase 1.
