//! Load and serve agent skills (SKILL.md) from defaults + user save dir.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

struct SkillMeta {
    description: String,
    path: PathBuf,
}

pub fn skills_dirs() -> (PathBuf, PathBuf) {
    let defaults = env::var("OS_SKILLS_DEFAULTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/defaults"));
    let user = env::var("OS_SKILLS_USER")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::paths::app_support().join("skills"));
    (defaults, user)
}

/// Merged skill map (saved overrides default by name).
fn list_skills(defaults: &Path, user: &Path) -> BTreeMap<String, SkillMeta> {
    let mut map = BTreeMap::new();
    collect_dir(defaults, &mut map);
    collect_dir(user, &mut map);
    map
}

fn collect_dir(dir: &Path, map: &mut BTreeMap<String, SkillMeta>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for ent in entries.flatten() {
        let path = ent.path().join("SKILL.md");
        if !path.is_file() {
            continue;
        }
        if let Some((name, meta)) = parse_skill(&path) {
            map.insert(name, meta);
        }
    }
}

fn parse_skill(path: &Path) -> Option<(String, SkillMeta)> {
    let text = fs::read_to_string(path).ok()?;
    let (name, description) = parse_frontmatter(&text)?;
    Some((
        name,
        SkillMeta {
            description,
            path: path.to_path_buf(),
        },
    ))
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

pub fn save_skill(name: &str, body: &str) -> Result<PathBuf, String> {
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
    Ok(path)
}

pub fn list_response() -> Vec<String> {
    let (defaults, user) = skills_dirs();
    let skills = list_skills(&defaults, &user);
    let n = skills.len();
    let rows = skills.iter().map(|(name, s)| {
        // Frontmatter names are untrusted text: sanitize like desc so a '|'
        // in a name cannot inject ROW fields.
        let name = sanitize(name);
        let desc = sanitize(&s.description);
        let src = if s.path.starts_with(&defaults) {
            "default"
        } else {
            "saved"
        };
        format!("ROW name={name}|src={src}|desc={desc}")
    });
    crate::text::framed_ok(format!("OK skills.list n={n}"), rows)
}

pub fn get_response(name: &str) -> Vec<String> {
    let (defaults, user) = skills_dirs();
    let Some(body) = list_skills(&defaults, &user)
        .get(name)
        .and_then(|s| fs::read_to_string(&s.path).ok())
    else {
        return vec!["ERR skills.get not_found".into()];
    };
    // LINE payload is the whole rest of the line, not a ROW with
    // '|'-separated fields — preserve it verbatim so save→get
    // round-trips; only strip control chars that would break the
    // line framing (keep tabs for markdown code blocks).
    let lines = body.lines().map(|line| {
        // Preserve markdown tabs; scrub other controls that would break LINE framing.
        let scrubbed: String = line
            .chars()
            .map(|c| if c.is_control() && c != '\t' { ' ' } else { c })
            .collect();
        format!("LINE {scrubbed}")
    });
    crate::text::framed_ok("OK skills.get".into(), lines)
}

fn sanitize(s: &str) -> String {
    crate::text::sanitize(s, 120, false, false)
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
