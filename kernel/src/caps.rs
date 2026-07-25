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
}

impl Cap {
    pub const ALL: [Cap; 4] =
        [Cap::EmailSearch, Cap::SearchQuery, Cap::SkillsSave, Cap::WorkspaceIndex];

    pub const fn name(self) -> &'static str {
        match self {
            Cap::EmailSearch => "email.search",
            Cap::SearchQuery => "search.query",
            Cap::SkillsSave => "skills.save",
            Cap::WorkspaceIndex => "workspace.index",
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

    pub fn set(&mut self, cap: Cap, on: bool) {
        if on {
            self.bits |= 1 << cap.index();
        } else {
            self.bits &= !(1 << cap.index());
        }
    }

    /// Short ASCII status for the home footer (fits a 1024px row).
    pub fn footer_status(self) -> &'static str {
        let e = self.allows(Cap::EmailSearch);
        let s = self.allows(Cap::SearchQuery);
        let w = self.allows(Cap::SkillsSave);
        match (e, s, w) {
            (true, true, false) => "caps: email.search + search.query",
            (true, true, true) => "caps: email + search + skills.save",
            (true, false, false) => "caps: email.search only",
            (false, true, false) => "caps: search.query only",
            (false, false, false) => "caps: none granted",
            (true, false, true) => "caps: email.search + skills.save",
            (false, true, true) => "caps: search.query + skills.save",
            (false, false, true) => "caps: skills.save only",
        }
    }

    /// Count of granted caps.
    pub fn count(self) -> usize {
        Cap::ALL.iter().filter(|&&c| self.allows(c)).count()
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
    }

    #[test]
    fn from_bools_round_trips() {
        let c = Caps::from_bools(&[false, true, true]);
        assert!(!c.allows(Cap::EmailSearch));
        assert!(c.allows(Cap::SearchQuery));
        assert!(c.allows(Cap::SkillsSave));
        assert_eq!(c.count(), 2);
    }

    #[test]
    fn footer_is_ascii_and_short() {
        for bits in 0u8..8 {
            let c = Caps { bits };
            let s = c.footer_status();
            assert!(s.bytes().all(|b| (0x20..=0x7E).contains(&b)));
            assert!(s.len() < 48, "footer too long: {s}");
        }
    }

    #[test]
    fn names_match_setup_rows() {
        assert_eq!(Cap::EmailSearch.name(), "email.search");
        assert_eq!(Cap::SearchQuery.name(), "search.query");
        assert_eq!(Cap::SkillsSave.name(), "skills.save");
    }
}
