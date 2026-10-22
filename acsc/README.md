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
acsc --harness codex -a -y -p -m gpt-6.1-sol -e high
acsc --harness claude -k -. --model sonnet
acsc --harness antigravity -a --model pro
acsc --harness copilot -s use imperative mood -- --native-option value
acsc --harness codex -a --dry-run
acsc --harness codex -s --model-size large -e high
acsc --models --harness codex
acsc --config edit
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
| Model strength | `--model-size normal\|large\|xlarge` |
| Effort | `-e/--effort LEVEL`, `--raw-effort VALUE` |
| Fast | `-f/--fast`, `--no-fast` (Codex/Claude) |
| Executable | `--command PROGRAM` |
| Config | `--config-file PATH`, `--no-config` |
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
shortcuts are replaced by named values. Management commands require their flag
spellings: `--models`, `--config`, and `--completion`. Bare words such as
`models`, `config`, `completion`, and `help` are ordinary additional instructions.
After `--config`, the operations `init`, `view`, and `edit` select that command's
subcommands. Their options remain scoped: `--overwrite` appears in
`acsc --config init --help`, while top-level help lists the management entry
points. Arguments after `--` are always forwarded to the harness, including
words that look like management flags.

## Configuration and defaults

User configuration is optional TOML at:

- Unix: `$XDG_CONFIG_HOME/acsc/config.toml`, otherwise `~/.config/acsc/config.toml`.
- Windows: `%APPDATA%\acsc\config.toml`.

`--config-file PATH` selects another file; `--no-config` disables user
configuration. Their final occurrence wins. Missing default files are normal; malformed
files and explicitly selected missing files report errors. There is no
repository-local config or named-profile layer.

```sh
acsc --config edit                  # Open in $VISUAL, then $EDITOR
acsc --config view                  # Print the file exactly as written
acsc --config init                  # Create a commented reference to bundled defaults
acsc --config init --overwrite      # Replace an existing file
acsc --config edit --config-file ./my-config.toml
```

`init` creates parent directories and refuses to replace an existing file unless
`--overwrite` is supplied. Every setting and table header in the starter is
commented out, so creating it keeps the bundled behavior. Uncomment the table
header and individual settings you want to change. Harness and mode examples
are included, since neither has a bundled default.

`edit` creates the same starter if the file is missing. It uses the first
nonblank value of `$VISUAL` or `$EDITOR`, falling back to `vi` on Unix or
`notepad.exe` on Windows, and waits for the editor to exit. Editor values may
include arguments, such as `code --wait`, and use shell-style quoting for paths
with spaces. A value that names an existing executable file also works as a
literal path. Arguments are passed without shell evaluation; for example,
quote a Windows path as `'"C:\Program Files\Editor\editor.exe" --wait'` when
setting the environment variable in PowerShell. The editor inherits the terminal
and its exit code is returned.

`view` prints the selected file's contents without parsing TOML; it reports an
error if the file is missing. Editing also works with malformed configuration.
All three commands honor `--config-file PATH` before or after the subcommands,
with later selections winning. `--no-config` reports an error for these commands
unless followed by `--config-file PATH`.

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
and `flash`, and Claude `opus`, `fable`, `sonnet`, and `haiku`.
`pro` defaults to high effort.

`--model` resolves aliases within the final harness and otherwise accepts a
literal model ID. `--model-id` always uses its literal ID. `--model
harness-default` omits the native model option; the literal escape still
allows an ID spelled `harness-default`.

### Model strength

`--model-size normal|large|xlarge` selects acsc's relative model strength within
the final harness. Size and effort are independent. `--model-size`, `--model`,
and `--model-id` override each other in command-line order, including across
shell aliases. Selecting a size keeps an explicit effort.

The bundled mappings, checked on 2026-10-21, are:

| Harness | normal | large | xlarge |
|---|---|---|---|
| Antigravity | `gemini-3.8-flash` | `gemini-4-argon` | `gemini-4-argon` |
| Codex | `gpt-5.6-terra` | `gpt-6.1-sol` | `gpt-6-astra` |
| Claude | `claude-sonnet-5-5` | `claude-opus-5-5` | `claude-fable-5-1` |

"Latest" means the version baked into this table based on the local harness
catalog and published documentation. These mappings change with acsc updates
or your config; launch and completion never select a newer version from a
cache or network lookup. The current IDs were checked against the local
`agy models` output, the local Codex catalog,
[official OpenAI model docs](https://developers.openai.com/api/docs/models),
[Terra's model page](https://developers.openai.com/api/docs/models/gpt-5.6-terra),
[Antigravity model identifiers](https://www.antigravity.google/docs/enterprise/#supported-models-in-gemini-enterprise),
and [Claude Code's model documentation](https://code.claude.com/docs/en/model-config).

Set a default size globally or for one harness:

```toml
[defaults]
model_size = "normal"

[harnesses.claude]
model_size = "xlarge"

[harnesses.codex.model_sizes]
large = "gpt-6.1-sol"
```

Mapping values are literal native model IDs; user entries override individual
sizes. A harness-level `model` or `model_size` takes precedence over the global
`model_size`; the two are mutually exclusive within a harness table. Explicit
CLI model/size selections take precedence over both. With no size selection,
the existing harness model defaults apply.

Copilot has no bundled size mapping yet. Selecting an unconfigured size reports
an error naming the config entry to add. `acsc --models` shows the effective size
labels alongside their target IDs, and `--dry-run` shows the selected size, its
source, and the source of the mapped model ID. `acsc --config init` includes the
commented mappings.

### Effort and model metadata

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

Global defaults accept `harness`, `mode`, `model_size`, `skill`,
`effort`/`raw_effort`, `yes`, `push`, and `fast`. Harness sections accept those
applicable selections plus `model`, `model_sizes`, `command`, `base_args`,
`effort_map`, `native_settings`, and `models`.
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

`acsc --models` groups presets and suggestions by harness. `--harness` narrows
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
source <(acsc --completion bash)
source <(acsc --completion zsh)

# Fish (config.fish)
acsc --completion fish | source
```

```powershell
# PowerShell ($PROFILE)
acsc --completion powershell | Out-String | Invoke-Expression
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
