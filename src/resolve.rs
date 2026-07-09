use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use chrono::{Days, Utc};

use crate::config::Ctx;
use crate::models::{Project, Task, TimeEntry};
use crate::time::{fmt_duration, fmt_local_date};

pub(crate) fn looks_like_id(s: &str) -> bool {
    s.len() == 24 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// How far back suffix references are searched.
const SUFFIX_LOOKBACK_DAYS: u64 = 90;
const SUFFIX_LOOKBACK_LIMIT: usize = 1000;

/// The minimum suffix length accepted as an entry reference — a single
/// character works when it's unique.
pub const MIN_SUFFIX: usize = 1;

/// The entries suffix references are resolved against (last 90 days).
pub fn lookback_entries(ctx: &Ctx) -> Result<Vec<TimeEntry>> {
    let end = Utc::now();
    let start = end
        .checked_sub_days(Days::new(SUFFIX_LOOKBACK_DAYS))
        .unwrap_or(end);
    ctx.client.time_entries(
        &ctx.workspace_id,
        &ctx.user_id,
        start,
        end,
        Some(SUFFIX_LOOKBACK_LIMIT),
    )
}

fn common_suffix_len(a: &str, b: &str) -> usize {
    a.bytes()
        .rev()
        .zip(b.bytes().rev())
        .take_while(|(x, y)| x == y)
        .count()
}

/// For each id, the length of the shortest suffix that uniquely identifies it
/// within `ids` (at least MIN_SUFFIX characters).
pub fn unique_suffix_lens<'a>(ids: impl IntoIterator<Item = &'a str>) -> HashMap<String, usize> {
    let ids: Vec<&str> = ids.into_iter().collect();
    ids.iter()
        .map(|id| {
            let longest_shared = ids
                .iter()
                .filter(|other| **other != *id)
                .map(|other| common_suffix_len(id, other))
                .max()
                .unwrap_or(0);
            let len = (longest_shared + 1).clamp(MIN_SUFFIX, id.len());
            (id.to_string(), len)
        })
        .collect()
}

/// Find a time entry by full id, by a unique suffix of its id
/// (searched among the entries of the last 90 days), or by `@`
/// for the currently running timer.
pub fn entry(ctx: &Ctx, reference: &str) -> Result<TimeEntry> {
    let needle = reference.trim().to_lowercase();
    if needle == "@" {
        return ctx
            .client
            .running_entry(&ctx.workspace_id, &ctx.user_id)?
            .context("no timer is running");
    }
    if looks_like_id(&needle) {
        return ctx.client.time_entry(&ctx.workspace_id, &needle);
    }
    if needle.is_empty() || !needle.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "'{reference}' is not an entry id — use the full 24-character id, a hex id suffix \
             (see `clockify log`), or '@' for the running timer"
        );
    }

    let entries = lookback_entries(ctx)?;
    let matches: Vec<TimeEntry> = entries
        .into_iter()
        .filter(|e| e.id.ends_with(&needle))
        .collect();
    match matches.len() {
        1 => Ok(matches.into_iter().next().unwrap()),
        0 => bail!(
            "no entry in the last {SUFFIX_LOOKBACK_DAYS} days has an id ending in '{needle}' — \
             check `clockify log` or use the full id"
        ),
        _ => {
            let candidates: Vec<String> = matches
                .iter()
                .map(|e| {
                    format!(
                        "  {}  {}  {}  {}",
                        e.id,
                        fmt_local_date(e.time_interval.start),
                        fmt_duration(e.duration()),
                        if e.description.is_empty() {
                            "(no description)"
                        } else {
                            &e.description
                        }
                    )
                })
                .collect();
            bail!(
                "'{needle}' matches several entries:\n{}",
                candidates.join("\n")
            )
        }
    }
}

/// Find an item by full id, exact name, unique name substring, or unique id
/// suffix. Exact matches win outright; the fuzzy tier considers name
/// substrings and id suffixes together, so a suffix highlighted in list
/// output can never silently lose to a name that happens to contain it —
/// such collisions error out with the candidates instead.
pub(crate) fn pick<'a, T>(
    kind: &str,
    list_cmd: &str,
    needle: &str,
    items: &'a [T],
    id: impl Fn(&T) -> &str,
    name: impl Fn(&T) -> &str,
) -> Result<&'a T> {
    let needle = needle.trim();
    if needle.is_empty() {
        bail!("{kind} reference is empty");
    }
    let lower = needle.to_lowercase();
    if looks_like_id(&lower)
        && let Some(item) = items.iter().find(|i| id(i) == lower)
    {
        return Ok(item);
    }
    if let Some(item) = items.iter().find(|i| name(i).to_lowercase() == lower) {
        return Ok(item);
    }
    let is_hex = lower.chars().all(|c| c.is_ascii_hexdigit());
    let matches: Vec<&T> = items
        .iter()
        .filter(|i| name(i).to_lowercase().contains(&lower) || (is_hex && id(i).ends_with(&lower)))
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => {
            bail!("no {kind} matches '{needle}' — run `{list_cmd}` to see what's available")
        }
        many => {
            let candidates: Vec<String> = many
                .iter()
                .map(|i| {
                    let id = id(i);
                    format!("  {}  …{}", name(i), &id[id.len().saturating_sub(6)..])
                })
                .collect();
            bail!(
                "'{needle}' is ambiguous — matching {kind}s:\n{}",
                candidates.join("\n")
            )
        }
    }
}

pub fn project(ctx: &Ctx, needle: &str) -> Result<Project> {
    let projects = ctx.client.projects(&ctx.workspace_id)?;
    let active: Vec<Project> = projects.into_iter().filter(|p| !p.archived).collect();
    pick(
        "project",
        "clockify projects",
        needle,
        &active,
        |p| &p.id,
        |p| &p.name,
    )
    .cloned()
}

/// The configured default project of the current workspace, if any.
/// Errors when the config points at a project that no longer exists.
pub fn default_project(ctx: &Ctx) -> Result<Option<Project>> {
    let Some(default) = &ctx.default_project else {
        return Ok(None);
    };
    let projects = ctx.client.projects(&ctx.workspace_id)?;
    match projects.into_iter().find(|p| p.id == default.id) {
        Some(p) if !p.archived => Ok(Some(p)),
        Some(_) => bail!(
            "the default project '{}' is archived — pick another with `clockify projects default` \
             or pass --project / --no-project",
            default.name
        ),
        None => bail!(
            "the default project '{}' no longer exists — run `clockify projects default --clear`",
            default.name
        ),
    }
}

pub fn task(ctx: &Ctx, project_id: &str, needle: &str) -> Result<Task> {
    let tasks = ctx.client.tasks(&ctx.workspace_id, project_id)?;
    pick(
        "task",
        "clockify tasks <project>",
        needle,
        &tasks,
        |t| &t.id,
        |t| &t.name,
    )
    .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_suffix_lengths() {
        assert_eq!(common_suffix_len("abc123", "xyz123"), 3);
        assert_eq!(common_suffix_len("abc", "abc"), 3);
        assert_eq!(common_suffix_len("abc", "xyz"), 0);
    }

    fn item(id: &str, name: &str) -> (String, String) {
        (id.to_string(), name.to_string())
    }

    fn pick_item<'a>(items: &'a [(String, String)], needle: &str) -> Result<&'a (String, String)> {
        pick(
            "thing",
            "clockify things",
            needle,
            items,
            |t| &t.0,
            |t| &t.1,
        )
    }

    #[test]
    fn pick_resolves_unique_id_suffix() {
        let items = vec![
            item("aaaaaaaaaaaaaaaaaaaaaaa1", "Writing"),
            item("aaaaaaaaaaaaaaaaaaaaaaa2", "Design"),
        ];
        assert_eq!(pick_item(&items, "1").unwrap().1, "Writing");
        assert_eq!(pick_item(&items, "aa2").unwrap().1, "Design");
    }

    #[test]
    fn pick_exact_name_beats_id_suffix() {
        let items = vec![
            item("aaaaaaaaaaaaaaaaaaaa2024", "Writing"),
            item("aaaaaaaaaaaaaaaaaaaaaaa1", "2024"),
        ];
        assert_eq!(pick_item(&items, "2024").unwrap().1, "2024");
    }

    #[test]
    fn pick_full_id_beats_name_substring() {
        let full = "aaaaaaaaaaaaaaaaaaaaaaa1";
        let items = vec![
            item(full, "Writing"),
            item("bbbbbbbbbbbbbbbbbbbbbbb2", full), // name contains the other id
        ];
        assert_eq!(pick_item(&items, full).unwrap().1, "Writing");
    }

    #[test]
    fn pick_name_substring_vs_suffix_collision_is_ambiguous() {
        let items = vec![
            item("aaaaaaaaaaaaaaaaaaaaaaa1", "Lab work"),
            item("aaaaaaaaaaaaaaaaaaaaaaab", "Design"),
        ];
        // "ab" is a substring of "Lab work" and a suffix of the other id.
        let err = pick_item(&items, "ab").unwrap_err().to_string();
        assert!(err.contains("Lab work") && err.contains("Design"), "{err}");
    }

    #[test]
    fn pick_agreeing_name_and_suffix_resolve() {
        // "ab" matches the same item by name substring and id suffix.
        let items = vec![
            item("aaaaaaaaaaaaaaaaaaaaaaab", "Lab work"),
            item("aaaaaaaaaaaaaaaaaaaaaaa1", "Design"),
        ];
        assert_eq!(pick_item(&items, "ab").unwrap().1, "Lab work");
    }

    #[test]
    fn pick_ambiguous_suffix_errors() {
        let items = vec![
            item("aaaaaaaaaaaaaaaaaaaaaa19", "Writing"),
            item("aaaaaaaaaaaaaaaaaaaaaa29", "Design"),
        ];
        assert!(pick_item(&items, "9").is_err());
    }

    #[test]
    fn pick_rejects_empty_and_unknown_references() {
        let items = vec![item("aaaaaaaaaaaaaaaaaaaaaaa1", "Writing")];
        assert!(pick_item(&items, "").is_err());
        assert!(pick_item(&items, "   ").is_err());
        assert!(pick_item(&items, "zzz").is_err());
    }

    #[test]
    fn unique_suffixes_are_minimal() {
        // Last characters differ -> a single character is enough.
        let lens = unique_suffix_lens(["aaaa1a2b", "aaaa3c4d"]);
        assert_eq!(lens["aaaa1a2b"], 1);
        assert_eq!(lens["aaaa3c4d"], 1);
        // Shared 4-char suffix "9999" -> need 5 chars to disambiguate.
        let lens = unique_suffix_lens(["aaaa19999", "aaaa29999", "bbbb00000"]);
        assert_eq!(lens["aaaa19999"], 5);
        assert_eq!(lens["aaaa29999"], 5);
        assert_eq!(lens["bbbb00000"], 1);
        // A single id needs only one character.
        let lens = unique_suffix_lens(["deadbeef"]);
        assert_eq!(lens["deadbeef"], 1);
    }
}
