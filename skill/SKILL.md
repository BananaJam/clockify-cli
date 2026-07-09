---
name: clockify
description: Track time and expenses with the Clockify CLI - start and stop timers, add, edit and delete time entries and expenses, view logs and reports, and submit time or expenses for approval. Use when the user asks to track time, manage expenses, start or stop a timer, log hours, check what's running, review their timesheet, or submit approvals.
---

# Clockify CLI

`clockify` is a command-line Clockify client, already configured on this machine.

## Ground rules

- Pass `--json` on every command below to get machine-readable output; without it
  you get styled human output.
- Never run bare `clockify` — that opens an interactive TUI.
- `delete`, `discard`, and expense deletion prompt for confirmation; always pass
  `-y`.
- `submit` and `expenses submit` prompt for confirmation; pass `-y` when the user
  clearly asked to submit for approval.
- If a command fails with "invalid API key" or a missing-config error, stop and
  ask the user to run `clockify auth` themselves — setup is interactive.

## Commands

```sh
clockify status --json                        # running timer, or null when idle
clockify start "description" -p <project> --json
clockify stop --json                          # stop and save the running timer
clockify discard -y --json                    # stop WITHOUT saving the time
clockify add "description" --from 09:30 --to 10:15 -p <project> --json
clockify log --today --json                   # entries (also --week, --from/--to)
clockify report --week --json                 # time per project (also --month)
clockify submit -y --json                     # submit this month's time approval
clockify submit --week -y --json              # submit this week's time approval
clockify submit --resubmit -y --json          # resubmit rejected/withdrawn time
clockify expenses --month --json              # list expenses
clockify expenses categories --json
clockify expenses add --amount 12.50 --category meals --date today -p <project> --file <path> --json
clockify expenses edit <id> --amount 14 --file <path> --json
clockify expenses delete <id> -y --json
clockify expenses submit -y --json            # submit this month's expenses
clockify edit <id> -d "text" -p <project> --from <t> --to <t> --json
clockify delete <id> -y --json
clockify projects --json
clockify tasks <project> --json               # tasks of one project (-t on start/add)
clockify workspaces --json
```

`start` automatically stops any already-running timer first.

## Entry references

- `@` always means the running timer: `clockify edit @ -p backend` moves the
  running timer to another project without restarting it.
- Otherwise use an entry `id` from any `--json` output; any unique suffix of the
  id also works (they resolve against the last 90 days of entries).

## Projects, tasks, times

- Projects and tasks match by name: case-insensitive, substring is enough.
  An ambiguous name fails listing the candidates — pick one and retry. Exact
  ids from `projects --json` / `tasks --json` always work.
- Unique id suffixes (highlighted in list output) also work for projects,
  tasks, workspaces, and expense categories, resolving against the listed set.
- Times accept `HH:MM`, `yesterday 17:00`, `YYYY-MM-DD HH:MM`, or RFC 3339,
  interpreted in the local timezone. Date flags on `log`/`report` accept
  `YYYY-MM-DD`, `today`, or `yesterday`.
- The user may have a default project: when `--project` is omitted, `start` and
  `add` fall back to it. Pass `--no-project` to force a project-less entry.

## JSON shapes

A time entry (returned by status/log/start/stop/add/edit):

```json
{"id": "…", "description": "…", "project": {"id": "…", "name": "…"},
 "start": "2026-07-04T09:00:00Z", "end": null,
 "duration_seconds": 4200, "running": true}
```

`project` and `end` may be null; `end: null` means the timer is running.
An expense (returned by expenses list/show/add/edit):

```json
{"id": "…", "date": "2026-07-04", "total": 12.5,
 "category": {"id": "…", "name": "Meals"},
 "project": {"id": "…", "name": "Backend"}, "file": {"id": "…", "name": "receipt.jpg"}}
```

`delete`/`discard` return `{"deleted": id}` / `{"discarded": id}`; `report`
returns `{"from", "to", "total_seconds", "projects": [{"id", "name",
"duration_seconds", "percent"}]}`. `submit` returns `{"id", "state", "period",
"from", "to", "entry_count", "total_seconds", "resubmitted"}`. `expenses submit`
returns `{"id", "state", "period", "from", "to", "expense_count",
"total_amount", "resubmitted"}`.

## Caveats

- Some workspaces require a project on completed entries; if `add` or `stop`
  fails with "Project is required", ask the user which project to use.
- Billability follows the project's default when an entry changes project —
  the CLI handles this; don't try to set it separately.
- Clockify submits time and expenses separately. Use `submit` for time entries
  and `expenses submit` or `submit --expenses` for expenses.
- Many workspaces require receipt files for expenses. If Clockify rejects an
  expense because a file is missing, ask the user for a receipt path and pass
  `--file <path>`.
