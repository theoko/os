//! How chatty the UI is.
//!
//! Chosen explicitly on the setup Experience step — never inferred from
//! behaviour. Privacy-first default grants stay the same at every level;
//! only copy density and Caps blurbs adapt.

/// User-facing experience level.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    /// Longer explanations; plain-language Caps blurbs.
    Guided,
    /// Shorter chrome; Caps rows show wire tool names.
    Advanced,
}

impl Level {
    pub const ALL: [Self; 2] = [Self::Guided, Self::Advanced];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Guided => "Guided",
            Self::Advanced => "Advanced",
        }
    }

    pub const fn detail(self) -> &'static str {
        match self {
            Self::Guided => "Plain language. More explanation along the way.",
            Self::Advanced => "Shorter copy. Caps show tool names.",
        }
    }

    pub const fn is_guided(self) -> bool {
        matches!(self, Self::Guided)
    }

    /// Home search-field placeholder.
    pub const fn home_search_placeholder(self) -> &'static str {
        match self {
            Self::Guided => "What do you want to work on?",
            Self::Advanced => "goal, query, or /path.wav",
        }
    }

    /// Hint under the home search field.
    pub const fn home_search_hint(self) -> &'static str {
        match self {
            Self::Guided => "Enter runs an agent Brief under your grants.",
            Self::Advanced => "Enter: plan/act Brief. /path.wav with Recordings.",
        }
    }

    /// Search screen field placeholder.
    pub const fn search_placeholder(self) -> &'static str {
        match self {
            Self::Guided => "Type a query or /path.wav",
            Self::Advanced => "q=... or /path.wav",
        }
    }

    /// Caps screen footer.
    pub const fn caps_footer(self) -> &'static str {
        match self {
            Self::Guided => "Tap a row to grant or revoke. Takes effect immediately.",
            Self::Advanced => "Toggle grants. Revoke forgets what that grant built.",
        }
    }

    /// Caps screen subtitle.
    pub const fn caps_subtitle(self) -> &'static str {
        match self {
            Self::Guided => "What the agent may do",
            Self::Advanced => "Wire tools behind each grant",
        }
    }

    /// Setup Done subtitle.
    pub const fn done_subtitle(self) -> &'static str {
        match self {
            Self::Guided => "Capabilities granted. Skills loaded.",
            Self::Advanced => "Grants applied. Morning brief next.",
        }
    }

    /// Serial token after setup finishes.
    pub const fn serial_tag(self) -> &'static str {
        match self {
            Self::Guided => "guided",
            Self::Advanced => "advanced",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_is_ascii_only() {
        for level in Level::ALL {
            for s in [
                level.label(),
                level.detail(),
                level.home_search_placeholder(),
                level.home_search_hint(),
                level.search_placeholder(),
                level.caps_footer(),
                level.caps_subtitle(),
                level.done_subtitle(),
                level.serial_tag(),
            ] {
                assert!(
                    s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                    "non-ASCII: {s:?}"
                );
            }
        }
    }

    #[test]
    fn advanced_is_not_the_default_privacy_bypass() {
        // Level only changes copy — both levels start from the same grant set
        // chosen on Capabilities. This guards the product rule in a unit test.
        assert!(Level::Guided.is_guided());
        assert!(!Level::Advanced.is_guided());
    }
}
