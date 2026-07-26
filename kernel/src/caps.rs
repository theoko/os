//! Guest-side capability grants.
//!
//! Connectors on the host bridge are tools behind caps — the setup assistant
//! chooses the grant set, and MCP calls must check here before talking COM2.
//! No ambient root: a missing grant is a hard deny, not a soft skip-with-try.

/// Named capabilities that mirror bridge tools / setup rows.
#[derive(Clone, Copy)]
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
    pub(crate) const ALL: [Cap; 5] = [
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

    /// One-line UI blurb for setup / Capabilities rows.
    pub const fn blurb(self) -> &'static str {
        match self {
            Cap::EmailSearch => "Read the inbox through the host bridge",
            Cap::SearchQuery => "Query the built-in knowledge corpus",
            Cap::SkillsSave => "Write new skill playbooks to disk",
            Cap::WorkspaceIndex => "Search your own files on this machine",
            Cap::AudioTranscribe => "Transcribe recordings and index what was said",
        }
    }
}

/// Bitset of granted capabilities (one bit per [`Cap`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    bits: u8,
}

impl Caps {
    /// Defaults match the setup assistant: read tools on, disk writes off.
    pub(crate) const fn default_grants() -> Self {
        Self {
            bits: (1 << Cap::EmailSearch as usize) | (1 << Cap::SearchQuery as usize),
        }
    }

    pub const fn none() -> Self {
        Self { bits: 0 }
    }

    pub const fn allows(self, cap: Cap) -> bool {
        self.bits & (1 << cap as usize) != 0
    }

    /// How many named capabilities are currently granted.
    ///
    /// [`Cap`] values are dense bits `0..ALL.len()`, so the popcount of the
    /// bitset matches an `allows` walk over [`Cap::ALL`].
    pub const fn granted_count(self) -> usize {
        self.bits.count_ones() as usize
    }

    /// Flip capability `i` in place. Out-of-range is a no-op.
    pub fn toggle(&mut self, i: usize) {
        if let Some(c) = Cap::ALL.get(i) {
            self.bits ^= 1 << *c as usize;
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
    fn wire_names_are_stable() {
        assert_eq!(Cap::EmailSearch.name(), "email.search");
        assert_eq!(Cap::SearchQuery.name(), "search.query");
        assert_eq!(Cap::SkillsSave.name(), "skills.save");
        assert_eq!(Cap::WorkspaceIndex.name(), "workspace.index");
        assert_eq!(Cap::AudioTranscribe.name(), "audio.transcribe");
    }

    #[test]
    fn every_cap_has_an_ascii_blurb() {
        for cap in Cap::ALL {
            let b = cap.blurb();
            assert!(!b.is_empty(), "{:?} blurb empty", cap.name());
            assert!(
                b.bytes().all(|c| (0x20..=0x7E).contains(&c)),
                "non-ASCII blurb: {b:?}"
            );
        }
    }

    #[test]
    fn toggle_flips_only_the_named_capability() {
        let mut g = Caps::none();
        g.toggle(0);
        assert!(g.allows(Cap::ALL[0]));
        assert!(!g.allows(Cap::ALL[1]), "toggling one must not affect another");
        g.toggle(0);
        assert!(!g.allows(Cap::ALL[0]), "must toggle back off");
    }

    #[test]
    fn toggle_out_of_range_is_a_noop() {
        let mut g = Caps::default_grants();
        let before = g;
        g.toggle(99);
        assert_eq!(g, before);
    }
}
