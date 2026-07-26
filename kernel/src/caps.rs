//! Guest-side capability grants.
//!
//! Connectors on the host bridge are tools behind caps — the setup assistant
//! chooses the grant set, and MCP calls must check here before talking COM2.
//! No ambient root: a missing grant is a hard deny, not a soft skip-with-try.

/// Named capabilities that mirror bridge tools / setup rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cap {
    EmailSearch = 0,
    SearchQuery = 1,
    SkillsSave = 2,
    /// Index and search the user's own documents. Off by default: a personal
    /// file tree is not something to opt someone into silently.
    WorkspaceIndex = 3,
    /// Transcribe local audio/video and index the text. Off by default: a
    /// recording can contain anyone, not just the user.
    AudioTranscribe = 4,
}

impl Cap {
    pub const ALL: [Cap; 5] = [
        Cap::EmailSearch,
        Cap::SearchQuery,
        Cap::SkillsSave,
        Cap::WorkspaceIndex,
        Cap::AudioTranscribe,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Cap::EmailSearch => "email.search",
            Cap::SearchQuery => "search.query",
            Cap::SkillsSave => "skills.save",
            Cap::WorkspaceIndex => "workspace.index",
            Cap::AudioTranscribe => "audio.transcribe",
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Bitset of granted capabilities (one bit per [`Cap`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    bits: u8,
}

impl Caps {
    /// Defaults match the setup assistant: read tools on, disk writes off.
    pub const fn default_grants() -> Self {
        Self {
            bits: (1 << Cap::EmailSearch.index()) | (1 << Cap::SearchQuery.index()),
        }
    }

    pub const fn none() -> Self {
        Self { bits: 0 }
    }

    pub fn from_bools(flags: &[bool]) -> Self {
        let mut bits = 0u8;
        for (i, on) in flags.iter().enumerate().take(Cap::ALL.len()) {
            if *on {
                bits |= 1 << i;
            }
        }
        Self { bits }
    }

    pub const fn allows(self, cap: Cap) -> bool {
        self.bits & (1 << cap.index()) != 0
    }

    /// How many named capabilities are currently granted.
    pub fn granted_count(self) -> usize {
        Cap::ALL.iter().filter(|&&c| self.allows(c)).count()
    }

    pub fn set(&mut self, cap: Cap, on: bool) {
        if on {
            self.bits |= 1 << cap.index();
        } else {
            self.bits &= !(1 << cap.index());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_denies_skills_save() {
        let c = Caps::default_grants();
        assert!(c.allows(Cap::EmailSearch));
        assert!(c.allows(Cap::SearchQuery));
        assert!(!c.allows(Cap::SkillsSave));
        assert_eq!(c.granted_count(), 2);
    }

    #[test]
    fn from_bools_round_trips() {
        let c = Caps::from_bools(&[false, true, true]);
        assert!(!c.allows(Cap::EmailSearch));
        assert!(c.allows(Cap::SearchQuery));
        assert!(c.allows(Cap::SkillsSave));
    }

    #[test]
    fn wire_names_are_stable() {
        assert_eq!(Cap::EmailSearch.name(), "email.search");
        assert_eq!(Cap::SearchQuery.name(), "search.query");
        assert_eq!(Cap::SkillsSave.name(), "skills.save");
        assert_eq!(Cap::WorkspaceIndex.name(), "workspace.index");
        assert_eq!(Cap::AudioTranscribe.name(), "audio.transcribe");
    }
}
