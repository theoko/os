//! Load and serve agent skills (SKILL.md) from defaults + user save dir.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub builtin: bool,
}

pub fn skills_dirs() -> (PathBuf, PathBuf) {
    let defaults = env::var("OS_SKILLS_DEFAULTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/defaults")
        });
    let user = env::var("OS_SKILLS_USER")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_home().join("Library/Application Support/os/skills"));
    (defaults, user)
}

fn dirs_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn list_skills() -> Vec<SkillMeta> {
    let (defaults, user) = skills_dirs();
    let mut map: BTreeMap<String, SkillMeta> = BTreeMap::new();
    let mut tmp = Vec::new();
    collect_dir(&defaults, true, &mut tmp);
    for s in tmp.drain(..) {
        map.insert(s.name.clone(), s);
    }
    collect_dir(&user, false, &mut tmp);
    for s in tmp.drain(..) {
        map.insert(s.name.clone(), s); // saved overrides default
    }
    map.into_values().collect()
}

fn collect_dir(dir: &Path, builtin: bool, out: &mut Vec<SkillMeta>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for ent in entries.flatten() {
        let path = ent.path().join("SKILL.md");
        if !path.is_file() {
            continue;
        }
        if let Some(meta) = parse_skill(&path, builtin) {
            out.push(meta);
        }
    }
}

fn parse_skill(path: &Path, builtin: bool) -> Option<SkillMeta> {
    let text = fs::read_to_string(path).ok()?;
    let (name, description) = parse_frontmatter(&text)?;
    Some(SkillMeta {
        name,
        description,
        path: path.to_path_buf(),
        builtin,
    })
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

pub fn get_skill_body(name: &str) -> Option<String> {
    list_skills()
        .into_iter()
        .find(|s| s.name == name)
        .and_then(|s| fs::read_to_string(s.path).ok())
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
    let body = if body.starts_with("---") {
        body.to_string()
    } else {
        format!("---\nname: {name}\ndescription: User-saved skill.\n---\n\n{body}")
    };
    fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Delete the user skills tree. Revoking Save skills must forget what it
/// wrote — defaults under `OS_SKILLS_DEFAULTS` are never touched.
pub fn forget() -> Result<&'static str, String> {
    let (defaults, user) = skills_dirs();
    // Refuse to wipe the curated defaults if env mispoints USER at them.
    if let (Ok(d), Ok(u)) = (defaults.canonicalize(), user.canonicalize()) {
        if d == u {
            return Err("refusing to forget defaults dir".into());
        }
    }
    match fs::remove_dir_all(&user) {
        Ok(()) => Ok("removed"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("nothing_to_remove"),
        Err(e) => Err(format!("remove {}: {e}", user.display())),
    }
}

pub fn list_response() -> Vec<String> {
    let skills = list_skills();
    let n = skills.len();
    let mut out = vec![format!("OK skills.list n={n}")];
    for s in &skills {
        // Frontmatter names are untrusted text: sanitize like desc so a '|'
        // in a name cannot inject ROW fields.
        let name = sanitize(&s.name);
        let desc = sanitize(&s.description);
        let src = if s.builtin { "default" } else { "saved" };
        out.push(format!("ROW name={name}|src={src}|desc={desc}"));
    }
    out.push("END".into());
    out
}

pub fn get_response(name: &str) -> Vec<String> {
    match get_skill_body(name) {
        Some(body) => {
            let mut out = vec!["OK skills.get".into()];
            for line in body.lines() {
                // LINE payload is the whole rest of the line, not a ROW with
                // '|'-separated fields — preserve it verbatim so save→get
                // round-trips; only strip control chars that would break the
                // line framing (keep tabs for markdown code blocks).
                out.push(format!("LINE {}", strip_line_controls(line)));
            }
            out.push("END".into());
            out
        }
        None => vec!["ERR skills.get not_found".into()],
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\r' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .take(120)
        .collect()
}

fn strip_line_controls(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() && c != '\t' { ' ' } else { c })
        .collect()
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
        let (defaults, _) = skills_dirs();
        assert!(
            defaults.join("email-triage/SKILL.md").is_file(),
            "missing {}",
            defaults.display()
        );
        let list = list_skills();
        assert!(list.iter().any(|s| s.name == "email-triage"));
        assert!(list.iter().any(|s| s.name == "agent-plan-act"));
    }

    #[test]
    fn forget_removes_saved_skills_not_defaults() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-skills-forget-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        unsafe {
            env::set_var("OS_SKILLS_USER", &dir);
        }
        save_skill("unit-saved", "names email.search").unwrap();
        assert!(dir.join("unit-saved/SKILL.md").is_file());
        assert!(list_skills().iter().any(|s| s.name == "unit-saved" && !s.builtin));
        assert_eq!(forget().unwrap(), "removed");
        assert!(!dir.exists());
        assert!(
            list_skills().iter().any(|s| s.name == "email-triage" && s.builtin),
            "defaults must survive skills.forget"
        );
        assert_eq!(forget().unwrap(), "nothing_to_remove");
        unsafe {
            env::remove_var("OS_SKILLS_USER");
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
