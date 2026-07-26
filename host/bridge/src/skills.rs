//! Load and serve agent skills (SKILL.md) from defaults + user save dir.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn skills_dirs() -> (PathBuf, PathBuf) {
    let defaults = env::var("OS_SKILLS_DEFAULTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/defaults"));
    let user = env::var("OS_SKILLS_USER")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::paths::app_support().join("skills"));
    (defaults, user)
}

/// Merged skill map name → description (saved overrides default by name).
fn list_skills(defaults: &Path, user: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    collect_dir(defaults, &mut map);
    collect_dir(user, &mut map);
    map
}

fn collect_dir(dir: &Path, map: &mut BTreeMap<String, String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for ent in entries.flatten() {
        let path = ent.path().join("SKILL.md");
        if !path.is_file() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            if let Some((name, desc)) = parse_frontmatter(&text) {
                map.insert(name, desc);
            }
        }
    }
}

fn parse_frontmatter(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let fm = &rest[..end];
    let mut name = None;
    let mut description = String::new();
    let mut in_desc = false;
    for line in fm.lines() {
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(v.trim().trim_matches('"').to_string());
            in_desc = false;
        } else if let Some(v) = line.strip_prefix("description:") {
            let v = v.trim();
            if v == ">-" || v == "|" {
                in_desc = true;
                description.clear();
            } else {
                description = v.trim_matches('"').to_string();
                in_desc = false;
            }
        } else if in_desc {
            let t = line.trim();
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(t);
        }
    }
    Some((name?, description))
}

pub fn save_skill(name: &str, body: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("invalid_name".into());
    }
    let (_, user) = skills_dirs();
    let dir = user.join(name);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("SKILL.md");
    if body.starts_with("---") {
        fs::write(&path, body).map_err(|e| e.to_string())?;
    } else {
        fs::write(
            &path,
            format!("---\nname: {name}\ndescription: User-saved skill.\n---\n\n{body}"),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Matches guest `skills::MAX_LISTED` (buffer / paint / home count).
const GUEST_MAX_LISTED: usize = 6;

pub fn list_response() -> Vec<String> {
    let (defaults, user) = skills_dirs();
    let skills = list_skills(&defaults, &user);
    let rows = skills.iter().take(GUEST_MAX_LISTED).map(|(name, desc)| {
        // Frontmatter is untrusted: cap to guest `skills::Slot` and ASCII so a
        // '|' / non-atlas glyph cannot inject ROW fields or paint garbage.
        let name = sanitize_slot(name, NAME_WIDTH);
        let desc = sanitize_slot(desc, DESC_WIDTH);
        format!("ROW name={name}|desc={desc}")
    });
    crate::text::framed_ok("OK skills.list".into(), rows)
}

/// Guest `skills::Slot` sizes (name / desc).
const NAME_WIDTH: usize = 28;
const DESC_WIDTH: usize = 40;

fn sanitize_slot(s: &str, max: usize) -> String {
    crate::text::sanitize(s, max, true, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_frontmatter() {
        let sample = "---\nname: demo\ndescription: >-\n  Hello world skill.\n---\n\n# Demo\n";
        let (n, d) = parse_frontmatter(sample).unwrap();
        assert_eq!(n, "demo");
        assert!(d.contains("Hello world"));
    }

    #[test]
    fn defaults_dir_lists_builtins() {
        let (defaults, user) = skills_dirs();
        assert!(
            defaults.join("email-triage/SKILL.md").is_file(),
            "missing {}",
            defaults.display()
        );
        let list = list_skills(&defaults, &user);
        assert!(list.contains_key("email-triage"));
        assert!(list.contains_key("agent-plan-act"));
    }
}
