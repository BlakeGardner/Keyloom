//! Keyboards Keyloom recognises by their identifiers, and what it knows
//! about them beyond what evdev reports.
//!
//! The generic detection (`keyboard.rs`, `monitor.rs`) guesses a size
//! from the keys a device reports and the digits in its name. That
//! guess is wrong for keyboards whose driver advertises every key their
//! fn layer can reach: a compact Magic Keyboard claims a numeric keypad
//! and the ISO key. A keyboard listed here is drawn from the table
//! instead: which deck, which generation of legends, which key sits in
//! its corner. The physical variant (ANSI, ISO, JIS) of such a keyboard
//! comes from the HID country code the kernel exposes beside its event
//! node, unless the product id fixes it.
//!
//! Adding a model is adding a row to [`KNOWN_KEYBOARDS`] and the same
//! row to `docs/Supported_Keyboards.md`; a new shape needs a deck of its
//! own in `ui::model` and a form factor in `keyboard`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::keyboard::{FORM_APPLE_COMPACT, FORM_APPLE_FULL};

/// Apple's vendor id over USB.
pub const APPLE_USB: u16 = 0x05ac;
/// Apple's vendor id over Bluetooth.
pub const APPLE_BLUETOOTH: u16 = 0x004c;

/// Where the recognised keyboards' vendor ids are seen: Apple's
/// keyboards report one id over USB and another over Bluetooth.
const APPLE_VENDORS: &[u16] = &[APPLE_USB, APPLE_BLUETOOTH];

/// A keyboard model recognised by its vendor and product id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownKeyboard {
    /// The vendor ids the product id is seen under.
    pub vendors: &'static [u16],
    pub product: u16,
    /// The marketing name, with the model number(s) it was sold under.
    pub model: &'static str,
    /// The deck to draw: an index into [`crate::keyboard::FORM_FACTORS`].
    pub form: usize,
    /// The physical variant, when the product id fixes it; otherwise
    /// the HID country code decides.
    pub variant: Option<Variant>,
    /// What the deck needs beyond its size.
    pub details: Details,
}

/// The parts of a recognised keyboard's deck that its size does not
/// say. One variant per family of keyboards Keyloom draws specially.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Details {
    Apple(AppleModel),
}

/// An Apple keyboard as printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppleModel {
    pub legends: AppleLegends,
    pub corner: Corner,
    /// The fn (globe) key is the first key of the bottom row. On the
    /// full-size keyboards before the USB-C generation it is the first
    /// key of the navigation cluster instead, and the bottom row starts
    /// with a wider control key.
    pub fn_at_left: bool,
}

/// Which generation of legends the function row prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppleLegends {
    /// Launchpad on F4, blank F5 and F6 (the 2015 design, still sold as
    /// the Magic Keyboard with Numeric Keypad).
    Classic,
    /// Spotlight, Dictation, and Do Not Disturb on F4 to F6 (2021 on).
    Modern,
}

/// The key in the top-right corner of the main block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    Eject,
    Lock,
    TouchId,
}

/// A keyboard's physical variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    #[default]
    Ansi,
    Iso,
    Jis,
}

impl Variant {
    /// The variant a HID country code implies. The codes are the
    /// `bCountryCode` values of the HID specification: 13 is the
    /// ISO keyboard as such, and the European countries sell
    /// ISO-shaped keyboards; 15 is Japan; the US (33), Latin America,
    /// and an unspecified country (0) are ANSI.
    pub fn from_hid_country(code: u8) -> Self {
        match code {
            2 | 5..=14 | 18 | 19 | 22..=29 | 31 | 32 | 35 => Self::Iso,
            15 => Self::Jis,
            _ => Self::Ansi,
        }
    }

    pub fn is_iso(self) -> bool {
        self == Self::Iso
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ansi => "ANSI",
            Self::Iso => "ISO",
            Self::Jis => "JIS",
        })
    }
}

const fn apple(
    product: u16,
    model: &'static str,
    form: usize,
    legends: AppleLegends,
    corner: Corner,
    fn_at_left: bool,
) -> KnownKeyboard {
    KnownKeyboard {
        vendors: APPLE_VENDORS,
        product,
        model,
        form,
        variant: None,
        details: Details::Apple(AppleModel {
            legends,
            corner,
            fn_at_left,
        }),
    }
}

/// Every keyboard Keyloom recognises. The product ids are the ones the
/// kernel's `hid_apple` driver lists (`drivers/hid/hid-ids.h`); the
/// model numbers are Apple's.
pub static KNOWN_KEYBOARDS: &[KnownKeyboard] = &[
    apple(
        0x0267,
        "Magic Keyboard (2015, A1644)",
        FORM_APPLE_COMPACT,
        AppleLegends::Classic,
        Corner::Eject,
        true,
    ),
    apple(
        0x026c,
        "Magic Keyboard with Numeric Keypad (2017, A1843)",
        FORM_APPLE_FULL,
        AppleLegends::Classic,
        Corner::Eject,
        false,
    ),
    apple(
        0x029c,
        "Magic Keyboard (2021, A2450)",
        FORM_APPLE_COMPACT,
        AppleLegends::Modern,
        Corner::Lock,
        true,
    ),
    apple(
        0x029a,
        "Magic Keyboard with Touch ID (2021, A2449)",
        FORM_APPLE_COMPACT,
        AppleLegends::Modern,
        Corner::TouchId,
        true,
    ),
    apple(
        0x029f,
        "Magic Keyboard with Touch ID and Numeric Keypad (2021, A2520)",
        FORM_APPLE_FULL,
        AppleLegends::Modern,
        Corner::TouchId,
        false,
    ),
    apple(
        0x0320,
        "Magic Keyboard (USB-C, 2024, A3203)",
        FORM_APPLE_COMPACT,
        AppleLegends::Modern,
        Corner::Lock,
        true,
    ),
    apple(
        0x0321,
        "Magic Keyboard with Touch ID (USB-C, 2024, A3118)",
        FORM_APPLE_COMPACT,
        AppleLegends::Modern,
        Corner::TouchId,
        true,
    ),
    apple(
        0x0322,
        "Magic Keyboard with Touch ID and Numeric Keypad (USB-C, 2024, A3119)",
        FORM_APPLE_FULL,
        AppleLegends::Modern,
        Corner::TouchId,
        true,
    ),
];

/// An Apple keyboard whose product id is not listed: drawn as the
/// current compact keyboard, or as the full-size one when its name
/// says so.
static APPLE_UNLISTED_COMPACT: KnownKeyboard = apple(
    0,
    "Apple keyboard (model not listed)",
    FORM_APPLE_COMPACT,
    AppleLegends::Modern,
    Corner::TouchId,
    true,
);
static APPLE_UNLISTED_FULL: KnownKeyboard = apple(
    0,
    "Apple keyboard with numeric keypad (model not listed)",
    FORM_APPLE_FULL,
    AppleLegends::Modern,
    Corner::TouchId,
    true,
);

/// Keyboards that present Apple's vendor id without being Apple's,
/// by the start of their name: the same list the kernel's `hid_apple`
/// driver keeps to treat them as ordinary keyboards. They are guessed
/// like any other keyboard.
const NOT_APPLE: &[&str] = &[
    "SONiX KN85 Keyboard",
    "SONiX USB DEVICE",
    "SONiX AK870 PRO",
    "Keychron",
    "AONE",
    "GANSS",
    "Hailuck",
    "Jamesdonkey",
    "A3R",
    "hfd.cn",
    "WKB603",
    "TH87",
    "HFD Epomaker TH87",
    "2.4G Wireless Receiver",
    "Thock TKL Wireless",
    "USB Dongle",
];

/// The keyboard a device's identifiers and name describe, if Keyloom
/// recognises it.
pub fn identify(vendor: u16, product: u16, name: &str) -> Option<&'static KnownKeyboard> {
    if let Some(known) = KNOWN_KEYBOARDS
        .iter()
        .find(|known| known.product == product && known.vendors.contains(&vendor))
    {
        return Some(known);
    }
    if !APPLE_VENDORS.contains(&vendor) || NOT_APPLE.iter().any(|clone| name.starts_with(clone)) {
        return None;
    }
    Some(if name.contains("Numeric Keypad") {
        &APPLE_UNLISTED_FULL
    } else {
        &APPLE_UNLISTED_COMPACT
    })
}

/// How the kernel's `hid_apple` driver is configured, from its module
/// parameters (`/sys/module/hid_apple/parameters`). It translates the
/// function row and can swap modifier keys in the kernel, before any
/// program sees the keys, so the deck has to know its settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppleDriver {
    /// 0: the function row always sends F keys and fn does nothing;
    /// 1: media keys, F keys with fn held; 2: the other way round;
    /// 3 (the default): 1 on Apple's own keyboards; 4: media keys only.
    pub fnmode: u8,
    /// -1 (the default): swap the two keys an ISO keyboard reports
    /// crosswise when its country code says ISO; 0: never; 1: always.
    pub iso_layout: i8,
    /// 0: as printed; 1: option and command swapped; 2: swapped on the
    /// left side only.
    pub swap_opt_cmd: u8,
    pub swap_ctrl_cmd: bool,
    pub swap_fn_leftctrl: bool,
}

impl Default for AppleDriver {
    fn default() -> Self {
        Self {
            fnmode: 3,
            iso_layout: -1,
            swap_opt_cmd: 0,
            swap_ctrl_cmd: false,
            swap_fn_leftctrl: false,
        }
    }
}

impl AppleDriver {
    /// The parameters as the running kernel has them, defaults for any
    /// that cannot be read.
    pub fn read() -> Self {
        Self::read_from(Path::new("/sys/module/hid_apple/parameters"))
    }

    /// [`Self::read`] against a parameters directory.
    pub fn read_from(dir: &Path) -> Self {
        let value = |name: &str| fs::read_to_string(dir.join(name)).ok();
        Self::parse(value)
    }

    /// The parameters from their textual values, defaults for any that
    /// are absent or unreadable.
    pub fn parse(value: impl Fn(&str) -> Option<String>) -> Self {
        let defaults = Self::default();
        let number = |name: &str| value(name).and_then(|text| text.trim().parse::<i64>().ok());
        Self {
            fnmode: number("fnmode")
                .and_then(|n| u8::try_from(n).ok())
                .unwrap_or(defaults.fnmode),
            iso_layout: number("iso_layout")
                .and_then(|n| i8::try_from(n).ok())
                .unwrap_or(defaults.iso_layout),
            swap_opt_cmd: number("swap_opt_cmd")
                .and_then(|n| u8::try_from(n).ok())
                .unwrap_or(defaults.swap_opt_cmd),
            swap_ctrl_cmd: number("swap_ctrl_cmd").is_some_and(|n| n != 0),
            swap_fn_leftctrl: number("swap_fn_leftctrl").is_some_and(|n| n != 0),
        }
    }

    /// Whether the function row sends media keys unless fn is held.
    pub fn media_keys_first(&self) -> bool {
        matches!(self.fnmode, 1 | 3 | 4)
    }

    /// Whether the driver swaps the codes of the two keys an ISO Apple
    /// keyboard reports crosswise (the key in the top-left corner and
    /// the one beside the left shift).
    pub fn iso_swapped(&self, variant: Variant) -> bool {
        self.iso_layout > 0 || (self.iso_layout < 0 && variant.is_iso())
    }
}

/// A connected keyboard Keyloom recognised, with what was read about
/// it when it was seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recognized {
    pub keyboard: &'static KnownKeyboard,
    pub variant: Variant,
    /// The `hid_apple` driver's settings, when that driver has the
    /// keyboard. `None` with another driver, which reports the
    /// function row as plain F keys and no fn key.
    pub apple_driver: Option<AppleDriver>,
}

impl Recognized {
    /// A recognised keyboard as its drivers present it, from the event
    /// node's entries under `/sys/class/input`.
    pub fn observe(keyboard: &'static KnownKeyboard, event: &Path) -> Self {
        Self::observe_in(keyboard, Path::new("/sys/class/input"), event)
    }

    /// [`Self::observe`] against an explicit `/sys/class/input`.
    pub fn observe_in(
        keyboard: &'static KnownKeyboard,
        sys_class_input: &Path,
        event: &Path,
    ) -> Self {
        let hid = hid_device_dir(sys_class_input, event);
        let variant = keyboard.variant.unwrap_or_else(|| {
            hid.as_deref()
                .and_then(hid_country)
                .map_or(Variant::Ansi, Variant::from_hid_country)
        });
        let apple_driver = match keyboard.details {
            Details::Apple(_) => hid
                .as_deref()
                .and_then(driver_name)
                .filter(|driver| driver == "apple")
                .map(|_| AppleDriver::read()),
        };
        Self {
            keyboard,
            variant,
            apple_driver,
        }
    }

    /// The Apple keyboard this is, if it is one.
    pub fn apple(&self) -> Option<&AppleModel> {
        match &self.keyboard.details {
            Details::Apple(model) => Some(model),
        }
    }
}

/// The HID device an event node belongs to: `/sys/class/input/eventN`
/// leads to its input device, whose parent is the HID device with the
/// `country` and `driver` entries. `None` for a node without one, such
/// as a virtual keyboard.
fn hid_device_dir(sys_class_input: &Path, event: &Path) -> Option<PathBuf> {
    let name = event.file_name()?;
    let dir = sys_class_input.join(name).join("device").join("device");
    dir.join("country").is_file().then_some(dir)
}

/// The HID country code, which sysfs prints in hex.
fn hid_country(hid: &Path) -> Option<u8> {
    let text = fs::read_to_string(hid.join("country")).ok()?;
    u8::from_str_radix(text.trim(), 16).ok()
}

/// The name of the kernel driver bound to a HID device.
fn driver_name(hid: &Path) -> Option<String> {
    let target = fs::read_link(hid.join("driver")).ok()?;
    Some(target.file_name()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    #[test]
    fn magic_keyboards_are_recognised_over_usb_and_bluetooth() {
        let touch_id = identify(APPLE_BLUETOOTH, 0x029a, "Blake’s Magic Keyboard").unwrap();
        assert_eq!(touch_id.form, FORM_APPLE_COMPACT);
        assert_eq!(
            touch_id.details,
            Details::Apple(AppleModel {
                legends: AppleLegends::Modern,
                corner: Corner::TouchId,
                fn_at_left: true,
            })
        );
        assert_eq!(
            identify(APPLE_USB, 0x029a, "Magic Keyboard with Touch ID"),
            Some(touch_id),
            "the same product id over USB is the same keyboard"
        );
        let keypad = identify(APPLE_USB, 0x026c, "Magic Keyboard with Numeric Keypad").unwrap();
        assert_eq!(keypad.form, FORM_APPLE_FULL);
        assert_eq!(
            keypad.details,
            Details::Apple(AppleModel {
                legends: AppleLegends::Classic,
                corner: Corner::Eject,
                fn_at_left: false,
            })
        );
        assert_eq!(identify(0xfffe, 0x0036, "@HFD NEO80 Keyboard"), None);
    }

    #[test]
    fn unlisted_apple_products_fall_back_by_name_and_clones_are_not_apple() {
        let unlisted = identify(APPLE_BLUETOOTH, 0x0999, "Someone’s Magic Keyboard").unwrap();
        assert_eq!(unlisted.form, FORM_APPLE_COMPACT);
        let full = identify(APPLE_USB, 0x0999, "Magic Keyboard with Numeric Keypad").unwrap();
        assert_eq!(full.form, FORM_APPLE_FULL);
        for clone in ["Keychron K2", "SONiX USB DEVICE", "USB Dongle"] {
            assert_eq!(
                identify(APPLE_USB, 0x024f, clone),
                None,
                "{clone} borrows Apple's vendor id"
            );
        }
    }

    #[test]
    fn every_listed_product_id_is_unique() {
        for (i, a) in KNOWN_KEYBOARDS.iter().enumerate() {
            for b in &KNOWN_KEYBOARDS[i + 1..] {
                assert!(
                    a.product != b.product || a.vendors != b.vendors,
                    "{} and {} share a product id",
                    a.model,
                    b.model
                );
            }
            assert!(a.product != 0, "{} has no product id", a.model);
        }
    }

    #[test]
    fn hid_country_codes_map_to_variants() {
        assert_eq!(Variant::from_hid_country(0x21), Variant::Ansi);
        assert_eq!(Variant::from_hid_country(0x0d), Variant::Iso);
        assert_eq!(Variant::from_hid_country(0x0f), Variant::Jis);
        assert_eq!(Variant::from_hid_country(0), Variant::Ansi);
        assert_eq!(Variant::from_hid_country(32), Variant::Iso, "UK");
        assert_eq!(Variant::from_hid_country(9), Variant::Iso, "German");
        assert_eq!(
            Variant::from_hid_country(17),
            Variant::Ansi,
            "Latin American"
        );
        assert_eq!(Variant::Iso.to_string(), "ISO");
    }

    #[test]
    fn driver_parameters_parse_with_defaults_for_the_unreadable() {
        let defaults = AppleDriver::parse(|_| None);
        assert_eq!(defaults, AppleDriver::default());
        assert!(defaults.media_keys_first());
        assert!(defaults.iso_swapped(Variant::Iso));
        assert!(!defaults.iso_swapped(Variant::Ansi));

        let pc_style = AppleDriver::parse(|name| {
            Some(
                match name {
                    "fnmode" => "2\n",
                    "iso_layout" => "0\n",
                    "swap_opt_cmd" => "1\n",
                    "swap_ctrl_cmd" => "0\n",
                    "swap_fn_leftctrl" => "1\n",
                    _ => return None,
                }
                .to_owned(),
            )
        });
        assert_eq!(
            pc_style,
            AppleDriver {
                fnmode: 2,
                iso_layout: 0,
                swap_opt_cmd: 1,
                swap_ctrl_cmd: false,
                swap_fn_leftctrl: true,
            }
        );
        assert!(!pc_style.media_keys_first());
        assert!(!pc_style.iso_swapped(Variant::Iso));
        assert!(AppleDriver::parse(|_| Some("garbage".to_owned())) == AppleDriver::default());
        assert!(
            AppleDriver::parse(|name| (name == "fnmode").then(|| "4".to_owned()))
                .media_keys_first()
        );
        assert!(
            !AppleDriver::parse(|name| (name == "fnmode").then(|| "0".to_owned()))
                .media_keys_first()
        );
        assert!(
            AppleDriver::parse(|name| (name == "iso_layout").then(|| "1".to_owned()))
                .iso_swapped(Variant::Ansi)
        );
    }

    #[test]
    fn driver_parameters_read_from_a_directory() {
        let temp = TempDir::new("hid-apple-params");
        std::fs::write(temp.path().join("fnmode"), "2\n").unwrap();
        std::fs::write(temp.path().join("swap_ctrl_cmd"), "1\n").unwrap();
        let driver = AppleDriver::read_from(temp.path());
        assert_eq!(driver.fnmode, 2);
        assert!(driver.swap_ctrl_cmd);
        assert_eq!(
            driver.iso_layout, -1,
            "an absent parameter keeps its default"
        );
        assert_eq!(
            AppleDriver::read_from(&temp.path().join("missing")),
            AppleDriver::default()
        );
    }

    /// Build `/sys/class/input/eventN/device/device` with a HID
    /// device's `country` file and `driver` link.
    fn fake_sysfs(label: &str, country: &str, driver: Option<&str>) -> (TempDir, PathBuf) {
        let temp = TempDir::new(label);
        let hid = temp.path().join("hid");
        std::fs::create_dir_all(&hid).unwrap();
        std::fs::write(hid.join("country"), country).unwrap();
        if let Some(driver) = driver {
            let drivers = temp.path().join("drivers");
            std::fs::create_dir_all(drivers.join(driver)).unwrap();
            std::os::unix::fs::symlink(drivers.join(driver), hid.join("driver")).unwrap();
        }
        let input = temp.path().join("class").join("input");
        let event = input.join("event20");
        std::fs::create_dir_all(&event).unwrap();
        // eventN/device -> the input device, whose device -> the HID device.
        let input_device = temp.path().join("input29");
        std::fs::create_dir_all(&input_device).unwrap();
        std::os::unix::fs::symlink(&hid, input_device.join("device")).unwrap();
        std::os::unix::fs::symlink(&input_device, event.join("device")).unwrap();
        (temp, input)
    }

    #[test]
    fn a_recognised_keyboard_is_observed_through_sysfs() {
        let touch_id = identify(APPLE_BLUETOOTH, 0x029a, "Magic Keyboard").unwrap();
        let (_temp, input) = fake_sysfs("sysfs-iso", "0d\n", Some("apple"));
        let seen = Recognized::observe_in(touch_id, &input, Path::new("/dev/input/event20"));
        assert_eq!(seen.variant, Variant::Iso);
        assert!(seen.apple_driver.is_some());
        assert!(seen.apple().is_some());

        let (_temp, input) = fake_sysfs("sysfs-generic", "21\n", Some("hid-generic"));
        let seen = Recognized::observe_in(touch_id, &input, Path::new("/dev/input/event20"));
        assert_eq!(seen.variant, Variant::Ansi);
        assert_eq!(
            seen.apple_driver, None,
            "another driver than hid_apple sends plain F keys"
        );

        let (_temp, input) = fake_sysfs("sysfs-nodriver", "0f\n", None);
        let seen = Recognized::observe_in(touch_id, &input, Path::new("/dev/input/event20"));
        assert_eq!(seen.variant, Variant::Jis);
        assert_eq!(seen.apple_driver, None);

        let seen = Recognized::observe_in(
            touch_id,
            Path::new("/keyloom-no-such-sysfs"),
            Path::new("/dev/input/event20"),
        );
        assert_eq!(seen.variant, Variant::Ansi, "no sysfs: ANSI");
        assert_eq!(seen.apple_driver, None);
    }
}
