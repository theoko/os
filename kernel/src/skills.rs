//! Builtin skill catalog (always available offline) + optional bridge merge.

/// A skill name shown in the home UI.
pub struct SkillRef {
    pub name: &'static str,
    pub blurb: &'static str,
}

/// Defaults shipped with the OS (mirrors skills/defaults/).
pub const BUILTIN: &[SkillRef] = &[
    SkillRef {
        name: "agent-plan-act",
        blurb: "Plan, act, report",
    },
    SkillRef {
        name: "capability-safe-tools",
        blurb: "Least-privilege caps",
    },
    SkillRef {
        name: "email-triage",
        blurb: "Inbox via MCP email",
    },
    SkillRef {
        name: "inbox-brief",
        blurb: "Short mail brief",
    },
    SkillRef {
        name: "knowledge-search",
        blurb: "Corpus search",
    },
];

pub struct SkillPeek {
    pub count: usize,
    pub names: [[u8; 28]; 8],
}

impl SkillPeek {
    pub fn from_builtin() -> Self {
        let mut peek = Self {
            count: 0,
            names: [[0; 28]; 8],
        };
        for s in BUILTIN.iter().take(peek.names.len()) {
            copy_name(&mut peek.names[peek.count], s.name);
            peek.count += 1;
        }
        peek
    }

    pub fn name_at(&self, i: usize) -> &str {
        let n = self.names[i]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.names[i].len());
        core::str::from_utf8(&self.names[i][..n]).unwrap_or("")
    }
}

fn copy_name(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let b = src.as_bytes();
    let n = b.len().min(dst.len());
    dst[..n].copy_from_slice(&b[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_present() {
        let p = SkillPeek::from_builtin();
        assert!(p.count >= 5);
        assert_eq!(p.name_at(0), "agent-plan-act");
    }
}
