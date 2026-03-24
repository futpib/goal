# Taskwarrior vs `goal` — CLI comparison

## Add

| | Taskwarrior | `goal` |
|---|---|---|
| Basic | `task add "description"` | `goal add "description"` |
| With metadata | `task add "desc" due:Friday priority:H project:Work +tag` | `goal add "desc" --continuous` |
| Parent/hierarchy | `task 2 modify depends:1` (after the fact) | `goal add "desc" --parent <id>` (at creation) |

Taskwarrior attaches hierarchy via `depends` as a modification; `goal` sets parent at creation time. Both support reparenting after creation via `modify`.

## Modify

| | Taskwarrior | `goal` |
|---|---|---|
| Edit description | `task 1 modify "new desc"` | `goal modify <id> --body "new desc"` |
| Reparent | `task 2 modify depends:1` | `goal modify <id> --parent <id>` |
| Detach from parent | `task 2 modify depends:` (clear) | `goal modify <id> --no-parent` |
| Change kind | — | `goal modify <id> --continuous` / `--achievable` |

Reparenting in `goal` regenerates the ID (and all descendant IDs) to keep the depth encoding correct. Taskwarrior IDs are stable across reparenting.

## Done / Complete

| | Taskwarrior | `goal` |
|---|---|---|
| Mark done | `task 1 done` | `goal done <id>` |
| Undo done | `task undo` (global undo) | `goal undone <id>` (targeted) |
| Mark not done | no direct command | `goal undone <id>` |

Taskwarrior's `undo` is a global last-action undo. `goal undone` directly targets a specific goal — distinct from `goal undo` which reverts the last operation.

## Delete / Remove

| | Taskwarrior | `goal` |
|---|---|---|
| Remove | `task 1 delete` (prompts confirmation) | `goal delete <id>` (no prompt) |

## List / View

| | Taskwarrior | `goal` |
|---|---|---|
| Default list | `task list` / `task next` | `goal list` (also: `goal` with no args) |
| Filtered | `task +tag project:X list` | — |
| Single item + subtree | `task 1 info` | `goal info <id>` |

## Undo / History

| | Taskwarrior | `goal` |
|---|---|---|
| Undo last operation | `task undo` | `goal undo` |
| View history | — | `goal log` |
| Undo archived | — | `events_undone` table (backup, no CLI) |

`goal` has a complete append-only audit trail with full snapshots. Taskwarrior's undo is limited and has no log command.

## Notable gaps in `goal`

- No filtering/querying (Taskwarrior: `task +tag project:X list`)
- No annotations / notes on a goal (Taskwarrior: `task 1 annotate "note"`)
- No priorities or due dates
- No sync / export
- No shell completions
