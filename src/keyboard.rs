//! Form-factor and layout-variant detection for the on-screen keyboard.
//!
//! Two independent axes describe the rendered deck:
//!
//! * A [`FormFactor`] (physical size, 100% down to 60%). [`form_for_keys`]
//!   guesses it from the keys a device reports and [`form_for_name`] from
//!   size hints in the device's marketing name.
//! * A variant (ANSI or ISO). [`detect_iso`] guesses it from the system's
//!   configured XKB layout.
//!
//! Detection only picks defaults — the size and layout pickers in the UI
//! always win. See `docs/Form_Factor_Detection.md` for the full story.

use std::path::PathBuf;

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

/// XKB layout names that use the ISO physical assembly (102nd key, tall
/// Enter, AltGr) rather than ANSI. `us` and its variants stay ANSI.
static ISO_XKB_LAYOUTS: &[&str] = &[
    "gb", "ie", "de", "fr", "es", "pt", "it", "nl", "be", "dk", "no", "se", "fi", "is", "ch", "at",
    "pl", "cz", "sk", "hu", "ro", "hr", "si", "ee", "lv", "lt", "gr", "tr",
];

/// Best-effort detection of the physical layout variant (ANSI vs ISO)
/// from the system's configured XKB layout.
///
/// Sources, in order: the COSMIC compositor configuration, the
/// `XKB_DEFAULT_LAYOUT` environment, and `/etc/default/keyboard`.
/// Returns `None` when nothing is configured.
pub fn detect_iso() -> Option<bool> {
    let (layout, _) = cosmic_config_xkb()
        .or_else(env_xkb)
        .or_else(etc_default_xkb)?;
    Some(layout_is_iso(&layout))
}

/// Whether an XKB layout string (possibly a comma-separated list, as
/// configured) names an ISO-assembly layout. The first entry wins.
fn layout_is_iso(layout: &str) -> bool {
    let layout = layout.split(',').next().unwrap_or_default().trim();
    ISO_XKB_LAYOUTS.contains(&layout)
}

/// The layout configured for the COSMIC compositor, if any.
fn cosmic_config_xkb() -> Option<(String, String)> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    let path = base.join("cosmic/com.system76.CosmicComp/v1/xkb_config");
    parse_cosmic_xkb(&std::fs::read_to_string(path).ok()?)
}

fn parse_cosmic_xkb(text: &str) -> Option<(String, String)> {
    let layout = ron_str_field(text, "layout").filter(|layout| !layout.trim().is_empty())?;
    let variant = ron_str_field(text, "variant").unwrap_or_default();
    Some((layout, variant))
}

/// Extract a `name: "value"` string field from a RON document, without
/// pulling in a full RON parser.
fn ron_str_field(text: &str, name: &str) -> Option<String> {
    let mut rest = text;

    while let Some(pos) = rest.find(name) {
        let preceded = rest[..pos].chars().next_back();
        let candidate = &rest[pos + name.len()..];
        rest = candidate;

        // Reject matches inside longer identifiers (e.g. `my_layout`).
        if preceded.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }

        let Some(after_colon) = candidate.trim_start().strip_prefix(':') else {
            continue;
        };

        // Non-string values (e.g. `options: None`) are not ours to parse.
        let Some(value) = after_colon.trim_start().strip_prefix('"') else {
            continue;
        };

        return value.find('"').map(|end| value[..end].to_owned());
    }

    None
}

/// The layout from the `XKB_DEFAULT_*` environment (honored by wlroots
/// compositors and others), if set.
fn env_xkb() -> Option<(String, String)> {
    let layout = std::env::var("XKB_DEFAULT_LAYOUT")
        .ok()
        .filter(|layout| !layout.trim().is_empty())?;
    let variant = std::env::var("XKB_DEFAULT_VARIANT").unwrap_or_default();
    Some((layout, variant))
}

/// The system-wide layout from `/etc/default/keyboard`, if present.
fn etc_default_xkb() -> Option<(String, String)> {
    parse_etc_default(&std::fs::read_to_string("/etc/default/keyboard").ok()?)
}

fn parse_etc_default(text: &str) -> Option<(String, String)> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let value = line
                .trim()
                .strip_prefix(name)?
                .trim_start()
                .strip_prefix('=')?;
            Some(
                value
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_owned(),
            )
        })
    };

    let layout = field("XKBLAYOUT").filter(|layout| !layout.trim().is_empty())?;
    Some((layout, field("XKBVARIANT").unwrap_or_default()))
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

    /// XKB layout names map to the right physical assembly.
    #[test]
    fn matches_xkb_names() {
        assert!(!layout_is_iso("us"));
        assert!(layout_is_iso("gb"));
        assert!(layout_is_iso("de"));
        // Multiple configured layouts: the first wins.
        assert!(layout_is_iso("fr,us"));
        assert!(!layout_is_iso("us,de"));
        // Unknown layouts default to ANSI.
        assert!(!layout_is_iso("xx"));
        assert!(!layout_is_iso(""));
    }

    #[test]
    fn parses_cosmic_config() {
        let text = r#"(
    rules: "",
    model: "",
    layout: "de,us",
    variant: "nodeadkeys",
    options: None,
    repeat_delay: 400,
    repeat_rate: 45,
)"#;

        assert_eq!(
            parse_cosmic_xkb(text),
            Some(("de,us".to_owned(), "nodeadkeys".to_owned()))
        );

        // An unset layout is no detection at all.
        assert_eq!(parse_cosmic_xkb(r#"(layout: "", variant: "")"#), None);
    }

    #[test]
    fn parses_etc_default_keyboard() {
        let text = "# KEYBOARD CONFIGURATION FILE\nXKBMODEL=\"pc105\"\nXKBLAYOUT=\"gb\"\nXKBVARIANT=\"\"\nXKBOPTIONS=\"\"\n";
        assert_eq!(
            parse_etc_default(text),
            Some(("gb".to_owned(), String::new()))
        );
        assert_eq!(parse_etc_default("XKBMODEL=pc105\n"), None);
    }
}
