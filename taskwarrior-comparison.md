# Taskwarrior vs `goal` — CLI comparison

## Add

| | Taskwarrior | `goal` |
|---|---|---|
| Basic | `task add "description"` | `goal add "description"` |
| With metadata | `task add "desc" due:Friday priority:H project:Work +tag` | `goal add "desc" --continuous +tag` |
| Parent/hierarchy | `task 2 modify depends:1` (after the fact) | `goal add "desc" --parent <id>` (at creation) |

Taskwarrior attaches hierarchy via `depends` as a modification; `goal` sets parent at creation time. Both support reparenting after creation via `modify`.

## Modify

| | Taskwarrior | `goal` |
|---|---|---|
| Edit description | `task 1 modify "new desc"` | `goal modify <id> --body "new desc"` |
| Reparent | `task 2 modify depends:1` | `goal modify <id> --parent <id>` |
| Detach from parent | `task 2 modify depends:` (clear) | `goal modify <id> --no-parent` |
| Change kind | — | `goal modify <id> --continuous` / `--achievable` |
| Add/remove tags | `task 1 modify +tag -tag` | `goal modify <id> +tag -tag` |

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
| Remove | `task 1 delete` (prompts confirmation) | `goal delete <id>` (prompts confirmation; `--yes` / `-y` to skip) |

## List / View

| | Taskwarrior | `goal` |
|---|---|---|
| Default list | `task list` / `task next` | `goal list` (also: `goal` with no args) |
| Filtered | `task +tag project:X list` | — |
| Single item + subtree | `task 1 info` | `goal info <id>` |

## Annotations

| | Taskwarrior | `goal` |
|---|---|---|
| Add note | `task 1 annotate "note"` | `goal annotate <id> "note"` |
| Edit note | `task 1 denotate` + re-annotate | `goal annotate <id> --edit <ann-id> "new text"` |
| Delete note | `task 1 denotate "note"` | `goal annotate <id> --delete <ann-id>` |
| View notes | shown in `task 1 info` | shown in `goal info <id>` |

Annotation IDs in `goal` are short random strings (prefix-resolvable like goal IDs).

## Undo / History

| | Taskwarrior | `goal` |
|---|---|---|
| Undo last operation | `task undo` | `goal undo` |
| View history | — | `goal log` |
| Undo archived | — | `events_undone` table (backup, no CLI) |

`goal` has a complete append-only audit trail with full snapshots. Taskwarrior's undo is limited and has no log command.

## Tags

| | Taskwarrior | `goal` |
|---|---|---|
| Add tag at creation | `task add "desc" +tag` | `goal add "desc" +tag` |
| Add tag via modify | `task 1 modify +tag` | `goal modify <id> +tag` |
| Remove tag via modify | `task 1 modify -tag` | `goal modify <id> -tag` |
| View tags (list) | shown in task list | shown inline after body in `goal list` |
| View tags (detail) | shown in `task 1 info` | shown in `goal info <id>` |

Both tools use the same `+tag` / `-tag` syntax. Multiple tags can be specified in a single command. Tags cascade-delete with their goal.

## Notable gaps in `goal`

- No filtering/querying (Taskwarrior: `task +tag project:X list`)

- No priorities or due dates
- No sync / export
- No shell completions
