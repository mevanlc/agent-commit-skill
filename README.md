# agent-commit-skill

Git commit helper for AI coding CLIs (Codex CLI, Claude Code, GitHub Copilot CLI, and Antigravity).

This repo provides:
- Codex, Copilot, and Antigravity skills (`commit`, `gdf-commit`) and Claude slash commands (`/commit`, `/gdf-commit`)
- A shared bash helper, `commit-tool/commit-tool.sh`, that prints a structured **Commit Review** workflow and normally requires explicit user confirmation before committing
- Optional preflight hooks (for example, identity checks)
- [acsc](acsc/README.md), a Rust CLI that unifies model, effort, and commit-mode selection across the four agent harnesses

## Install

The installers:
- Symlink `~/.local/share/agent-commit-skill` to this repo (or fail if that path exists as a non-symlink directory)
- Seed defaults into `~/.config/agent-commit-skill/` (never overwrites existing config)
- Symlink command artifacts into:
  - Codex CLI: `~/.codex/skills/`
  - Claude Code: `~/.claude/commands/`
  - GitHub Copilot CLI: `~/.copilot/skills/`
  - Antigravity CLI: `~/.gemini/antigravity-cli/skills/`
  - Agent Skills (`.agents`): `~/.agents/skills/`

### macOS/Linux (bash)

```bash
./install.sh --codex --claude --copilot --agy --dotagents --hooks
./install.sh --dotagents --check
```

### Windows (PowerShell)

```powershell
./install.ps1 -Codex -Claude -Copilot -Agy -DotAgents -Hooks
./install.ps1 -DotAgents -Check
```

Notes:
- The commit helper is a bash script; on Windows this workflow is intended to run via Git Bash.
- Creating symlinks on Windows may require Developer Mode or elevated privileges.

## Usage

After installation, use these in your agent CLI:

Codex:

```text
$commit --staged [additional instructions...]
$commit --staged-yes [additional instructions...]
$commit --all [additional instructions...]
$commit --all-yes [additional instructions...]
$commit --ask [additional instructions...]
```

If you use a `gdf` git wrapper and want *all* git operations to go through it:

```text
$gdf-commit --staged|--staged-yes|--all|--all-yes|--ask [additional instructions...]
```

Claude:

```text
/commit --staged [additional instructions...]
/commit --staged-yes [additional instructions...]
/commit --all [additional instructions...]
/commit --all-yes [additional instructions...]
/commit --ask [additional instructions...]
/gdf-commit --staged|--staged-yes|--all|--all-yes|--ask [additional instructions...]
```

GitHub Copilot CLI:

```text
/commit --staged [additional instructions...]
/commit --staged-yes [additional instructions...]
/commit --all [additional instructions...]
/commit --all-yes [additional instructions...]
/commit --ask [additional instructions...]
/gdf-commit --staged|--staged-yes|--all|--all-yes|--ask [additional instructions...]
```

The `-yes` modes still present the Commit Review, but treat its `y/n` gate as
preanswered `y` and proceed without waiting for another reply. `--ask` always
remains interactive. It summarizes staged changes, unstaged changes, and
untracked files separately; recommends whether mixed buckets warrant split
commits; and checks untracked files against the repository's ignore rules before
staging.

Any mode also takes a `-push` suffix — `--staged-push`, `--staged-yes-push`,
`--all-push`, `--all-yes-push`, `--ask-push` — which asks for a push once the
commit lands. The push happens only after a successful commit, covers the
current branch only, stops to ask before creating an upstream for a branch that
has none, and never rewrites history to force a rejected push through.

Every path that can reach a commit ends with a **Final Report** section telling
the agent how to close out: first every change it made to the repository beyond
the staging, commit, and push the mode authorizes (including edits made to
satisfy a pre-commit hook) — unconditionally, even when small and successful —
then any substantial issues even if the outcome was green, then exactly one
outcome line. The outcome vocabulary is `No commit was attempted: {reason}.`,
`Commit failed: {reason}.`, and `Committed successfully.`; the `-push` modes
swap the last for `Committed and pushed successfully.`, `Committed
successfully, push not attempted: {reason}.`, or `Committed successfully, push
failed:`.

For large diffs, the helper may write the diff to a temp file under `/tmp/` to avoid CLI output truncation and will instruct you to delete it after review.

## Configuration

Config location (default): `~/.config/agent-commit-skill/`

- `commit-tool.config`
  - `report_recent_commits=5` (set `0` to disable)
- Hooks (installed with `--hooks` / `-Hooks`): `~/.config/agent-commit-skill/hooks/`
  - `hook-preflight-01-id-check.sh` and `hook-preflight-01-id-check.config`

Override the config directory by setting `AGENT_COMMIT_CONFIG_DIR`.

## Development

Build and validate acsc separately with `cargo build --manifest-path acsc/Cargo.toml`
and `cargo nextest run --manifest-path acsc/Cargo.toml`. See its
[usage and configuration guide](acsc/README.md) for installation, aliases, and
dynamic completion.

Run tests (requires bash + git):

```bash
bash test/run_all.sh
```

Each case renders the helper against a generated repo and diffs the result
against a blessed `expected.txt`. `bash test/regen_expecteds.sh [case...]`
re-blesses them — **read the diff before committing a regen**, since the golden
is the contract the agent actually receives and a regen will just as happily
enshrine a defect as fix one.

`test/lint_goldens.sh` (also run by `run_all.sh`) asserts what case diffs
structurally cannot: that a defect already blessed into a golden gets caught.
It checks fence balance, headings not glued to preceding content, sequential
instruction numbering, no duplicate headings, and no `wc`-padded counts.
