# Rewind for AI coding agents

AI agents edit many files quickly. Rewind records the folder independently of the agent and of git, so you can see what happened
and undo it. Three ways to use it, from simplest to most automatic.

## 1. Wrap the command

```bash
cd ~/code/shop
rewind run -- claude "refactor the auth module"       # any command works
rewind run --folder ~/code/shop --name aider -- aider --yes
```

What happens: Rewind saves a labelled checkpoint, records every change while the command runs (your Ctrl+C goes to the command, the
report still prints), then prints a report and the exact undo command. Exit status is the command's.

```bash
rewind agent list
rewind agent report            # latest run, or an id
rewind agent undo              # dry run: shows what would change
rewind agent undo --apply --remove-new
```

`--remove-new` also deletes files the run created (their content stays in history, and a label is saved before every undo, so you can
undo the undo with `rewind rewind-folder`).

### What the report warns about
- 5 or more files deleted
- files that look like secrets were touched (`.env`, `*.pem`, `*.key`, `id_rsa*`, `credentials*`, `secrets*`, `.npmrc`, ...)
- a file lost at least 50 lines and more than 3x what it gained
- 100 or more files touched

## 2. MCP server (the agent protects itself)

```bash
claude mcp add rewind -- rewind mcp            # Claude Code
```

Cursor / other clients, in their MCP config:

```json
{ "mcpServers": { "rewind": { "command": "rewind", "args": ["mcp"] } } }
```

Tools: `rewind_begin_session`, `rewind_session_report`, `rewind_undo_session` (dry run unless `confirm=true`), `rewind_checkpoint`,
`rewind_file_history`, `rewind_restore_file`, `rewind_deleted_files`.

Tell the agent when to use them, for example in `CLAUDE.md` / `AGENTS.md`:

```markdown
Before editing a project, call rewind_begin_session for the project folder.
Before a risky refactor, mass rename or delete, call rewind_checkpoint.
When you finish, call rewind_session_report and mention anything it flags.
```

The MCP process also watches the folder, so changes are recorded even if the Rewind app is closed.

## 3. Hooks (a checkpoint before every tool call)

`rewind checkpoint --every 120` saves a checkpoint of the current folder unless one was saved in the last 120 seconds, so it is cheap
enough to run before every tool call. For Claude Code, in `.claude/settings.json` (check the current hooks documentation for the exact
format of your version):

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Write|Edit|MultiEdit|Bash",
        "hooks": [ { "type": "command", "command": "rewind checkpoint --every 120 --name claude-code" } ] }
    ]
  }
}
```

Then recover with `rewind label <folder>` (list checkpoints) and `rewind rewind-folder <folder> "2026-11-21 14:05" --apply`.

## In the app
**Tools > AI agents**: pick a folder, *Checkpoint and start*, later *What changed* and *Undo...*. Runs started from the CLI or MCP show up there too.

## Limits (read this)
- Only files inside the protected folder, and only files under 25 MB, outside ignored folders (`.git`, `node_modules`, `target`, ...).
- Not undoable: database writes, installed packages, `git push`, anything outside the folder.
- The tripwire uses 4x higher thresholds while a run is active in that folder, so a big refactor does not look like ransomware.
- Rewind is not a sandbox. It cannot stop an agent, it lets you see and reverse what it did.
