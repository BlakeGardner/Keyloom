//! Form-factor and layout-variant detection for the on-screen keyboard.
//!
//! Two independent axes describe the rendered deck:
//!
//! * A [`FormFactor`] (physical size, 100% down to 60%). [`form_for_keys`]
//!   guesses it from the keys a device reports and [`form_for_name`] from
//!   size hints in the device's marketing name.
//! * A variant (ANSI or ISO), detected per device from its evdev capabilities
//!   by the keyboard monitor.
//!
//! Detection only picks defaults — the size and layout pickers in the UI
//! always win. See `docs/Form_Factor_Detection.md` for the full story.

/// A physical keyboard size, from full-size (100%) down to 60%.
pub struct FormFactor {
    /// Human-readable name shown in the size picker.
    pub name: &'static str,
}

/// All supported form factors, largest first.
pub static FORM_FACTORS: &[FormFactor] = &[
    FormFactor {
        name: "100% · Full-size",
    },
    FormFactor {
        name: "80% · Tenkeyless",
    },
    FormFactor {
        name: "75% · Compact",
    },
    FormFactor { name: "65%" },
    FormFactor { name: "60%" },
];

/// Indexes into [`FORM_FACTORS`].
pub const FORM_FULL: usize = 0;
pub const FORM_TKL: usize = 1;
pub const FORM_SEVENTY_FIVE: usize = 2;
pub const FORM_SIXTY_FIVE: usize = 3;
pub const FORM_SIXTY: usize = 4;

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
        assert_eq!(FORM_FACTORS.len(), 5);
        assert!(FORM_FACTORS[FORM_FULL].name.starts_with("100%"));
        assert!(FORM_FACTORS[FORM_TKL].name.starts_with("80%"));
        assert!(FORM_FACTORS[FORM_SEVENTY_FIVE].name.starts_with("75%"));
        assert!(FORM_FACTORS[FORM_SIXTY_FIVE].name.starts_with("65%"));
        assert!(FORM_FACTORS[FORM_SIXTY].name.starts_with("60%"));
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
