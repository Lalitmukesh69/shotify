use iced::border::Radius;
use iced::overlay::menu;
use iced::widget::{
  button, checkbox, container, pick_list, scrollable, slider, svg, text_input,
};
use iced::{Border, Color, Shadow, Theme, Vector};

/// UI theme appearance for the native Shotify application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
  #[default]
  Dark,
  Light,
}

impl ThemeMode {
  pub fn to_iced_theme(self) -> Theme {
    match self {
      Self::Dark => Theme::Dark,
      Self::Light => Theme::Light,
    }
  }
}

/// Design tokens aligned with Shotify's normal UI (`ui/src/App.css`, `Sidebar.jsx`, `SettingsModal.jsx`).
#[derive(Debug, Clone, Copy)]
pub struct PaletteTokens {
  pub canvas_bg: Color,
  pub panel_bg: Color,
  pub subcard_bg: Color,
  pub control_bg: Color,
  pub control_hover: Color,
  pub tab_rail_bg: Color,
  pub tab_hover: Color,
  pub chip_bg: Color,
  pub tooltip_bg: Color,
  pub border: Color,
  pub border_strong: Color,
  pub panel_hairline: Color,
  pub text_primary: Color,
  pub text_secondary: Color,
  pub text_muted: Color,
  pub text_disabled: Color,
  pub accent_purple: Color,
  pub accent_purple_hover: Color,
  pub accent_purple_pressed: Color,
  pub accent_purple_tint: Color,
  pub accent_error: Color,
}

pub fn tokens(mode: ThemeMode) -> PaletteTokens {
  match mode {
    ThemeMode::Dark => PaletteTokens {
      canvas_bg: Color::from_rgb8(0x0E, 0x0E, 0x12),
      panel_bg: Color::from_rgb8(0x11, 0x11, 0x15),
      subcard_bg: Color::from_rgb8(0x1E, 0x1E, 0x24),
      control_bg: Color::from_rgb8(0x2C, 0x2C, 0x31),
      control_hover: Color::from_rgb8(0x36, 0x36, 0x3D),
      tab_rail_bg: Color::from_rgb8(0x16, 0x16, 0x1A),
      tab_hover: Color::from_rgb8(0x23, 0x23, 0x2A),
      chip_bg: Color::from_rgb8(0x0F, 0x11, 0x16),
      tooltip_bg: Color::from_rgb8(0x18, 0x18, 0x1E),
      border: Color::from_rgba(1.0, 1.0, 1.0, 0.10),
      border_strong: Color::from_rgba(1.0, 1.0, 1.0, 0.20),
      panel_hairline: Color::from_rgb8(0x1B, 0x1F, 0x23),
      text_primary: Color::WHITE,
      text_secondary: Color::from_rgba(1.0, 1.0, 1.0, 0.85),
      text_muted: Color::from_rgb8(0x94, 0x94, 0x9E),
      text_disabled: Color::from_rgba(1.0, 1.0, 1.0, 0.36),
      accent_purple: Color::from_rgb8(0x77, 0x00, 0xFF),
      accent_purple_hover: Color::from_rgb8(0x8A, 0x1A, 0xFF),
      accent_purple_pressed: Color::from_rgb8(0x64, 0x00, 0xDC),
      accent_purple_tint: Color::from_rgba8(0x77, 0x00, 0xFF, 0.24),
      accent_error: Color::from_rgb8(0xEF, 0x44, 0x44),
    },
    ThemeMode::Light => PaletteTokens {
      canvas_bg: Color::from_rgb8(0xEC, 0xED, 0xF1),
      panel_bg: Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.97),
      subcard_bg: Color::from_rgb8(0xF4, 0xF5, 0xF8),
      control_bg: Color::from_rgb8(0xE6, 0xE7, 0xEC),
      control_hover: Color::from_rgb8(0xDA, 0xDC, 0xE3),
      tab_rail_bg: Color::from_rgb8(0xDC, 0xDE, 0xE5),
      tab_hover: Color::from_rgb8(0xD0, 0xD2, 0xDB),
      chip_bg: Color::from_rgb8(0xE8, 0xEA, 0xF0),
      tooltip_bg: Color::from_rgba8(0xFF, 0xFF, 0xFF, 0.98),
      border: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
      border_strong: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
      panel_hairline: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
      text_primary: Color::from_rgb8(0x14, 0x15, 0x1A),
      text_secondary: Color::from_rgba(0.08, 0.09, 0.12, 0.78),
      text_muted: Color::from_rgb8(0x52, 0x58, 0x66),
      text_disabled: Color::from_rgba(0.08, 0.09, 0.12, 0.36),
      accent_purple: Color::from_rgb8(0x77, 0x00, 0xFF),
      accent_purple_hover: Color::from_rgb8(0x8A, 0x1A, 0xFF),
      accent_purple_pressed: Color::from_rgb8(0x64, 0x00, 0xDC),
      accent_purple_tint: Color::from_rgba8(0x77, 0x00, 0xFF, 0.14),
      accent_error: Color::from_rgb8(0xDC, 0x26, 0x26),
    },
  }
}

pub fn canvas_backdrop_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(t.canvas_bg.into()),
      text_color: Some(t.text_primary),
      ..Default::default()
    }
  }
}

/// Slim borderless top titlebar background matching `ui/src/WindowControls.jsx`.
pub fn window_titlebar_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    let bg = match mode {
      ThemeMode::Dark => Color::from_rgb8(0x05, 0x05, 0x07),
      ThemeMode::Light => t.tab_rail_bg,
    };
    container::Style {
      background: Some(bg.into()),
      text_color: Some(t.text_muted),
      ..Default::default()
    }
  }
}

/// Window caption button style (Minimize, Maximize/Restore, Close with crimson hover).
pub fn window_control_button_style(
  mode: ThemeMode,
  is_close: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let (bg, text_color) = match status {
      button::Status::Pressed => (
        Some(
          if is_close {
            Color::from_rgb8(0xC5, 0x0E, 0x1F)
          } else {
            t.control_hover
          }
          .into(),
        ),
        if is_close { Color::WHITE } else { t.text_primary },
      ),
      button::Status::Hovered => (
        Some(
          if is_close {
            Color::from_rgba8(0xE8, 0x11, 0x23, 0.88)
          } else {
            Color { a: 0.10, ..t.text_primary }
          }
          .into(),
        ),
        if is_close { Color::WHITE } else { t.text_primary },
      ),
      button::Status::Active | button::Status::Disabled => (None, t.text_muted),
    };
    button::Style {
      background: bg,
      text_color,
      border: Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: Radius::from(4.0),
      },
      ..Default::default()
    }
  }
}

pub fn chrome_sidebar_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(t.panel_bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.panel_hairline,
        width: 1.0,
        radius: Radius::from(18.0),
      },
      shadow: Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
        offset: Vector::new(0.0, 4.0),
        blur_radius: 12.0,
      },
    }
  }
}

/// Modular Claude-style surface card inside the inspector sidebar.
pub fn sidebar_section_card_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(t.subcard_bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.border,
        width: 1.0,
        radius: Radius::from(12.0),
      },
      ..Default::default()
    }
  }
}

/// Quiet hairline-framed pill readout for card metadata and slider values.
pub fn readout_chip_style(
  mode: ThemeMode,
  accent: bool,
) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(if accent { t.accent_purple_tint } else { t.chip_bg }.into()),
      text_color: Some(if accent { t.text_primary } else { t.text_secondary }),
      border: Border {
        color: if accent {
          Color {
            a: 0.45,
            ..t.accent_purple_hover
          }
        } else {
          t.border
        },
        width: 1.0,
        radius: Radius::from(999.0),
      },
      ..Default::default()
    }
  }
}

pub fn floating_panel_style(mode: ThemeMode, radius: f32) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(t.panel_bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.panel_hairline,
        width: 1.0,
        radius: Radius::from(radius),
      },
      shadow: Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
        offset: Vector::new(0.0, 4.0),
        blur_radius: 14.0,
      },
    }
  }
}

pub fn empty_card_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let bg = match mode {
      ThemeMode::Dark => Color::from_rgb8(0x24, 0x24, 0x29),
      ThemeMode::Light => Color::WHITE,
    };
    let t = tokens(mode);
    container::Style {
      background: Some(bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.border,
        width: 1.0,
        radius: Radius::from(24.0),
      },
      shadow: Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.32),
        offset: Vector::new(0.0, 24.0),
        blur_radius: 48.0,
      },
    }
  }
}

pub fn modal_panel_style(mode: ThemeMode, radius: f32) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    let bg = match mode {
      ThemeMode::Dark => Color::from_rgb8(0x17, 0x17, 0x1B),
      ThemeMode::Light => t.panel_bg,
    };
    container::Style {
      background: Some(bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.border,
        width: 1.0,
        radius: Radius::from(radius),
      },
      shadow: Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.62),
        offset: Vector::new(0.0, 24.0),
        blur_radius: 60.0,
      },
    }
  }
}

pub fn settings_section_card_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(t.subcard_bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.border,
        width: 1.0,
        radius: Radius::from(16.0),
      },
      ..Default::default()
    }
  }
}

pub fn tab_rail_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(t.tab_rail_bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.border,
        width: 1.0,
        radius: Radius::from(10.0),
      },
      ..Default::default()
    }
  }
}

pub fn edition_badge_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    let text_col = match mode {
      ThemeMode::Dark => Color::from_rgb8(0xD8, 0xB4, 0xFE),
      ThemeMode::Light => t.accent_purple,
    };
    container::Style {
      background: Some(t.accent_purple_tint.into()),
      text_color: Some(text_col),
      border: Border {
        color: Color::from_rgba8(0x8A, 0x1A, 0xFF, 0.45),
        width: 1.0,
        radius: Radius::from(999.0),
      },
      ..Default::default()
    }
  }
}

pub fn toast_style(
  mode: ThemeMode,
  is_error: bool,
  is_success: bool,
) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    let accent = if is_error {
      Color { a: 0.45, ..t.accent_error }
    } else if is_success {
      Color { a: 0.45, ..t.accent_purple }
    } else {
      t.border_strong
    };
    container::Style {
      background: Some(t.tooltip_bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: accent,
        width: 1.0,
        radius: Radius::from(10.0),
      },
      shadow: Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
        offset: Vector::new(0.0, 10.0),
        blur_radius: 28.0,
      },
    }
  }
}

pub fn tooltip_container_style(mode: ThemeMode) -> impl Fn(&Theme) -> container::Style {
  move |_| {
    let t = tokens(mode);
    container::Style {
      background: Some(t.tooltip_bg.into()),
      text_color: Some(t.text_primary),
      border: Border {
        color: t.border_strong,
        width: 1.0,
        radius: Radius::from(6.0),
      },
      shadow: Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
        offset: Vector::new(0.0, 4.0),
        blur_radius: 12.0,
      },
    }
  }
}

pub fn pill_button_style(
  mode: ThemeMode,
  active: bool,
  radius: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let (bg, text_color, border_col) = match status {
      button::Status::Disabled => (
        Color { a: 0.45, ..t.control_bg },
        t.text_disabled,
        Color::TRANSPARENT,
      ),
      button::Status::Pressed => (
        if active { t.accent_purple_pressed } else { t.control_hover },
        if active { Color::WHITE } else { t.text_primary },
        if active { t.accent_purple_hover } else { t.border },
      ),
      button::Status::Hovered => (
        if active { t.accent_purple_hover } else { t.control_hover },
        if active { Color::WHITE } else { t.text_primary },
        if active { t.accent_purple_hover } else { t.border_strong },
      ),
      button::Status::Active => (
        if active { t.accent_purple } else { t.control_bg },
        if active { Color::WHITE } else { t.text_primary },
        if active { t.accent_purple } else { Color::TRANSPARENT },
      ),
    };
    button::Style {
      background: Some(bg.into()),
      text_color,
      border: Border {
        color: border_col,
        width: if border_col == Color::TRANSPARENT { 0.0 } else { 1.0 },
        radius: Radius::from(radius),
      },
      ..Default::default()
    }
  }
}

pub fn tab_pill_button_style(
  mode: ThemeMode,
  active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let bg = if active {
      Some(t.accent_purple.into())
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
      Some(t.tab_hover.into())
    } else {
      None
    };
    let text_color = if active { Color::WHITE } else { t.text_secondary };
    button::Style {
      background: bg,
      text_color,
      border: Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: Radius::from(7.0),
      },
      ..Default::default()
    }
  }
}

pub fn purple_primary_button_style(
  mode: ThemeMode,
  radius: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let (bg, text_color) = match status {
      button::Status::Disabled => (Color { a: 0.35, ..t.accent_purple }, t.text_disabled),
      button::Status::Hovered => (t.accent_purple_hover, Color::WHITE),
      button::Status::Pressed => (t.accent_purple_pressed, Color::WHITE),
      button::Status::Active => (t.accent_purple, Color::WHITE),
    };
    button::Style {
      background: Some(bg.into()),
      text_color,
      border: Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: Radius::from(radius),
      },
      ..Default::default()
    }
  }
}

pub fn annotation_icon_button_style(
  mode: ThemeMode,
  active: bool,
  radius: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let (bg, text_color) = match status {
      button::Status::Disabled => (None, t.text_disabled),
      button::Status::Pressed => (
        Some(if active { t.accent_purple_pressed } else { t.tab_hover }.into()),
        if active { Color::WHITE } else { t.text_primary },
      ),
      button::Status::Hovered => (
        Some(if active { t.accent_purple_hover } else { t.tab_hover }.into()),
        if active { Color::WHITE } else { t.text_primary },
      ),
      button::Status::Active => (
        if active { Some(t.accent_purple.into()) } else { None },
        if active { Color::WHITE } else { t.text_primary },
      ),
    };
    button::Style {
      background: bg,
      text_color,
      border: Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: Radius::from(radius),
      },
      ..Default::default()
    }
  }
}

pub fn thumb_tile_button_style(
  mode: ThemeMode,
  active: bool,
  radius: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let bg = if active {
      Some(t.accent_purple_tint.into())
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
      Some(t.tab_hover.into())
    } else {
      None
    };
    button::Style {
      background: bg,
      text_color: t.text_primary,
      border: Border {
        color: if active {
          t.accent_purple
        } else if matches!(status, button::Status::Hovered) {
          t.border_strong
        } else {
          t.border
        },
        width: if active { 2.0 } else { 1.0 },
        radius: Radius::from(radius),
      },
      ..Default::default()
    }
  }
}

pub fn footer_action_button_style(
  mode: ThemeMode,
  is_primary: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let (bg, text_color) = match status {
      button::Status::Disabled => (
        if is_primary {
          Color { a: 0.35, ..t.accent_purple }
        } else {
          Color { a: 0.45, ..t.control_bg }
        },
        t.text_disabled,
      ),
      button::Status::Pressed => (
        if is_primary { t.accent_purple_pressed } else { t.control_hover },
        if is_primary { Color::WHITE } else { t.text_primary },
      ),
      button::Status::Hovered => (
        if is_primary { t.accent_purple_hover } else { t.control_hover },
        if is_primary { Color::WHITE } else { t.text_primary },
      ),
      button::Status::Active => (
        if is_primary { t.accent_purple } else { t.control_bg },
        if is_primary { Color::WHITE } else { t.text_primary },
      ),
    };
    button::Style {
      background: Some(bg.into()),
      text_color,
      border: Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: Radius::from(6.0),
      },
      ..Default::default()
    }
  }
}

pub fn ghost_button_style(mode: ThemeMode) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let (bg, text_color) = match status {
      button::Status::Disabled => (None, t.text_disabled),
      button::Status::Hovered | button::Status::Pressed => {
        (Some(Color { a: 0.07, ..t.text_primary }.into()), t.text_primary)
      }
      button::Status::Active => (None, t.text_secondary),
    };
    button::Style {
      background: bg,
      text_color,
      border: Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: Radius::from(7.0),
      },
      ..Default::default()
    }
  }
}

pub fn danger_pill_button_style(mode: ThemeMode) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let bg = match status {
      button::Status::Hovered | button::Status::Pressed => Color { a: 0.30, ..t.accent_error },
      _ => Color { a: 0.18, ..t.accent_error },
    };
    button::Style {
      background: Some(bg.into()),
      text_color: t.accent_error,
      border: Border {
        color: Color { a: 0.50, ..t.accent_error },
        width: 1.0,
        radius: Radius::from(6.0),
      },
      ..Default::default()
    }
  }
}

pub fn swatch_button_style(
  mode: ThemeMode,
  fill: Color,
  selected: bool,
  radius: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style {
  move |_, status| {
    let t = tokens(mode);
    let border_col = match status {
      button::Status::Disabled => Color { a: 0.35, ..t.border },
      _ if selected => t.accent_purple,
      button::Status::Hovered => t.border_strong,
      _ => t.border,
    };
    let bg = match status {
      button::Status::Disabled => Color { a: 0.40, ..fill },
      _ => fill,
    };
    button::Style {
      background: Some(bg.into()),
      text_color: t.text_primary,
      border: Border {
        color: border_col,
        width: if selected { 2.0 } else { 1.0 },
        radius: Radius::from(radius),
      },
      ..Default::default()
    }
  }
}

pub fn validated_input_style(
  mode: ThemeMode,
  valid: bool,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
  move |_, status| {
    let t = tokens(mode);
    let border_col = if !valid {
      t.accent_error
    } else {
      match status {
        text_input::Status::Focused => t.accent_purple,
        text_input::Status::Hovered => t.border_strong,
        _ => t.border,
      }
    };
    text_input::Style {
      background: t.control_bg.into(),
      border: Border {
        color: border_col,
        width: 1.0,
        radius: Radius::from(6.0),
      },
      icon: t.text_muted,
      placeholder: t.text_muted,
      value: t.text_primary,
      selection: Color::from_rgba8(0x77, 0x00, 0xFF, 0.35),
    }
  }
}

pub fn input_style(mode: ThemeMode) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
  validated_input_style(mode, true)
}

pub fn dark_slider_style(mode: ThemeMode) -> impl Fn(&Theme, slider::Status) -> slider::Style {
  move |_, status| {
    let t = tokens(mode);
    let handle_bg = match status {
      slider::Status::Hovered | slider::Status::Dragged => Color::from_rgb8(0xF5, 0xF5, 0xF7),
      _ => Color::WHITE,
    };
    slider::Style {
      rail: slider::Rail {
        backgrounds: (t.accent_purple.into(), t.control_bg.into()),
        width: 4.0,
        border: Border {
          color: Color::TRANSPARENT,
          width: 0.0,
          radius: Radius::from(2.0),
        },
      },
      handle: slider::Handle {
        shape: slider::HandleShape::Rectangle {
          width: 18,
          border_radius: Radius::from(7.0),
        },
        background: handle_bg.into(),
        border_width: 1.0,
        border_color: t.border_strong,
      },
    }
  }
}

pub fn checkbox_style(mode: ThemeMode) -> impl Fn(&Theme, checkbox::Status) -> checkbox::Style {
  move |_, status| {
    let t = tokens(mode);
    let (is_checked, hovered, disabled) = match status {
      checkbox::Status::Active { is_checked } => (is_checked, false, false),
      checkbox::Status::Hovered { is_checked } => (is_checked, true, false),
      checkbox::Status::Disabled { is_checked } => (is_checked, false, true),
    };
    let bg = if disabled {
      Color { a: 0.35, ..t.control_bg }
    } else if is_checked {
      if hovered { t.accent_purple_hover } else { t.accent_purple }
    } else if hovered {
      t.control_hover
    } else {
      t.control_bg
    };
    checkbox::Style {
      background: bg.into(),
      icon_color: if disabled { t.text_disabled } else { Color::WHITE },
      border: Border {
        color: if is_checked {
          t.accent_purple
        } else if hovered {
          t.border_strong
        } else {
          t.border
        },
        width: 1.0,
        radius: Radius::from(4.0),
      },
      text_color: Some(if disabled { t.text_disabled } else { t.text_primary }),
    }
  }
}

pub fn pick_list_style(mode: ThemeMode) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
  move |_, status| {
    let t = tokens(mode);
    let border_col = match status {
      pick_list::Status::Opened => t.accent_purple,
      pick_list::Status::Hovered => t.border_strong,
      pick_list::Status::Active => t.border,
    };
    let bg = match status {
      pick_list::Status::Hovered | pick_list::Status::Opened => t.control_hover,
      pick_list::Status::Active => t.control_bg,
    };
    pick_list::Style {
      text_color: t.text_primary,
      placeholder_color: t.text_muted,
      handle_color: t.text_secondary,
      background: bg.into(),
      border: Border {
        color: border_col,
        width: 1.0,
        radius: Radius::from(6.0),
      },
    }
  }
}

pub fn pick_list_menu_style(mode: ThemeMode) -> impl Fn(&Theme) -> menu::Style {
  move |_| {
    let t = tokens(mode);
    menu::Style {
      background: t.subcard_bg.into(),
      border: Border {
        color: t.border_strong,
        width: 1.0,
        radius: Radius::from(8.0),
      },
      text_color: t.text_secondary,
      selected_text_color: Color::WHITE,
      selected_background: t.accent_purple.into(),
    }
  }
}

pub fn svg_tint_style(color: Color) -> impl Fn(&Theme, svg::Status) -> svg::Style {
  move |_, _| svg::Style { color: Some(color) }
}

/// Razor-thin 2.5px vertical scrollbar tucked 3px inside the panel edge.
pub fn minimal_scrollbar_direction() -> scrollable::Direction {
  scrollable::Direction::Vertical(
    scrollable::Scrollbar::new()
      .width(4.0)
      .scroller_width(2.5)
      .margin(3.0),
  )
}

pub fn subtle_scrollable_style(
  mode: ThemeMode,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
  move |_, status| {
    let t = tokens(mode);
    let scroller_col = match status {
      scrollable::Status::Dragged {
        is_vertical_scrollbar_dragged: true,
        ..
      }
      | scrollable::Status::Dragged {
        is_horizontal_scrollbar_dragged: true,
        ..
      } => Color {
        a: 0.55,
        ..t.accent_purple_hover
      },
      scrollable::Status::Dragged { .. } => Color {
        a: 0.08,
        ..t.text_primary
      },
      scrollable::Status::Hovered {
        is_vertical_scrollbar_hovered: true,
        ..
      }
      | scrollable::Status::Hovered {
        is_horizontal_scrollbar_hovered: true,
        ..
      } => Color {
        a: 0.18,
        ..t.text_primary
      },
      scrollable::Status::Hovered { .. } => Color {
        a: 0.04,
        ..t.text_primary
      },
      scrollable::Status::Active => Color::TRANSPARENT,
    };
    let rail = scrollable::Rail {
      background: None,
      border: Border::default(),
      scroller: scrollable::Scroller {
        color: scroller_col,
        border: Border {
          radius: Radius::from(999.0),
          ..Default::default()
        },
      },
    };
    scrollable::Style {
      container: container::Style::default(),
      vertical_rail: rail,
      horizontal_rail: rail,
      gap: None,
    }
  }
}

pub fn parse_hex_to_iced_color(raw: &str, fallback: Color) -> Color {
  shotify_core::parse_css_color(raw)
    .map(|c| Color::from_rgba8(c.r, c.g, c.b, c.a))
    .unwrap_or(fallback)
}
