//! Capability grants.
//!
//! Connectors on the host bridge are tools behind caps — the setup assistant
//! chooses the grant set, and MCP calls must check here before talking to the
//! bridge. No ambient root: a missing grant is a hard deny, not a soft
//! skip-with-try.
//!
//! std port of `kernel/src/caps.rs`. The bitset is the model, not a no_std
//! workaround, so it is unchanged; only [`Caps::describe`] moved off a
//! caller-supplied byte buffer.

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
    /// Send mail. OFF by default; still needs an explicit Confirm send on
    /// Brief — grant alone never auto-sends (AGENTS non-negotiable #3).
    EmailSend = 6,
}

impl Cap {
    /// Screen order. The setup screen's labels address `ALL[i]`, so these must
    /// agree — a mismatch shows the right switch against the wrong name.
    pub const ALL: [Cap; 7] = [
        Cap::EmailSearch,
        Cap::EmailSend,
        Cap::SearchQuery,
        Cap::WorkspaceIndex,
        Cap::AudioTranscribe,
        Cap::SkillsSave,
        Cap::PortalSync,
    ];

    /// What this is called on screen. Plain language: someone deciding what
    /// the machine may touch is not reading an API.
    pub const fn label(self) -> &'static str {
        match self {
            Cap::EmailSearch => "Email",
            Cap::EmailSend => "Send mail",
            Cap::SearchQuery => "Built-in docs",
            Cap::WorkspaceIndex => "Your files",
            Cap::AudioTranscribe => "Recordings",
            Cap::SkillsSave => "Save skills",
            Cap::PortalSync => "Online services",
        }
    }

    /// The consequence of granting it, in one clause (Guided UI).
    pub const fn detail(self) -> &'static str {
        match self {
            Cap::EmailSearch => "Read inbox and calendar",
            Cap::EmailSend => "Send after you confirm on Brief",
            Cap::SearchQuery => "Search what ships with the OS",
            Cap::WorkspaceIndex => "Search project folders you choose",
            Cap::AudioTranscribe => "Type a media path in Search",
            Cap::SkillsSave => "Write playbooks; revoke deletes them",
            Cap::PortalSync => "Teddy and market portals",
        }
    }

    /// Blurb beside the Caps label — plain language or wire name by level.
    pub const fn blurb(self, level: crate::level::Level) -> &'static str {
        match level {
            crate::level::Level::Guided => self.detail(),
            crate::level::Level::Advanced => self.name(),
        }
    }

    /// The wire/tool identifier. Shown in Advanced Caps rows.
    pub const fn name(self) -> &'static str {
        match self {
            Cap::EmailSearch => "email.search",
            Cap::EmailSend => "email.send",
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
    /// Defaults match the setup assistant: privacy first — only the corpus
    /// that ships with the OS is searchable. Personal data stays off.
    pub const fn default_grants() -> Self {
        Self {
            bits: 1 << Cap::SearchQuery.index(),
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

    /// Describe the grant set in plain language, for the home footer.
    ///
    /// An earlier version returned one of eight fixed strings covering only
    /// three capabilities, so enabling Your files, Recordings or Online
    /// services changed nothing on screen — the home footer quietly disagreed
    /// with the switches the user had just set. Every granted cap must appear.
    pub fn describe(self) -> String {
        if self.count() == 0 {
            return "Nothing enabled".to_string();
        }
        let mut s = String::from("On: ");
        let mut first = true;
        for cap in Cap::ALL {
            if !self.allows(cap) {
                continue;
            }
            if !first {
                s.push_str(", ");
            }
            first = false;
            s.push_str(cap.label());
        }
        s
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
        assert!(c.allows(Cap::SearchQuery));
        assert!(!c.allows(Cap::EmailSearch));
        assert!(!c.allows(Cap::EmailSend));
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
        // The footer shares one row with the rest of the home chrome, so the
        // 96-byte budget outlives the fixed buffer it used to be written into.
        for bits in 0u8..=127 {
            let c = Caps { bits };
            let s = c.describe();
            assert!(s.bytes().all(|b| (0x20..=0x7E).contains(&b)));
            assert!(s.len() < 96, "footer too long: {s}");
        }
    }

    #[test]
    fn footer_names_every_granted_cap() {
        let mut c = Caps::none();
        for cap in Cap::ALL {
            c.set(cap, true);
        }
        let s = c.describe();
        for cap in Cap::ALL {
            assert!(s.contains(cap.label()), "{} missing from {s}", cap.name());
        }
    }

    #[test]
    fn names_match_setup_rows() {
        assert_eq!(Cap::EmailSearch.name(), "email.search");
        assert_eq!(Cap::EmailSend.name(), "email.send");
        assert_eq!(Cap::SearchQuery.name(), "search.query");
        assert_eq!(Cap::SkillsSave.name(), "skills.save");
    }

    #[test]
    fn send_mail_is_not_implied_by_email_read() {
        let mut c = Caps::none();
        c.set(Cap::EmailSearch, true);
        assert!(!c.allows(Cap::EmailSend));
    }

    #[test]
    fn email_detail_names_calendar() {
        let d = Cap::EmailSearch.detail();
        assert!(d.contains("calendar"), "{d}");
        assert!(d.bytes().all(|b| (0x20..=0x7E).contains(&b)), "{d}");
    }

    #[test]
    fn workspace_detail_does_not_claim_the_whole_machine() {
        let d = Cap::WorkspaceIndex.detail();
        assert!(!d.contains("this machine"), "{d}");
        assert!(d.contains("project") || d.contains("folder"), "{d}");
        assert!(d.bytes().all(|b| (0x20..=0x7E).contains(&b)));
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
            assert_eq!(
                seen & bit,
                0,
                "{} shares a bit with another cap",
                cap.name()
            );
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
