//! Terminal-native underwater field for the CodeWhale transcript.
//!
//! The field is atmosphere, never content: callers paint it only into cells
//! outside occupied transcript text. Reduced motion freezes the field but does
//! not remove it, so choosing an underwater treatment always has a visible
//! result.

use ratatui::style::Color;

use crate::palette::{PaletteMode, UiTheme};

/// Appearance treatment for the underwater shell.
///
/// Parsed once from persisted settings so rendering and scheduling code can
/// branch on typed state instead of scattered string comparisons. Treatment
/// is appearance only: ambient life belongs to every underwater treatment,
/// while motion is governed separately by `low_motion`/`fancy_animations`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OceanTreatment {
    /// State-reactive water column painted from the theme's [`OceanRamp`].
    #[default]
    Ombre,
    /// Plain theme surface with the same state grammar and ambient life.
    Flat,
    /// Legacy full-chrome compatibility shell. Persisted settings normalize
    /// unknown values to ombre, so this is reachable only through explicit
    /// internal selection (tests and future compatibility wiring).
    Classic,
}

impl OceanTreatment {
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let value = value.trim();
        if value.eq_ignore_ascii_case("flat") {
            Self::Flat
        } else if value.eq_ignore_ascii_case("classic") {
            Self::Classic
        } else {
            Self::Ombre
        }
    }

    #[must_use]
    pub fn is_ombre(self) -> bool {
        self == Self::Ombre
    }

    #[must_use]
    pub fn is_flat(self) -> bool {
        self == Self::Flat
    }

    #[must_use]
    pub fn is_classic(self) -> bool {
        self == Self::Classic
    }

    /// Every underwater treatment keeps idle ambient life; only the legacy
    /// classic shell stays still. Flat means a plain surface, not a lifeless
    /// ocean, and Terminal-owned backgrounds still carry foreground life.
    #[must_use]
    pub fn supports_ambient_life(self) -> bool {
        !self.is_classic()
    }
}

/// Minimum empty-water size that earns decorative ambient life. Below this,
/// content and controls own every cell. Shared by the renderer and the idle
/// animation scheduler so redraws are never scheduled for invisible life.
pub const AMBIENT_MIN_WIDTH: u16 = 68;
pub const AMBIENT_MIN_HEIGHT: u16 = 15;

/// Ambient-life ink for a theme, independent of the ombre ramp. Flat keeps
/// the same sunk seafoam as ombre; palettes without RGB surfaces (Terminal)
/// fall back to the raw secondary accent so the terminal's own palette
/// colors the life.
#[must_use]
pub fn ambient_ink(theme: &UiTheme) -> Color {
    let seafoam = rgb(theme.accent_secondary).unwrap_or((79, 209, 197));
    match rgb(theme.surface_bg) {
        Some(base) => color(mix(seafoam, base, 0.42)),
        None => theme.accent_secondary,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OceanRamp {
    pub surface: Color,
    pub middle: Color,
    pub deep: Color,
    pub ambient: Color,
}

impl OceanRamp {
    #[must_use]
    pub fn for_theme(theme: &UiTheme) -> Option<Self> {
        let base = rgb(theme.surface_bg)?;
        let seafoam = rgb(theme.accent_secondary).unwrap_or((79, 209, 197));

        let (surface, middle, deep) = match theme.mode {
            PaletteMode::Light | PaletteMode::SolarizedLight => (
                mix(base, seafoam, 0.07),
                mix(base, seafoam, 0.13),
                mix(base, (70, 139, 196), 0.18),
            ),
            PaletteMode::Dark | PaletteMode::Grayscale => (
                mix(base, (30, 71, 103), 0.24),
                mix(base, (7, 30, 54), 0.40),
                mix(base, (2, 9, 24), 0.64),
            ),
        };

        Some(Self {
            surface: color(surface),
            middle: color(middle),
            deep: color(deep),
            ambient: color(mix(seafoam, base, 0.42)),
        })
    }

    #[must_use]
    pub fn color_at(self, row: u16, height: u16) -> Color {
        if height <= 1 {
            return self.surface;
        }
        let position = f32::from(row.min(height - 1)) / f32::from(height - 1);
        if position <= 0.42 {
            mix_colors(self.surface, self.middle, position / 0.42)
        } else {
            mix_colors(self.middle, self.deep, (position - 0.42) / 0.58)
        }
    }

    #[must_use]
    pub fn color_at_phase(self, row: u16, height: u16, elapsed_ms: u128) -> Color {
        let base = self.color_at(row, height);
        let depth = if height <= 1 {
            0.0
        } else {
            f32::from(row.min(height - 1)) / f32::from(height - 1)
        };
        let cycle = (elapsed_ms % 12_000) as f32 / 12_000.0;
        let breath = (cycle * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        mix_colors(base, self.ambient, breath * 0.075 * (1.0 - depth))
    }
}

#[must_use]
fn rgb(value: Color) -> Option<(u8, u8, u8)> {
    match value {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

#[must_use]
fn color((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

#[must_use]
fn mix_colors(from: Color, to: Color, amount: f32) -> Color {
    match (rgb(from), rgb(to)) {
        (Some(from), Some(to)) => color(mix(from, to, amount)),
        _ => from,
    }
}

#[must_use]
fn mix(from: (u8, u8, u8), to: (u8, u8, u8), amount: f32) -> (u8, u8, u8) {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| {
        (f32::from(a) + (f32::from(b) - f32::from(a)) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distance(a: Color, b: Color) -> u16 {
        let (ar, ag, ab) = rgb(a).expect("RGB color");
        let (br, bg, bb) = rgb(b).expect("RGB color");
        ar.abs_diff(br) as u16 + ag.abs_diff(bg) as u16 + ab.abs_diff(bb) as u16
    }

    #[test]
    fn whale_ramp_is_perceptibly_deep_not_merely_non_equal() {
        let ramp = OceanRamp::for_theme(&crate::palette::UI_THEME).expect("RGB theme");
        assert!(
            distance(ramp.surface, ramp.deep) >= 32,
            "the selected underwater treatment must read at a glance"
        );
        assert_ne!(ramp.color_at(0, 20), ramp.color_at(19, 20));
    }

    #[test]
    fn light_theme_stays_light_enough_for_light_theme_text() {
        let ramp = OceanRamp::for_theme(&crate::palette::LIGHT_UI_THEME).expect("RGB theme");
        let (r, g, b) = rgb(ramp.deep).expect("RGB color");
        assert!(u16::from(r) + u16::from(g) + u16::from(b) > 420);
    }

    #[test]
    fn inherited_terminal_background_reports_no_ramp() {
        let mut theme = crate::palette::UI_THEME;
        theme.surface_bg = Color::Reset;
        assert_eq!(OceanRamp::for_theme(&theme), None);
    }

    #[test]
    fn every_shipped_theme_has_an_intentional_ocean_treatment() {
        use crate::palette::{SELECTABLE_THEMES, ThemeId};

        for id in SELECTABLE_THEMES {
            let ramp = OceanRamp::for_theme(&id.ui_theme());
            if matches!(id, ThemeId::Terminal) {
                assert_eq!(ramp, None, "Terminal must keep its inherited background");
            } else {
                let ramp = ramp.unwrap_or_else(|| panic!("{} has no ocean ramp", id.name()));
                assert_ne!(
                    ramp.surface,
                    ramp.deep,
                    "{} lost underwater depth",
                    id.name()
                );
            }
        }
    }

    #[test]
    fn treatment_parses_saved_values_and_defaults_to_ombre() {
        assert_eq!(OceanTreatment::parse("flat"), OceanTreatment::Flat);
        assert_eq!(OceanTreatment::parse(" FLAT "), OceanTreatment::Flat);
        assert_eq!(OceanTreatment::parse("classic"), OceanTreatment::Classic);
        assert_eq!(OceanTreatment::parse("ombre"), OceanTreatment::Ombre);
        assert_eq!(OceanTreatment::parse("kelp"), OceanTreatment::Ombre);
        assert_eq!(OceanTreatment::parse(""), OceanTreatment::Ombre);
    }

    #[test]
    fn every_underwater_treatment_keeps_ambient_life() {
        assert!(OceanTreatment::Ombre.supports_ambient_life());
        assert!(OceanTreatment::Flat.supports_ambient_life());
        assert!(!OceanTreatment::Classic.supports_ambient_life());
    }

    #[test]
    fn ambient_ink_matches_the_ramp_and_survives_reset_surfaces() {
        // RGB themes: flat fish wear the same sunk seafoam as the ombre ramp
        // so switching treatment never recolors the life.
        let theme = crate::palette::UI_THEME;
        let ramp = OceanRamp::for_theme(&theme).expect("RGB theme");
        assert_eq!(ambient_ink(&theme), ramp.ambient);

        // Terminal-owned surfaces have no RGB base; the raw secondary accent
        // lets the terminal's own palette color the life.
        let terminal = crate::palette::TERMINAL_UI_THEME;
        assert_eq!(ambient_ink(&terminal), terminal.accent_secondary);
    }

    #[test]
    fn shimmer_is_subtle_and_concentrated_near_the_surface() {
        let ramp = OceanRamp::for_theme(&crate::palette::UI_THEME).expect("RGB theme");
        let surface_a = ramp.color_at_phase(0, 20, 0);
        let surface_b = ramp.color_at_phase(0, 20, 3_000);
        let deep_a = ramp.color_at_phase(19, 20, 0);
        let deep_b = ramp.color_at_phase(19, 20, 3_000);

        let surface_shift = distance(surface_a, surface_b);
        assert!(
            (2..=18).contains(&surface_shift),
            "surface shift was {surface_shift}"
        );
        assert_eq!(
            deep_a, deep_b,
            "the floor should stay perceptually anchored"
        );
    }
}
