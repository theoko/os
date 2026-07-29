//! Guest-side capability grants.
//!
//! Cap-gated guest CALLs (Inbox/Knowledge) and scope flags (`files=1` /
//! `audio=1`) check here before talking COM2. No ambient root for those
//! paths: a missing grant is a hard deny, not a soft skip-with-try.
//!
//! Not every COM2 tool is Cap-gated: `skills.list` is free; `doc.read` only
//! forwards scopes (no Knowledge re-check). `skills.save` stays host-socket-only
//! (nc / bridge LINE…END) — no guest Cap. `email.send` is a host policy stub.

/// Named capabilities: UI rows + COM2 scope / forget wiring.
///
/// [`Self::label`] is what setup / Capabilities paint. [`Self::name`] is the
/// stable host producer / grant id (Inbox/Knowledge CALL verbs; Files/Transcripts
/// are host `workspace.index` / `audio.transcribe` — guest search sends
/// `files=1` / `audio=1`). Revoke purge CALLs use [`Self::forget_tool`].
#[derive(Clone, Copy)]
pub enum Cap {
    EmailSearch = 0,
    SearchQuery = 1,
    /// Search the user's own documents (`files=1`). Off by default: a personal
    /// file tree is not something to opt someone into silently. Guest search
    /// lazy-builds the host index when empty.
    WorkspaceIndex = 2,
    /// Include host-indexed transcripts in search (`audio=1`). Off by default:
    /// a recording can contain anyone, not just the user. Guest never sends
    /// `path=`; populate via host `CALL audio.transcribe path=…`.
    AudioTranscribe = 3,
}

impl Cap {
    /// Dense table for UI rows and revoke walks (`main` is a separate binary).
    pub const ALL: [Cap; 4] = [
        Cap::EmailSearch,
        Cap::SearchQuery,
        Cap::WorkspaceIndex,
        Cap::AudioTranscribe,
    ];

    /// Host producer / grant id. Not the Capabilities row title.
    ///
    /// Inbox/Knowledge: guest `CALL` verb. Files/Transcripts: host index tools
    /// (guest never CALLs these — scopes are `files=1` / `audio=1`).
    pub const fn name(self) -> &'static str {
        match self {
            Cap::EmailSearch => "email.search",
            Cap::SearchQuery => "search.query",
            Cap::WorkspaceIndex => "workspace.index",
            Cap::AudioTranscribe => "audio.transcribe",
        }
    }

    /// Short UI title for setup / Capabilities rows (what the grant *does*).
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Cap::EmailSearch => "Inbox",
            Cap::SearchQuery => "Knowledge",
            Cap::WorkspaceIndex => "Files",
            Cap::AudioTranscribe => "Transcripts",
        }
    }

    /// One-line UI blurb for setup / Capabilities rows.
    pub(crate) const fn blurb(self) -> &'static str {
        match self {
            Cap::EmailSearch => "Peek inbox count through the host bridge",
            Cap::SearchQuery => "Query the host knowledge corpus",
            Cap::WorkspaceIndex => "Search your own files on this machine",
            Cap::AudioTranscribe => "Search transcripts the host has indexed",
        }
    }

    /// Bridge tool that purges what this grant produced, if any.
    pub(crate) const fn forget_tool(self) -> Option<&'static str> {
        match self {
            Cap::WorkspaceIndex => Some("workspace.forget"),
            Cap::AudioTranscribe => Some("audio.forget"),
            _ => None,
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
    fn default_grants_are_read_tools_only() {
        let c = Caps::default_grants();
        assert!(c.allows(Cap::EmailSearch));
        assert!(c.allows(Cap::SearchQuery));
        assert!(!c.allows(Cap::WorkspaceIndex));
        assert!(!c.allows(Cap::AudioTranscribe));
        assert_eq!(c.granted_count(), 2);
    }

    #[test]
    fn producer_names_are_stable() {
        assert_eq!(Cap::EmailSearch.name(), "email.search");
        assert_eq!(Cap::SearchQuery.name(), "search.query");
        assert_eq!(Cap::WorkspaceIndex.name(), "workspace.index");
        assert_eq!(Cap::AudioTranscribe.name(), "audio.transcribe");
    }

    #[test]
    fn every_cap_has_ascii_label_and_blurb() {
        for cap in Cap::ALL {
            for s in [cap.label(), cap.blurb()] {
                assert!(!s.is_empty(), "{:?} empty", cap.name());
                assert!(
                    s.bytes().all(|c| (0x20..=0x7E).contains(&c)),
                    "non-ASCII: {s:?}"
                );
            }
        }
    }

    #[test]
    fn forget_tools_match_index_producers() {
        assert_eq!(Cap::WorkspaceIndex.forget_tool(), Some("workspace.forget"));
        assert_eq!(Cap::AudioTranscribe.forget_tool(), Some("audio.forget"));
        assert_eq!(Cap::EmailSearch.forget_tool(), None);
        assert_eq!(Cap::SearchQuery.forget_tool(), None);
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
