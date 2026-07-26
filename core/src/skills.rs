//! Builtin skill catalog (always available offline) + optional bridge merge.
//!
//! std port of `kernel/src/skills.rs`. The catalog and the workflows are
//! unchanged; [`SkillPeek`] no longer stores names in fixed byte arrays.

use crate::caps::Cap;

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
    SkillRef {
        name: "teddy-portals",
        blurb: "Teddy API + live portals",
    },
    SkillRef {
        name: "market-portals",
        blurb: "Live market health + fear/greed",
    },
];

/// A deliberately small executable view of a playbook.  This is not a YAML
/// runtime: it is the stable representation the richer host-side skill format
/// can compile down to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Workflow {
    pub title: &'static str,
    pub required: Option<Cap>,
    pub steps: &'static [&'static str],
}

const PLAN_STEPS: &[&str] = &[
    "State the goal",
    "Check the capabilities needed",
    "Make a short plan",
    "Review before acting",
    "Report what happened",
];
const EMAIL_STEPS: &[&str] = &[
    "Check email permission",
    "Search a narrow inbox query",
    "Rank the urgent messages",
    "Draft replies for review",
];
const SEARCH_STEPS: &[&str] = &[
    "Choose a short search query",
    "Search the knowledge corpus",
    "Read the strongest source",
    "Use cited results in the plan",
];
const SAFE_TOOL_STEPS: &[&str] = &[
    "Name the task",
    "Choose the smallest capability",
    "Check what will be shared",
    "Ask before a consequential action",
];

/// Resolve a catalog name into an interactive, review-first workflow.
pub fn workflow_for(name: &str) -> Workflow {
    match name {
        "email-triage" | "inbox-brief" => Workflow {
            title: "Email triage",
            required: Some(Cap::EmailSearch),
            steps: EMAIL_STEPS,
        },
        "knowledge-search" => Workflow {
            title: "Knowledge search",
            required: Some(Cap::SearchQuery),
            steps: SEARCH_STEPS,
        },
        "capability-safe-tools" => Workflow {
            title: "Safe tools",
            required: None,
            steps: SAFE_TOOL_STEPS,
        },
        _ => Workflow {
            title: "Plan and act",
            required: None,
            steps: PLAN_STEPS,
        },
    }
}

/// Rows the skills list shows at once.
///
/// This survives the move off fixed arrays: it is a screen budget, not a
/// storage limit. `skills.list` on the bridge is unbounded, and a peek that
/// silently grew past the panel would push the footer off the display.
pub const MAX_SLOTS: usize = 8;

/// One row of a [`SkillPeek`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillEntry {
    pub name: String,
    pub desc: String,
    /// `true` when the bridge ROW said `src=saved` (user-written playbook).
    pub saved: bool,
}

/// Names (+ short descs) from builtins or `CALL skills.list`.
#[derive(Clone, Debug, Default)]
pub struct SkillPeek {
    /// True when the last fill came from the host bridge.
    pub from_bridge: bool,
    entries: Vec<SkillEntry>,
}

impl SkillPeek {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_builtin() -> Self {
        let mut peek = Self::empty();
        for s in BUILTIN.iter().take(MAX_SLOTS) {
            peek.push(s.name, s.blurb);
        }
        peek
    }

    pub fn push(&mut self, name: &str, desc: &str) -> bool {
        self.push_src(name, desc, false)
    }

    pub fn push_src(&mut self, name: &str, desc: &str, saved: bool) -> bool {
        if self.entries.len() >= MAX_SLOTS {
            return false;
        }
        self.entries.push(SkillEntry {
            name: name.to_string(),
            desc: desc.to_string(),
            saved,
        });
        true
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[SkillEntry] {
        &self.entries
    }

    pub fn name_at(&self, i: usize) -> &str {
        self.entries.get(i).map_or("", |e| e.name.as_str())
    }

    pub fn desc_at(&self, i: usize) -> &str {
        self.entries.get(i).map_or("", |e| e.desc.as_str())
    }

    pub fn is_saved_at(&self, i: usize) -> bool {
        self.entries.get(i).is_some_and(|e| e.saved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_present() {
        let p = SkillPeek::from_builtin();
        assert!(p.count() >= 7);
        assert_eq!(p.name_at(0), "agent-plan-act");
        assert!(
            BUILTIN.iter().any(|s| s.name == "teddy-portals"),
            "teddy API + portals skill must ship in the ISO"
        );
        assert!(
            BUILTIN.iter().any(|s| s.name == "market-portals"),
            "market portals skill must ship in the ISO"
        );
        assert!(!p.from_bridge);
    }

    #[test]
    fn push_caps_at_slot_limit() {
        let mut p = SkillPeek::empty();
        for i in 0..MAX_SLOTS {
            assert!(p.push("n", "d"), "slot {i}");
        }
        assert!(!p.push("overflow", "no"));
        assert_eq!(p.count(), MAX_SLOTS);
    }

    #[test]
    fn saved_flag_tracks_src() {
        let mut p = SkillPeek::empty();
        assert!(p.push_src("guest-starter", "from guest", true));
        assert!(p.push_src("email-triage", "builtin", false));
        assert!(p.is_saved_at(0));
        assert!(!p.is_saved_at(1));
        assert!(!p.is_saved_at(9));
    }

    #[test]
    fn workflows_name_the_capability_before_the_action() {
        let email = workflow_for("email-triage");
        assert_eq!(email.required, Some(Cap::EmailSearch));
        assert!(email.steps.len() >= 3);
        let planning = workflow_for("agent-plan-act");
        assert_eq!(planning.required, None);
    }

    #[test]
    fn long_names_survive_intact() {
        // The kernel truncated into [u8; 28] / [u8; 40]; a bridge skill with a
        // long name came back clipped. Nothing should clip now.
        let long = "workspace-index-and-transcribe-everything";
        let desc = "A description comfortably longer than the old forty-byte field.";
        let mut p = SkillPeek::empty();
        assert!(p.push(long, desc));
        assert_eq!(p.name_at(0), long);
        assert_eq!(p.desc_at(0), desc);
    }

    #[test]
    fn out_of_range_rows_are_empty_not_panics() {
        let p = SkillPeek::from_builtin();
        assert_eq!(p.name_at(p.count()), "");
        assert_eq!(p.desc_at(99), "");
    }
}
