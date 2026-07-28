//! Copy for everything the guest cannot do without a host bridge.
//!
//! "Bridge offline" describes a condition the user can fix by starting the
//! host. On a standalone image there is no host to start, so the same words
//! name a remedy that does not exist — worse than saying nothing, because it
//! sends the reader off to debug a connection the machine was never built to
//! have. These say what is permanently true of the device instead.
//!
//! The capability is identical either way. Only the explanation changes.

use crate::mcp::standalone;

// ---- Brief lines (agent.rs) ------------------------------------------------

pub const fn search_local_only() -> &'static str {
    if standalone() {
        "Local keywords only."
    } else {
        "Bridge offline - local keywords only."
    }
}

/// The empty answer from the index that ships inside the image.
///
/// "No offline hits" names offline as the reason, which invites the reader to
/// go back online and try again. On a standalone image there is no other
/// state to reach: the index inside the ISO is the whole world, and the honest
/// sentence is that this machine does not know that word.
pub const fn no_local_hits() -> &'static str {
    if standalone() {
        "Nothing on this device matches that."
    } else {
        "No offline hits for that query."
    }
}

/// The way forward after an empty answer.
pub const fn retry_broader() -> &'static str {
    if standalone() {
        "Try a broader word - the built-in index is small."
    } else {
        "Try a broader word, or start the bridge."
    }
}

/// Why a screen of findings has nothing to tap.
///
/// Not "the index cannot open documents" any more — it can, for everything
/// baked with an extract. These particular rows are the ones it holds no text
/// for, and saying otherwise would tell the reader to stop trying.
pub const fn titles_only() -> &'static str {
    if standalone() {
        "Titles only - no text stored for these."
    } else {
        "Titles only - reading needs the bridge."
    }
}

pub const fn mail_unavailable() -> &'static str {
    if standalone() {
        "Mail is unavailable on this device."
    } else {
        "Bridge offline for mail."
    }
}

pub const fn mail_unreadable() -> &'static str {
    if standalone() {
        "Mail is unavailable on this device."
    } else {
        "Bridge offline - cannot read mail."
    }
}

pub const fn calendar_unavailable() -> &'static str {
    if standalone() {
        "Calendar is unavailable on this device."
    } else {
        "Bridge offline for calendar."
    }
}

pub const fn corpus_unavailable() -> &'static str {
    if standalone() {
        "Corpus unavailable - only the baked index is on this device."
    } else {
        "Bridge offline - corpus unavailable."
    }
}

pub const fn portals_unavailable() -> &'static str {
    if standalone() {
        "Portals are unavailable on this device."
    } else {
        "Bridge offline for portals."
    }
}

pub const fn cannot_save() -> &'static str {
    if standalone() {
        "Saving is unavailable on this device."
    } else {
        "Bridge offline - cannot save."
    }
}

// ---- Search screen (searchui.rs) -------------------------------------------

pub const fn search_no_matches() -> &'static str {
    if standalone() {
        "No matches. This device is offline - only built-in docs are searchable."
    } else {
        "No matches. Bridge offline - only built-in docs are searchable."
    }
}

/// Where answers come from, under the search field.
pub const fn search_source_note() -> &'static str {
    if standalone() {
        "Answers come from the index built into this device."
    } else {
        "Bridge offline - answering from the index baked into the kernel."
    }
}

pub const fn answered_from_guide() -> &'static str {
    if standalone() {
        "Answered from the built-in guide - this device is offline."
    } else {
        "Answered from the built-in guide - the bridge is offline."
    }
}

/// Shown when a row was opened and the image holds no text for it.
///
/// Not a connection failure and not a promise that opening is impossible —
/// documents baked with an extract do open. This one is simply not stored.
pub const fn cannot_open_documents() -> &'static str {
    if standalone() {
        "This device stores no text for that document."
    } else {
        "Bridge offline - cannot open documents."
    }
}

/// Footer under a document read from the baked corpus.
///
/// The reader would otherwise present a stored opening as the whole document,
/// which is the same lie in the other direction: a page that ends mid-thought
/// with nothing saying why.
pub const fn extract_only() -> &'static str {
    if standalone() {
        "-- extract: this device stores the opening of each document --"
    } else {
        "-- extract from the baked index, not the full document --"
    }
}

// ---- Status and config (screens.rs, ui.rs) ---------------------------------

/// The remedy line under an offline connector. `make utm-bridged` is advice
/// only a VM guest can act on.
pub const fn offline_remedy() -> &'static str {
    if standalone() {
        "Connectors are unavailable on this device."
    } else {
        "Bridge offline - run: make utm-bridged"
    }
}

/// Provenance for the built-in corpus rows on a Brief, when their bodies are
/// out of reach. The rows carry titles and no URL in that state, so this says
/// why they do not open — see `agent::fill_corpus_lines`.
pub const fn builtin_docs_titles_only() -> &'static str {
    if standalone() {
        "Built-in docs - titles only on this device."
    } else {
        "Built-in docs - titles only until the bridge is up."
    }
}

pub const fn skills_source() -> &'static str {
    if standalone() {
        "Built in"
    } else {
        "From host bridge"
    }
}

pub const fn save_starter_hint() -> &'static str {
    if standalone() {
        "Saved playbooks CALL granted tools. Saving is unavailable here."
    } else {
        "Saved playbooks CALL granted tools. Save starter needs the bridge."
    }
}

// ---- First-run setup (setup.rs) --------------------------------------------

pub const fn setup_bridge_title() -> &'static str {
    if standalone() {
        "Runs On Its Own"
    } else {
        "Connect the Bridge"
    }
}

pub const fn setup_bridge_sub_guided() -> &'static str {
    if standalone() {
        "This device works without a host. Everything runs locally."
    } else {
        "Connectors run on the host, never in the kernel."
    }
}

pub const fn setup_bridge_sub_plain() -> &'static str {
    if standalone() {
        "No host connectors. Local index only."
    } else {
        "Host MCP on COM2. Probe only until you grant."
    }
}

pub const fn setup_bridge_label() -> &'static str {
    if standalone() {
        "Local only"
    } else {
        "Host bridge"
    }
}

pub const fn setup_bridge_detail_guided() -> &'static str {
    if standalone() {
        "No host needed - nothing to start"
    } else {
        "Offline - start it with 'make bridge-run'"
    }
}

pub const fn setup_bridge_detail_plain() -> &'static str {
    if standalone() {
        "No host needed"
    } else {
        "Offline - make bridge-run"
    }
}

pub const fn setup_skills_builtin() -> &'static str {
    if standalone() {
        "Builtin playbooks."
    } else {
        "Builtin playbooks (bridge offline)."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line the two builds disagree about, so a new one cannot be added
    /// in only one configuration and go unnoticed.
    const ALL: &[fn() -> &'static str] = &[
        search_local_only,
        no_local_hits,
        retry_broader,
        titles_only,
        mail_unavailable,
        mail_unreadable,
        calendar_unavailable,
        corpus_unavailable,
        portals_unavailable,
        cannot_save,
        search_no_matches,
        search_source_note,
        answered_from_guide,
        cannot_open_documents,
        extract_only,
        offline_remedy,
        builtin_docs_titles_only,
        skills_source,
        save_starter_hint,
        setup_bridge_title,
        setup_bridge_sub_guided,
        setup_bridge_sub_plain,
        setup_bridge_label,
        setup_bridge_detail_guided,
        setup_bridge_detail_plain,
        setup_skills_builtin,
    ];

    #[test]
    fn standalone_copy_never_names_a_host_that_cannot_exist() {
        for line in ALL {
            let line = line();
            let lower = line.to_ascii_lowercase();
            if standalone() {
                assert!(
                    !lower.contains("bridge"),
                    "standalone copy names the bridge: {line}"
                );
                // A remedy the user cannot perform is worse than no remedy.
                assert!(
                    !lower.contains("make "),
                    "standalone copy tells the user to run make: {line}"
                );
            } else {
                // The hosted build must still say it somewhere in this set,
                // or the test would pass by both builds going vague.
                assert!(!lower.is_empty());
            }
        }
        if !standalone() {
            let hosted_mentions = ALL
                .iter()
                .filter(|f| f().to_ascii_lowercase().contains("bridge"))
                .count();
            assert!(
                hosted_mentions >= 12,
                "hosted copy stopped naming the bridge ({hosted_mentions} lines)"
            );
        }
    }
}
