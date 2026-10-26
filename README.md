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

Homebrew (macOS or Linux):

```bash
brew install danielwanja/tap/contextls
```

Without Homebrew — the installer picks the right binary, verifies its
checksum and puts it in `~/.local/bin` (no `sudo`, no Rust needed):

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/danielwanja/contextls/releases/latest/download/contextls-installer.sh | sh
```

With Rust 1.88+:

```bash
cargo install --git https://github.com/danielwanja/contextls
```

Prebuilt archives for macOS (Apple Silicon, Intel) and Linux (x86-64, arm64)
are on the [Releases](https://github.com/danielwanja/contextls/releases) page.

### Releasing

Releases are built by [`cargo-dist`](https://github.com/axodotdev/cargo-dist)
in GitHub Actions ([`release.yml`](.github/workflows/release.yml)). Bump the
version in `Cargo.toml`, commit, then push a matching tag:

```bash
git tag v0.2.0 && git push origin v0.2.0
```

That publishes the GitHub Release and updates the formula in
[`danielwanja/homebrew-tap`](https://github.com/danielwanja/homebrew-tap).

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
contextls --no-watch      # don't refresh when files change
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
| `o` | reveal in Finder (or the system file manager) |
| `s` / `S` | summarize this file with an installed AI CLI (`S` redoes; `s` again shows the file) |
| `p` | pick which CLI summarizes |
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

## Live refresh

The TUI watches the folder and the directory of every file it lists (so an
agent writing to its memory folder shows up too). After a change settles for
~400 ms it re-scans in the background and swaps the tree in place — same file
selected, same scroll position — and the status bar says what changed.
Summaries of a file that changed are dropped. A full re-scan runs every minute
regardless; where a watcher can't be created, known files are stat'ed every
5 s instead. `--no-watch` turns all of it off.

## Summaries

Some memory files get long. Press `s` on one and contextls asks an AI CLI you
already have installed and are logged into — no API keys involved. The first
installed of **claude → codex → grok → gemini → copilot → amp → opencode** is
the default; `p` (or `--summarizer <id>`) picks another, and
`--list-summarizers` shows what's available.

Nothing happens until you press `s`, and only that one file is sent. The CLI
runs with its tools disabled or read-only, in an empty temporary directory,
with a two-minute timeout; the summary is kept in memory for the session only.
Expect 5–20 s and a dent in that tool's quota.

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
| Grok Build (xAI) | `~/.grok/AGENTS.md`, `~/.grok/rules/`, memory | `AGENTS.md` / `CLAUDE.md` root → cwd, `.grok/rules/` |

Conventions were verified against each tool's docs on 2026-10-24; when a tool
changes its rules, edit the TOML. Adding a tool is usually a `[[tool]]` block
plus one test in `tests/tools.rs`.

### Out of scope

Server-side memories (ChatGPT, Claude.ai, Cursor Memories) and rules stored in
an app's settings database (Cursor User Rules) aren't files, so they aren't
listed — the tool's notes (`i`) say so where it matters.

## Principles

Read-only. Offline — except the file you explicitly summarize with `s`, which
goes to the CLI you chose. Never follows instructions found in the files it
reads.

## Develop

```bash
cargo test                       # unit + end-to-end fixtures + TUI snapshots
cargo clippy --all-targets -- -D warnings
cargo insta review               # after an intentional UI change
```
