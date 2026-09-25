//! Form-factor and layout-variant detection for the on-screen keyboard.
//!
//! Two independent axes describe the rendered deck:
//!
//! * A [`FormFactor`] (physical size, 100% down to 60%, and the Apple
//!   decks). [`form_for_keys`] guesses it from the keys a device reports
//!   and [`form_for_name`] from size hints in the device's marketing
//!   name; a keyboard Keyloom recognises by its identifiers
//!   (`known.rs`) skips both.
//! * A variant (ANSI or ISO), detected per device from its evdev
//!   capabilities by the keyboard monitor, or from the HID country code
//!   of a recognised keyboard.
//!
//! Detection only picks defaults — the size and layout pickers in the UI
//! always win. See `docs/Form_Factor_Detection.md` for the full story.

/// The family a form factor belongs to: which decks it is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// The PC sizes, drawn from the design's 100% deck.
    Standard,
    /// Apple's keyboards, drawn with their own geometry and legends.
    Apple,
}

/// A physical keyboard size, from full-size (100%) down to 60%, or one
/// of Apple's.
pub struct FormFactor {
    /// Human-readable name shown in the size picker.
    pub name: &'static str,
    pub family: Family,
    /// Size order across families, larger first: 0 is a full-size board
    /// with a numeric keypad.
    pub rank: u8,
}

/// All supported form factors, largest first within each family.
pub static FORM_FACTORS: &[FormFactor] = &[
    FormFactor {
        name: "100% · Full-size",
        family: Family::Standard,
        rank: 0,
    },
    FormFactor {
        name: "80% · Tenkeyless",
        family: Family::Standard,
        rank: 1,
    },
    FormFactor {
        name: "75% · Compact",
        family: Family::Standard,
        rank: 2,
    },
    FormFactor {
        name: "65%",
        family: Family::Standard,
        rank: 3,
    },
    FormFactor {
        name: "60%",
        family: Family::Standard,
        rank: 4,
    },
    FormFactor {
        name: "Apple · Compact",
        family: Family::Apple,
        rank: 3,
    },
    FormFactor {
        name: "Apple · Full-size",
        family: Family::Apple,
        rank: 0,
    },
];

/// Indexes into [`FORM_FACTORS`].
pub const FORM_FULL: usize = 0;
pub const FORM_TKL: usize = 1;
pub const FORM_SEVENTY_FIVE: usize = 2;
pub const FORM_SIXTY_FIVE: usize = 3;
pub const FORM_SIXTY: usize = 4;
pub const FORM_APPLE_COMPACT: usize = 5;
pub const FORM_APPLE_FULL: usize = 6;

/// The family of a form factor (the first for an index out of range).
pub fn family(form: usize) -> Family {
    FORM_FACTORS
        .get(form)
        .map_or(Family::Standard, |factor| factor.family)
}

/// The larger of two form factors by rank; on a tie, the one listed
/// first.
pub fn larger(a: usize, b: usize) -> usize {
    let rank = |form: usize| FORM_FACTORS.get(form).map_or(u8::MAX, |factor| factor.rank);
    if (rank(b), b) < (rank(a), a) { b } else { a }
}

/// Best-effort form factor guess from the keys a device reports.
///
/// Absence is meaningful — a device that doesn't report numpad scancodes
/// has no numpad — but presence is not: compact boards often report every
/// key their Fn layer can emit, which inflates the guess. Detection is a
/// starting point; the size picker always wins.
pub fn form_for_keys(numpad: bool, nav_cluster: bool, f_row: bool, arrows: bool) -> usize {
    if numpad {
        FORM_FULL
    } else if nav_cluster {
        FORM_TKL
    } else if f_row {
        FORM_SEVENTY_FIVE
    } else if arrows {
        FORM_SIXTY_FIVE
    } else {
        FORM_SIXTY
    }
}

/// Form factor hint from a device's marketing name.
///
/// Keyboards routinely encode their size (`NEO80`, `Q65`) or key count
/// (`GK61`, `K552-87`) in the product name, which is often more truthful
/// than the over-reported key capabilities.
pub fn form_for_name(name: &str) -> Option<usize> {
    name.split(|c: char| !c.is_ascii_digit())
        .find_map(|run| match run {
            "96" | "98" | "100" | "104" | "108" => Some(FORM_FULL),
            "80" | "87" | "88" => Some(FORM_TKL),
            "75" | "84" => Some(FORM_SEVENTY_FIVE),
            "65" | "66" | "67" | "68" => Some(FORM_SIXTY_FIVE),
            "60" | "61" => Some(FORM_SIXTY),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `FORM_*` index constants agree with the [`FORM_FACTORS`] order.
    #[test]
    fn form_indices_match() {
        assert_eq!(FORM_FACTORS.len(), 7);
        assert!(FORM_FACTORS[FORM_FULL].name.starts_with("100%"));
        assert!(FORM_FACTORS[FORM_TKL].name.starts_with("80%"));
        assert!(FORM_FACTORS[FORM_SEVENTY_FIVE].name.starts_with("75%"));
        assert!(FORM_FACTORS[FORM_SIXTY_FIVE].name.starts_with("65%"));
        assert!(FORM_FACTORS[FORM_SIXTY].name.starts_with("60%"));
        assert_eq!(FORM_FACTORS[FORM_APPLE_COMPACT].name, "Apple · Compact");
        assert_eq!(FORM_FACTORS[FORM_APPLE_FULL].name, "Apple · Full-size");
        for form in [
            FORM_FULL,
            FORM_TKL,
            FORM_SEVENTY_FIVE,
            FORM_SIXTY_FIVE,
            FORM_SIXTY,
        ] {
            assert_eq!(family(form), Family::Standard);
        }
        assert_eq!(family(FORM_APPLE_COMPACT), Family::Apple);
        assert_eq!(family(FORM_APPLE_FULL), Family::Apple);
        assert_eq!(family(usize::MAX), Family::Standard);
    }

    /// Larger boards win regardless of family; the standard sizes'
    /// index order is their size order, so name hints can shrink by
    /// taking the larger index.
    #[test]
    fn form_factors_compare_by_size() {
        assert_eq!(larger(FORM_TKL, FORM_FULL), FORM_FULL);
        assert_eq!(larger(FORM_SIXTY, FORM_SIXTY_FIVE), FORM_SIXTY_FIVE);
        assert_eq!(larger(FORM_APPLE_COMPACT, FORM_APPLE_FULL), FORM_APPLE_FULL);
        assert_eq!(larger(FORM_APPLE_FULL, FORM_TKL), FORM_APPLE_FULL);
        assert_eq!(larger(FORM_SIXTY, FORM_APPLE_COMPACT), FORM_APPLE_COMPACT);
        assert_eq!(
            larger(FORM_APPLE_FULL, FORM_FULL),
            FORM_FULL,
            "a tie keeps the order listed"
        );
        assert_eq!(larger(FORM_FULL, FORM_APPLE_FULL), FORM_FULL);
        assert_eq!(larger(usize::MAX, FORM_SIXTY), FORM_SIXTY);
        let standard = [
            FORM_FULL,
            FORM_TKL,
            FORM_SEVENTY_FIVE,
            FORM_SIXTY_FIVE,
            FORM_SIXTY,
        ];
        for pair in standard.windows(2) {
            assert_eq!(larger(pair[0], pair[1]), pair[0]);
            assert!(FORM_FACTORS[pair[0]].rank < FORM_FACTORS[pair[1]].rank);
        }
    }

    /// The capability cascade maps to the expected sizes.
    #[test]
    fn guesses_form_factors() {
        assert_eq!(form_for_keys(true, true, true, true), FORM_FULL);
        assert_eq!(form_for_keys(false, true, true, true), FORM_TKL);
        assert_eq!(form_for_keys(false, false, true, true), FORM_SEVENTY_FIVE);
        assert_eq!(form_for_keys(false, false, false, true), FORM_SIXTY_FIVE);
        assert_eq!(form_for_keys(false, false, false, false), FORM_SIXTY);
    }

    /// Device names hint at sizes; unrelated digits don't.
    #[test]
    fn guesses_forms_from_names() {
        assert_eq!(form_for_name("@HFD NEO80 Keyboard"), Some(FORM_TKL));
        assert_eq!(form_for_name("Keychron Q65"), Some(FORM_SIXTY_FIVE));
        assert_eq!(form_for_name("SKYLOONG GK61"), Some(FORM_SIXTY));
        assert_eq!(form_for_name("Redragon K552-87"), Some(FORM_TKL));
        assert_eq!(form_for_name("Corsair K100"), Some(FORM_FULL));
        assert_eq!(form_for_name("Epomaker TH84"), Some(FORM_SEVENTY_FIVE));
        assert_eq!(form_for_name("TESmart DKS202-P24"), None);
        assert_eq!(form_for_name("Logitech MX Keys"), None);
        assert_eq!(form_for_name(""), None);
    }
}
