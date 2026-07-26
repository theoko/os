//! Builtin skill catalog (always available offline) + optional bridge list.

/// Cap on bridge-listed skills (buffer, paint, home count, bridge `.take`).
pub(crate) const MAX_LISTED: usize = 6;

/// A skill name shown in the home UI.
pub(crate) struct SkillRef {
    pub name: &'static str,
    pub blurb: &'static str,
}

/// Defaults shipped with the OS (mirrors skills/defaults/).
pub(crate) const BUILTIN: &[SkillRef] = &[
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

/// Guest skill-row / bridge `guest_slot` budgets.
pub(crate) const NAME_CHARS: usize = 28;
pub(crate) const DESC_CHARS: usize = 40;

/// One skill row (fill via [`SkillPeek::push`]).
struct Slot {
    name: [u8; NAME_CHARS],
    desc: [u8; DESC_CHARS],
}

const EMPTY_SLOT: Slot = Slot {
    name: [0; NAME_CHARS],
    desc: [0; DESC_CHARS],
};

/// Names (+ short descs) from ISO builtins or a live bridge list.
///
/// One slot buffer for both sources. `live` is true after a framed
/// `skills.list` (including empty); false after ISO fill (offline / ERR).
pub struct SkillPeek {
    live: bool,
    count: usize,
    slots: [Slot; MAX_LISTED],
}

impl SkillPeek {
    const fn blank(live: bool) -> Self {
        Self {
            live,
            count: 0,
            slots: [EMPTY_SLOT; MAX_LISTED],
        }
    }

    /// Empty live peek ready for [`Self::push`] (bridge fill path).
    pub(crate) fn empty_live() -> Self {
        Self::blank(true)
    }

    /// ISO builtins copied into slots (offline / framed ERR fallback).
    ///
    /// `pub` for the kernel binary (`main` is a separate crate).
    pub fn from_builtins() -> Self {
        let mut p = Self::blank(false);
        for s in BUILTIN {
            let _ = p.push(s.name, s.blurb);
        }
        p
    }

    /// True after a framed `skills.list` (empty list stays live).
    pub fn is_live(&self) -> bool {
        self.live
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn push(&mut self, name: &str, desc: &str) -> bool {
        if self.count >= self.slots.len() {
            return false;
        }
        let slot = &mut self.slots[self.count];
        copy_field(&mut slot.name, name);
        copy_field(&mut slot.desc, desc);
        self.count += 1;
        true
    }

    pub(crate) fn name_at(&self, i: usize) -> &str {
        if i < self.count {
            str_at(&self.slots[i].name)
        } else {
            ""
        }
    }

    pub(crate) fn subtitle_at(&self, i: usize) -> &str {
        if i < self.count {
            str_at(&self.slots[i].desc)
        } else {
            ""
        }
    }
}

/// Decode the longest valid UTF-8 prefix — a cut mid-character must degrade
/// to a shorter string, not vanish entirely.
pub(crate) fn utf8_prefix(bytes: &[u8]) -> &str {
    match core::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
    }
}

/// Null-terminated fixed field as a string (UTF-8 prefix).
pub(crate) fn str_at(buf: &[u8]) -> &str {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    utf8_prefix(&buf[..n])
}

/// Copy `src` into a fixed field, never splitting a UTF-8 char.
pub(crate) fn copy_field(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let mut n = bytes.len().min(dst.len());
    // Never cut mid-character: a torn tail would make the whole field
    // undecodable when read back.
    while n > 0 && !src.is_char_boundary(n) {
        n -= 1;
    }
    dst[..n].copy_from_slice(&bytes[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_present() {
        let p = SkillPeek::from_builtins();
        assert_eq!(p.count(), BUILTIN.len());
        assert_eq!(p.name_at(0), "agent-plan-act");
        assert!(!p.is_live());
    }

    #[test]
    fn push_caps_at_slot_limit() {
        let mut p = SkillPeek::empty_live();
        for i in 0..MAX_LISTED {
            assert!(p.push("n", "d"), "slot {i}");
        }
        assert!(!p.push("overflow", "no"));
        assert_eq!(p.count(), MAX_LISTED);
        assert!(p.is_live());
    }

    #[test]
    fn empty_live_is_still_from_bridge() {
        let p = SkillPeek::empty_live();
        assert_eq!(p.count(), 0);
        assert!(p.is_live(), "framed empty list is not ISO builtins");
    }

    #[test]
    fn builtins_fit_slot_caps() {
        assert!(BUILTIN.len() <= MAX_LISTED);
        for s in BUILTIN {
            assert!(s.name.len() <= NAME_CHARS, "{}", s.name);
            assert!(s.blurb.len() <= DESC_CHARS, "{}", s.blurb);
        }
    }
}
