---
name: gdf-commit
description: Git commit helper. Use only when explicitly invoked. Do not consult memories.
---

You do not need to announce that you are about to use the skill, just use it.
You do not need to probe for the existence of the commit- script prior to
calling it, just call it. Report after-the-fact failures, if any.


# Gdf Commit

Use this skill to invoke `~/.local/share/agent-commit-skill/commit-tool/commit-tool.sh` with the `gdf` base command. Parse the user's latest request to extract one required mode flag, plus any extra commit instructions; skills do not receive a separate `$ARGUMENTS` placeholder.

## gdf Requirement

- Use `gdf` for all git operations in this workflow.
- Do not fall back to raw `git` without first discussing it with the user.
- If `gdf` is unavailable or behaves unexpectedly, stop immediately and report the exact issue.

## Required Mode

Accept exactly one of:

- `--staged`
- `--staged-yes`
- `--all`
- `--all-yes`
- `--ask`
- `--ask-no`

Every mode except `--ask-no` also accepts a `-push` suffix: `--staged-push`,
`--staged-yes-push`, `--all-push`, `--all-yes-push`, `--ask-push`.

If no mode is present in the user's request, ask the user to provide one.

## Command

Run:

```sh
~/.local/share/agent-commit-skill/commit-tool/commit-tool.sh gdf "<derived args>"
```

Build `<derived args>` from the actual user request. Include the chosen mode first, then any additional freeform instructions.

## Preconfirmed Modes

`--staged-yes` and `--all-yes` allow the user to make a quicker commit by
preanswering `y` to the `y/n` commit gate. Still present the Commit Review, then
commit without waiting for the user to reply.

Because the user will not have an opportunity to decline the commit, be
slightly more wary of unusual files and similar concerns. If you find such a
concern, drop out of preconfirmed mode and resolve it interactively with the
user. Once dropped, do not re-enter preconfirmed mode; use the ordinary `y/n`
commit gate before committing. Preconfirmed mode is engaged only when the user
selects `--staged-yes` or `--all-yes` at skill-invocation time.

If a pre-commit hook rejects the commit for a mechanical, low-risk issue, you
may fix it and retry while preconfirmed mode remains engaged. For a more
substantial pre-commit hook issue, drop out of preconfirmed mode and involve the
user interactively.

## Push Modes

A `-push` suffix is the user's clear and affirmative instruction to push after
the commit lands, and the helper output spells out the rules. In short: push
only once the commit has actually succeeded, push the current branch only, stop
and ask before creating an upstream for a branch that has none, and never
force-push, reset, rebase, or amend to force a rejected push through.

## Workflow

1. Verify the helper path exists. If it does not, tell the user to install or upgrade the tool.
2. Derive the argument string from the latest user request.
3. Run the helper script.
4. Follow the helper output exactly.
5. If it says `STOP`, stop and help the user resolve the issue first.
6. Present the requested Commit Review. In ordinary modes, wait for explicit user confirmation before running any commit command; in a preconfirmed mode, proceed as instructed without waiting.
7. Once confirmed or while preconfirmed mode remains engaged, perform the commit exactly as instructed.
8. Close out with the report the helper's Final Report section describes: every change you made to the repository beyond the staging, commit, and push the mode authorizes (including edits made to satisfy a pre-commit hook), then any substantial issues even when the outcome is green, then one outcome line. Do not report the commit hash unless the user asks for it.
9. If the helper requests heredoc-based commit input, put only the commit message in the heredoc. Do not put follow-up commands inside the heredoc.
10. Do not bypass git hooks without explicit user permission.
11. Do not run tests without explicit user permission (hooks may run them, though).

## Windows

On Windows, run the helper through Git for Windows Bash, even when the current
shell is PowerShell. Do not invoke a bare `bash`: it may select WSL, where `~`
resolves to a Linux home directory instead of the Windows profile containing
the installed helper.

From PowerShell, locate and invoke Git for Windows Bash explicitly:

```powershell
$gitExe = (Get-Command git -ErrorAction Stop).Source
$gitRoot = Split-Path (Split-Path $gitExe -Parent) -Parent
$gitBash = Join-Path $gitRoot 'bin\bash.exe'
if (-not (Test-Path -LiteralPath $gitBash -PathType Leaf)) {
    throw "Git for Windows Bash was not found at $gitBash"
}
& $gitBash -lc '~/.local/share/agent-commit-skill/commit-tool/commit-tool.sh gdf "<derived args>"'
```

Replace `<derived args>` before invoking the command, keeping the complete
derived string inside the shown double quotes so the helper receives it as one
argument. Use the same explicit `$gitBash` executable for any later Bash
commands required by the helper. If Git for Windows Bash is unavailable, stop
and inform the user.
