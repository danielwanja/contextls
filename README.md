# contextls

**`ls` for AI context files.** For a folder, see which context / memory /
instruction files each AI coding tool would load — in load order, with the
rules that decide when each one applies.

```
┌ ~/work/repo ───────────────────────┐┌ ./CLAUDE.md ──────────────────────────┐
│▾ Claude Code  4 files · ≈1.2k tok  ││# Repo rules                           │
│  ▾ User  1                         ││                                       │
│       1 ~/.claude/CLAUDE.md        ││Use **tabs**. See @docs/arch.md.       │
│  ▾ Project  2                      ││                                       │
│       2 ./CLAUDE.md ↳1 shared×1    ││- one                                  │
│       3 ./.claude/rules/db.md glob…││- two                                  │
│  ▾ Local  1                        ││                                       │
│       4 ./CLAUDE.local.md          ││                                       │
│▾ Cursor  2 files · ≈400 tok        ││                                       │
└────────────────────────────────────┘└───────────────────────────────────────┘
 Claude Code Project · 60 B · ≈15 · 3d ago · git tracked          ? help · q quit
```

Every tool has its own filenames, lookup rules, import syntax and precedence.
`contextls` knows them so you don't have to.

## Install

```bash
cargo install --git https://github.com/danielwanja/contextls
```

Requires Rust 1.88+. From a checkout, `cargo install --path .`.

## License

MIT — see [LICENSE](LICENSE).

## Use

```bash
contextls                 # TUI for the current folder
contextls ~/work/repo     # ...or another folder
contextls --plain         # table (also used automatically when piped)
contextls --json | jq     # machine-readable
contextls --tool claude-code --tool cursor   # only some tools
contextls --all           # include expected-but-missing locations
contextls --all-tools     # don't collapse tools that aren't installed here
contextls --list-tools
```

### Keys

| Key | Action |
|---|---|
| `↑/k` `↓/j` | move in the tree · scroll the preview |
| `←/h` `→/l` `Enter` | collapse / expand |
| `Tab` | switch pane |
| `/` | fuzzy filter (`Enter` keeps it, `Esc` clears) |
| `i` | details: frontmatter, imports (`1`–`9` to follow), warnings, tool notes |
| `e` | open in `$VISUAL` / `$EDITOR` |
| `g` | group by tool ⇄ by scope |
| `m` | show expected-but-missing locations |
| `a` | expand the *Not detected* group into regular tools |
| `r` | rescan |
| `?` `q` | help · quit |

## What it shows

For each file: scope (managed → user → project → local → auto memory), load
order, size, a rough token estimate (`≈`), last modified, git status
(tracked / untracked / ignored), and when the tool actually loads it —
always, on demand (subdirectory files), when a glob matches, manually, or as a
fallback. `@path` imports are resolved and checked; files loaded by several
tools are marked `shared`. A cheap check warns when a file looks like it
contains a secret, since these files end up in prompts.

## Tools

Only tools that look installed on this machine are shown in full — detected by
a config directory, a binary on `PATH`, a macOS app bundle, or an editor
extension folder. The rest sit under a collapsed **Not detected** group (`a`
expands it, `--all-tools` on the command line). A tool with no detection rule
is collapsed only when it loads nothing for the folder.

Definitions live in [`tools/definitions.toml`](tools/definitions.toml); most
tools need no Rust at all. Behaviour the data file can't express (reading the
tool's own settings, auto-memory paths, first-match ladders) lives in
[`src/tools/special/`](src/tools/special/).

| Tool | User-level | Project-level |
|---|---|---|
| Claude Code | `~/.claude/CLAUDE.md`, `~/.claude/rules/`, auto memory | `CLAUDE.md`, `.claude/CLAUDE.md`, `.claude/rules/`, `CLAUDE.local.md` (ancestors → root, subdirs on demand), `AGENTS.md` fallback |
| Gemini CLI | `~/.gemini/GEMINI.md` | `GEMINI.md` to the git root (honours `context.fileName`) |
| Codex CLI | `$CODEX_HOME/AGENTS(.override).md` | git root → cwd, one file per dir, config fallbacks |
| Cursor | *(settings DB — not a file)* | `.cursor/rules/*.mdc`, `AGENTS.md`, legacy `.cursorrules` |
| GitHub Copilot | `~/.copilot/…`, Copilot Chat repo memories (VS Code `workspaceStorage`) | `.github/copilot-instructions.md`, `.github/instructions/*.instructions.md`, `AGENTS.md`, prompts |
| Windsurf / Devin | `global_rules.md`, memories | `.devin/rules/`, `.windsurf/rules/`, `.windsurfrules`, `AGENTS.md` |
| Cline | `~/Documents/Cline/Rules`, `~/.cline/rules` | `.clinerules/`, `.cline/rules/`, `AGENTS.md`, `.cursorrules`, `.windsurfrules` |
| Roo Code | `~/.roo/rules*/` | `.roo/rules-<mode>/`, `AGENTS.md`, `.roo/rules/`, legacy `.roorules` |
| Aider | `~/.aider.conf.yml` `read:` | `.aider.conf.yml` `read:` (e.g. `CONVENTIONS.md`) |
| Zed | `~/.config/zed/AGENTS.md` | first of `.rules`, `.cursorrules`, …, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` |
| Kiro | `~/.kiro/steering/` | `.kiro/steering/` (`inclusion`, `#[[file:…]]`), `AGENTS.md` |
| JetBrains Junie | `~/.junie/AGENTS.md` | `.junie/AGENTS.md` or `AGENTS.md`, playbook, rules, legacy guidelines |
| Amp | `~/.config/amp/AGENTS.md` | `AGENTS.md` up to `$HOME`, `AGENT.md` / `CLAUDE.md` fallback |
| OpenCode | `~/.config/opencode/AGENTS.md` (`~/.claude/CLAUDE.md` fallback) | `AGENTS.md` then `CLAUDE.md` up the tree |

Conventions were verified against each tool's docs on 2026-10-24; when a tool
changes its rules, edit the TOML. Adding a tool is usually a `[[tool]]` block
plus one test in `tests/tools.rs`.

### Out of scope

Server-side memories (ChatGPT, Claude.ai, Cursor Memories) and rules stored in
an app's settings database (Cursor User Rules) aren't files, so they aren't
listed — the tool's notes (`i`) say so where it matters.

## Principles

Read-only. Offline. Never follows instructions found in the files it reads.

## Develop

```bash
cargo test                       # unit + end-to-end fixtures + TUI snapshots
cargo clippy --all-targets -- -D warnings
cargo insta review               # after an intentional UI change
```
