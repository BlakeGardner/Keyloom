//! Design tokens and shared styling ported from the design export
//! (`design/Keyboard Customization-controls-v2.dc.html`).
//!
//! The export specifies every color in OKLCH; [`oklch`] converts those
//! values to sRGB at runtime so the tokens below can mirror the design
//! stylesheet verbatim.

use cosmic::iced::gradient::Linear;
use cosmic::iced::{Background, Color, Gradient, Radians};
use cosmic::theme;
use cosmic::widget::button;

/// Convert an OKLCH color (lightness, chroma, hue in degrees) to sRGB.
pub fn oklch(l: f32, c: f32, h: f32) -> Color {
    oklcha(l, c, h, 1.0)
}

/// [`oklch`] with an explicit alpha channel.
pub fn oklcha(l: f32, c: f32, h: f32, alpha: f32) -> Color {
    let h = h.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());

    // OKLab -> non-linear LMS -> linear LMS (Björn Ottosson's reference).
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);

    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_93 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_4 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;

    Color {
        r: gamma(r),
        g: gamma(g),
        b: gamma(b),
        a: alpha,
    }
}

/// Linear-light sRGB to gamma-encoded sRGB, clamped to displayable range.
fn gamma(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    if u <= 0.003_130_8 {
        12.92 * u
    } else {
        1.055 * u.powf(1.0 / 2.4) - 0.055
    }
}

/// White with the given alpha, as used by the export's rgba() overlays.
pub fn white(alpha: f32) -> Color {
    Color::from_rgba(1.0, 1.0, 1.0, alpha)
}

/// Black with the given alpha, as used by the export's rgba() overlays.
pub fn black(alpha: f32) -> Color {
    Color::from_rgba(0.0, 0.0, 0.0, alpha)
}

// Core tokens (`:root` in the export).
pub fn bg() -> Color {
    oklch(0.145, 0.006, 152.0)
}
pub fn surface() -> Color {
    oklch(0.205, 0.007, 152.0)
}
pub fn fg() -> Color {
    oklch(0.93, 0.01, 152.0)
}
pub fn muted() -> Color {
    oklch(0.76, 0.01, 152.0)
}
pub fn border() -> Color {
    oklch(0.39, 0.008, 152.0)
}
pub fn accent() -> Color {
    oklch(0.78, 0.14, 152.0)
}

/// A top-to-bottom linear gradient, like `linear-gradient(top, bottom)`.
pub fn vgradient(top: Color, bottom: Color) -> Background {
    Background::Gradient(Gradient::Linear(
        Linear::new(Radians(std::f32::consts::PI))
            .add_stop(0.0, top)
            .add_stop(1.0, bottom),
    ))
}

/// Everything needed to describe one custom button appearance.
#[derive(Clone)]
pub struct ButtonStyle {
    pub bg: Option<Background>,
    pub hover_bg: Option<Background>,
    pub text: Color,
    pub hover_text: Option<Color>,
    pub border: Color,
    pub hover_border: Option<Color>,
    pub border_width: f32,
    pub radius: f32,
    /// Focus/selection ring drawn outside the border.
    pub outline: Option<(f32, Color)>,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            bg: None,
            hover_bg: None,
            text: fg(),
            hover_text: None,
            border: Color::TRANSPARENT,
            hover_border: None,
            border_width: 0.0,
            radius: 8.0,
            outline: None,
        }
    }
}

impl ButtonStyle {
    fn appearance(&self, hovered: bool) -> button::Style {
        let mut style = button::Style::new();
        style.background = if hovered {
            self.hover_bg.or(self.bg).map(|bg| brighten(bg, 1.08))
        } else {
            self.bg
        };
        style.text_color = Some(if hovered {
            self.hover_text.unwrap_or(self.text)
        } else {
            self.text
        });
        style.border_radius = self.radius.into();
        style.border_width = self.border_width;
        style.border_color = if hovered {
            self.hover_border.unwrap_or(self.border)
        } else {
            self.border
        };
        if let Some((width, color)) = self.outline {
            style.outline_width = width;
            style.outline_color = color;
        }
        style
    }

    /// Turn the description into a libcosmic button class.
    pub fn class(self) -> theme::Button {
        let normal = self.clone();
        let hover = self.clone();
        let press = self.clone();
        let disabled = self;
        theme::Button::Custom {
            active: Box::new(move |_, _| normal.appearance(false)),
            hovered: Box::new(move |_, _| hover.appearance(true)),
            pressed: Box::new(move |_, _| press.appearance(true)),
            disabled: Box::new(move |_| {
                let mut style = disabled.appearance(false);
                if let Some(color) = &mut style.text_color {
                    color.a *= 0.45;
                }
                if let Some(background) = &mut style.background {
                    *background = fade(*background, 0.45);
                }
                style.border_color.a *= 0.45;
                style
            }),
        }
    }
}

/// Approximate the export's `hover { filter: brightness(x) }`.
fn brighten(background: Background, factor: f32) -> Background {
    let lift = |color: Color| Color {
        r: (color.r * factor).min(1.0),
        g: (color.g * factor).min(1.0),
        b: (color.b * factor).min(1.0),
        a: color.a,
    };
    match background {
        Background::Color(color) => Background::Color(lift(color)),
        Background::Gradient(Gradient::Linear(mut linear)) => {
            for stop in linear.stops.iter_mut().flatten() {
                stop.color = lift(stop.color);
            }
            Background::Gradient(Gradient::Linear(linear))
        }
    }
}

/// Scale a background's alpha, for disabled states.
fn fade(background: Background, factor: f32) -> Background {
    match background {
        Background::Color(mut color) => {
            color.a *= factor;
            Background::Color(color)
        }
        Background::Gradient(gradient) => Background::Gradient(gradient.scale_alpha(factor)),
    }
}

/// `.quiet` buttons: neutral chrome used across the editor panels.
pub fn quiet(pressed: bool) -> theme::Button {
    ButtonStyle {
        bg: Some(Background::Color(if pressed {
            oklch(0.30, 0.04, 152.0)
        } else {
            oklch(0.26, 0.008, 152.0)
        })),
        hover_bg: Some(Background::Color(oklch(0.32, 0.008, 152.0))),
        text: oklch(0.95, 0.01, 152.0),
        hover_text: Some(oklch(0.98, 0.01, 152.0)),
        border: if pressed { accent() } else { border() },
        border_width: 1.0,
        radius: 8.0,
        ..Default::default()
    }
    .class()
}

/// Header/tab-bar tab: active tabs get the raised pill.
pub fn tab(active: bool) -> theme::Button {
    if active {
        ButtonStyle {
            bg: Some(Background::Color(oklch(0.36, 0.03, 152.0))),
            text: oklch(0.97, 0.01, 152.0),
            radius: 7.0,
            ..Default::default()
        }
        .class()
    } else {
        ButtonStyle {
            text: muted(),
            hover_text: Some(fg()),
            radius: 7.0,
            ..Default::default()
        }
        .class()
    }
}

/// Small translucent header buttons (profile picker, `⋯` menu).
pub fn header_chip() -> theme::Button {
    ButtonStyle {
        bg: Some(Background::Color(white(0.05))),
        hover_bg: Some(Background::Color(white(0.10))),
        text: oklch(0.9, 0.01, 152.0),
        border: white(0.10),
        border_width: 1.0,
        radius: 8.0,
        ..Default::default()
    }
    .class()
}

/// Selectable chips (layer picker, category filters, modifier toggles).
pub fn chip(active: bool) -> theme::Button {
    if active {
        ButtonStyle {
            bg: Some(Background::Color(oklch(0.32, 0.05, 152.0))),
            text: oklch(0.97, 0.02, 152.0),
            border: accent(),
            border_width: 1.0,
            radius: 8.0,
            ..Default::default()
        }
        .class()
    } else {
        ButtonStyle {
            bg: Some(Background::Color(white(0.035))),
            text: oklch(0.72, 0.01, 152.0),
            hover_text: Some(fg()),
            border: white(0.09),
            border_width: 1.0,
            radius: 8.0,
            ..Default::default()
        }
        .class()
    }
}

/// Rows inside the profile/device/menu popovers.
pub fn menu_row(active: bool) -> theme::Button {
    if active {
        ButtonStyle {
            bg: Some(Background::Color(oklch(0.32, 0.04, 152.0))),
            text: oklch(0.97, 0.01, 152.0),
            radius: 9.0,
            ..Default::default()
        }
        .class()
    } else {
        ButtonStyle {
            hover_bg: Some(Background::Color(white(0.08))),
            text: oklch(0.88, 0.01, 152.0),
            radius: 9.0,
            ..Default::default()
        }
        .class()
    }
}

/// Accented call-to-action buttons (`+ New group`, onboarding CTA).
pub fn accent_button() -> theme::Button {
    ButtonStyle {
        bg: Some(Background::Color(oklch(0.3, 0.04, 152.0))),
        hover_bg: Some(Background::Color(oklch(0.36, 0.06, 152.0))),
        text: oklch(0.93, 0.02, 152.0),
        border: oklch(0.45, 0.07, 152.0),
        border_width: 1.0,
        radius: 9.0,
        ..Default::default()
    }
    .class()
}

/// Subtle translucent buttons (Skip setup, Undo, Delete shortcut, …).
pub fn ghost_button() -> theme::Button {
    ButtonStyle {
        bg: Some(Background::Color(white(0.04))),
        hover_bg: Some(Background::Color(white(0.09))),
        text: oklch(0.85, 0.01, 152.0),
        border: white(0.10),
        border_width: 1.0,
        radius: 9.0,
        ..Default::default()
    }
    .class()
}

/// Output keycaps in the editor's action grid (`.output-keycap`):
/// tactile keys that distinguish "remap to a key" from app controls.
pub fn keycap(active: bool) -> theme::Button {
    ButtonStyle {
        bg: Some(vgradient(
            oklch(0.325, 0.007, 152.0),
            oklch(0.275, 0.007, 152.0),
        )),
        hover_bg: Some(vgradient(
            oklch(0.265, 0.007, 152.0),
            oklch(0.215, 0.007, 152.0),
        )),
        text: if active {
            oklch(0.98, 0.01, 152.0)
        } else {
            fg()
        },
        hover_text: Some(oklch(0.99, 0.01, 152.0)),
        border: if active {
            accent()
        } else {
            oklch(0.46, 0.007, 152.0)
        },
        hover_border: Some(if active { accent() } else { fg() }),
        border_width: 1.0,
        radius: 6.0,
        ..Default::default()
    }
    .class()
}

/// Flat category tabs in the editor (`.category-filter`).
pub fn flat_tab() -> theme::Button {
    ButtonStyle {
        hover_bg: Some(Background::Color(oklch(0.245, 0.007, 152.0))),
        text: fg(),
        hover_text: Some(oklch(0.99, 0.01, 152.0)),
        radius: 4.0,
        ..Default::default()
    }
    .class()
}

/// Outlined app control (`.editor-record`).
pub fn outline_button() -> theme::Button {
    ButtonStyle {
        hover_bg: Some(Background::Color(oklch(0.245, 0.007, 152.0))),
        text: fg(),
        hover_text: Some(oklch(0.99, 0.01, 152.0)),
        border: oklch(0.6, 0.008, 152.0),
        hover_border: Some(fg()),
        border_width: 1.0,
        radius: 8.0,
        ..Default::default()
    }
    .class()
}

/// Borderless flat app control (`.editor-disclosure`, `.editor-restore`).
pub fn flat_button() -> theme::Button {
    ButtonStyle {
        hover_bg: Some(Background::Color(oklch(0.245, 0.007, 152.0))),
        text: fg(),
        hover_text: Some(oklch(0.99, 0.01, 152.0)),
        radius: 8.0,
        ..Default::default()
    }
    .class()
}

/// Accent-filled primary action (`.editor-done`).
pub fn accent_filled() -> theme::Button {
    ButtonStyle {
        bg: Some(Background::Color(accent())),
        hover_bg: Some(Background::Color(oklch(0.86, 0.14, 152.0))),
        text: bg(),
        border: accent(),
        hover_border: Some(oklch(0.86, 0.14, 152.0)),
        border_width: 1.0,
        radius: 8.0,
        ..Default::default()
    }
    .class()
}
