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
    /// Exchange data with external portals (teddy, markets). OFF by
    /// default: every other capability is local, this one leaves the machine.
    PortalSync = 5,
}

impl Cap {
    /// Screen order. `setup::CAPS[i]` labels `ALL[i]`, so these must agree —
    /// a mismatch shows the right switch against the wrong name.
    pub const ALL: [Cap; 6] = [
        Cap::EmailSearch,
        Cap::SearchQuery,
        Cap::WorkspaceIndex,
        Cap::AudioTranscribe,
        Cap::SkillsSave,
        Cap::PortalSync,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Cap::EmailSearch => "email.search",
            Cap::SearchQuery => "search.query",
            Cap::SkillsSave => "skills.save",
            Cap::WorkspaceIndex => "workspace.index",
            Cap::AudioTranscribe => "audio.transcribe",
            Cap::PortalSync => "portal.sync",
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

    /// Build a grant set from switch states, in `Cap::ALL` (screen) order.
    ///
    /// The bit is taken from the capability itself, NOT from the flag's
    /// position. Those were the same until `ALL` was reordered to match the
    /// setup screen, after which position `i` addressed whatever enum variant
    /// happened to have discriminant `i` — so enabling "Your files" granted
    /// "Save skills". A consent screen that grants something other than what
    /// it names is the worst failure this model can have.
    pub fn from_bools(flags: &[bool]) -> Self {
        let mut bits = 0u8;
        for (i, on) in flags.iter().enumerate().take(Cap::ALL.len()) {
            if *on {
                bits |= 1 << Cap::ALL[i].index();
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
        // Flags are in Cap::ALL (screen) order, so index 2 is whatever the
        // third row shows — not whichever variant has discriminant 2.
        let c = Caps::from_bools(&[false, true, true]);
        assert!(!c.allows(Cap::ALL[0]));
        assert!(c.allows(Cap::ALL[1]));
        assert!(c.allows(Cap::ALL[2]));
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

#[cfg(test)]
mod order_tests {
    use super::*;

    #[test]
    fn a_switch_grants_the_capability_it_names() {
        // Regression: Cap::ALL is in screen order while index() is the enum
        // discriminant. Mapping flag position straight to bit position meant a
        // reorder silently granted the wrong capability.
        for (i, cap) in Cap::ALL.iter().enumerate() {
            let mut flags = [false; Cap::ALL.len()];
            flags[i] = true;
            let g = Caps::from_bools(&flags);
            assert!(g.allows(*cap), "flag {i} did not grant {}", cap.name());
            for other in Cap::ALL.iter().filter(|c| c.index() != cap.index()) {
                assert!(
                    !g.allows(*other),
                    "flag {i} ({}) also granted {}",
                    cap.name(),
                    other.name()
                );
            }
        }
    }

    #[test]
    fn every_capability_has_a_distinct_bit() {
        let mut seen = 0u8;
        for cap in Cap::ALL {
            let bit = 1u8 << cap.index();
            assert_eq!(seen & bit, 0, "{} shares a bit with another cap", cap.name());
            seen |= bit;
        }
    }

    #[test]
    fn no_capability_exceeds_the_bitset() {
        for cap in Cap::ALL {
            assert!(cap.index() < 8, "{} does not fit in u8", cap.name());
        }
    }
}
