use std::path::PathBuf;

use iced::border::Radius;
use iced::widget::{button, column, container, image as iced_image, row, stack, text, tooltip};
use iced::{Alignment, Border, Color, ContentFit, Element, Length};
use shotify_core::GradientPreset;

use crate::config::{BackgroundSelection, WallpaperTab};
use crate::message::{ColorPickerTarget, Message};
use crate::state::ShotifyApp;
use crate::theme::{parse_hex_to_iced_color, swatch_button_style, tokens, ThemeMode};
use crate::widgets::common::styled_tooltip;
use crate::widgets::icons::conic_custom_color_handle;

const TILE_SIZE: f32 = 28.0; // 7 * 28 + 6 * 4 = 220px (fits inside 224px Claude card body)

pub const SOLID_COLOR_SWATCHES: &[(&str, &str)] = &[
  ("Charcoal", "#111827"),
  ("Slate", "#334155"),
  ("Ice", "#E5E7EB"),
  ("Rose", "#FB7185"),
  ("Peach", "#FDBA74"),
  ("Honey", "#FBBF24"),
  ("Lime", "#A3E635"),
  ("Mint", "#34D399"),
  ("Aqua", "#22D3EE"),
  ("Sky", "#60A5FA"),
  ("Violet", "#A78BFA"),
  ("Fuchsia", "#F472B6"),
  ("Indigo", "#4F46E5"),
];

fn thumb_or_color_tile<'a>(
  mode: ThemeMode,
  thumb: Option<iced_image::Handle>,
  fallback_fill: Color,
  selected: bool,
  tip: String,
  on_press: Message,
) -> Element<'a, Message> {
  let t = tokens(mode);
  let border_w = if selected { 2.0 } else { 1.0 };

  let inner: Element<'a, Message> = if let Some(handle) = thumb {
    container(
      iced_image(handle)
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .into()
  } else {
    container(text(""))
      .width(Length::Fill)
      .height(Length::Fill)
      .style(move |_| container::Style {
        background: Some(fallback_fill.into()),
        ..Default::default()
      })
      .into()
  };

  let btn = button(inner)
    .on_press(on_press)
    .padding(0)
    .width(TILE_SIZE)
    .height(TILE_SIZE)
    .style(move |_, status| {
      let col = if selected {
        t.accent_purple
      } else if matches!(status, button::Status::Hovered) {
        t.border_strong
      } else {
        t.border
      };
      button::Style {
        background: Some(t.control_bg.into()),
        text_color: t.text_primary,
        border: Border {
          color: col,
          width: border_w,
          radius: Radius::from(8.0),
        },
        ..Default::default()
      }
    });

  tooltip(btn, styled_tooltip(mode, tip), tooltip::Position::Top).into()
}

fn color_swatch_tile<'a>(
  mode: ThemeMode,
  fill: Color,
  selected: bool,
  tip: String,
  on_press: Message,
) -> Element<'a, Message> {
  let btn = button(container(text("")).width(TILE_SIZE).height(TILE_SIZE))
    .on_press(on_press)
    .padding(0)
    .width(TILE_SIZE)
    .height(TILE_SIZE)
    .style(swatch_button_style(mode, fill, selected, 8.0));

  tooltip(btn, styled_tooltip(mode, tip), tooltip::Position::Top).into()
}

fn chunk_rows<'a>(items: Vec<Element<'a, Message>>, cols: usize) -> Element<'a, Message> {
  let mut col = column![].spacing(4);
  let mut iter = items.into_iter().peekable();
  while iter.peek().is_some() {
    let mut r = row![].spacing(4);
    for _ in 0..cols {
      if let Some(item) = iter.next() {
        r = r.push(item);
      } else {
        r = r.push(container(text("")).width(TILE_SIZE).height(TILE_SIZE));
      }
    }
    col = col.push(r);
  }
  col.into()
}

/// 7-column `28×28` preview grid matching `ui/src/WallpaperGrid.jsx`.
pub fn wallpaper_grid<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  match app.wallpaper_tab {
    WallpaperTab::Colors => {
      let mut tiles: Vec<Element<'a, Message>> = SOLID_COLOR_SWATCHES
        .iter()
        .map(|&(name, hex)| {
          let selected = matches!(
            &app.background_selection,
            BackgroundSelection::SolidColor { hex: active } if active.eq_ignore_ascii_case(hex)
          );
          color_swatch_tile(
            mode,
            parse_hex_to_iced_color(hex, t.control_bg),
            selected,
            name.to_string(),
            Message::SelectSolidColor(hex.to_string()),
          )
        })
        .collect();

      // 14th slot: rainbow conic `+` Custom color picker matching `WallpaperGrid.jsx` lines 185-211.
      let custom_selected = matches!(
        &app.background_selection,
        BackgroundSelection::SolidColor { hex }
          if !SOLID_COLOR_SWATCHES.iter().any(|(_, s)| s.eq_ignore_ascii_case(hex))
      );
      let conic_content = stack![
        container(
          iced_image(conic_custom_color_handle())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Cover),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true),
        container(
          container(text("+").size(13).color(Color::WHITE))
            .width(16)
            .height(16)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
              background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.42).into()),
              border: Border {
                radius: Radius::from(999.0),
                ..Default::default()
              },
              ..Default::default()
            }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill),
      ];
      let custom_btn = button(conic_content)
        .on_press(Message::OpenColorPicker(ColorPickerTarget::SolidColor))
        .padding(0)
        .width(TILE_SIZE)
        .height(TILE_SIZE)
        .style(move |_, status| button::Style {
          background: None,
          text_color: t.text_primary,
          border: Border {
            color: if custom_selected {
              t.accent_purple
            } else if matches!(status, button::Status::Hovered) {
              t.border_strong
            } else {
              t.border
            },
            width: if custom_selected { 2.0 } else { 1.0 },
            radius: Radius::from(8.0),
          },
          ..Default::default()
        });
      tiles.push(
        tooltip(
          custom_btn,
          styled_tooltip(mode, "Custom"),
          tooltip::Position::Top,
        )
        .into(),
      );

      chunk_rows(tiles, 7)
    }

    WallpaperTab::Gradients => {
      let tiles: Vec<Element<'a, Message>> = app
        .algorithm_presets
        .iter()
        .take(56)
        .map(|preset| match preset {
          GradientPreset::BlobGradient {
            id,
            name,
            config: blob_cfg,
            ..
          } => {
            let selected = matches!(
              &app.background_selection,
              BackgroundSelection::BlobGradient { config } if config.seed == blob_cfg.seed
            );
            let thumb = app.gradient_thumbs.get(id).cloned();
            let fill = parse_hex_to_iced_color(
              blob_cfg
                .colors
                .first()
                .map(String::as_str)
                .unwrap_or("#7700FF"),
              t.accent_purple,
            );
            thumb_or_color_tile(
              mode,
              thumb,
              fill,
              selected,
              name.clone(),
              Message::SelectBlobPreset(blob_cfg.clone()),
            )
          }
          GradientPreset::Gradient { id, name, config, .. } => {
            let selected = matches!(
              &app.background_selection,
              BackgroundSelection::Gradient { preset_id: Some(sel_id), .. } if sel_id == id
            );
            let thumb = app.gradient_thumbs.get(id).cloned();
            let fill = parse_hex_to_iced_color(&config.color_start, t.accent_purple);
            thumb_or_color_tile(
              mode,
              thumb,
              fill,
              selected,
              name.clone(),
              Message::SelectGradientPreset(preset.clone()),
            )
          }
        })
        .collect();
      chunk_rows(tiles, 7)
    }

    WallpaperTab::Auto => {
      let tiles: Vec<Element<'a, Message>> = app
        .auto_presets
        .iter()
        .take(14)
        .map(|preset| match preset {
          GradientPreset::BlobGradient {
            id: _,
            name,
            config: blob_cfg,
            ..
          } => {
            let selected = matches!(
              &app.background_selection,
              BackgroundSelection::BlobGradient { config } if config.seed == blob_cfg.seed
            );
            let thumb = app.auto_thumbs.get(&blob_cfg.seed).cloned();
            let fill = parse_hex_to_iced_color(
              blob_cfg
                .colors
                .first()
                .map(String::as_str)
                .unwrap_or("#7700FF"),
              t.accent_purple,
            );
            thumb_or_color_tile(
              mode,
              thumb,
              fill,
              selected,
              name.clone(),
              Message::SelectBlobPreset(blob_cfg.clone()),
            )
          }
          GradientPreset::Gradient { id, name, config, .. } => {
            let selected = matches!(
              &app.background_selection,
              BackgroundSelection::Gradient { theme, .. }
                if theme.color_start.eq_ignore_ascii_case(&config.color_start)
                  && theme.color_end.eq_ignore_ascii_case(&config.color_end)
            );
            let thumb = app.gradient_thumbs.get(id).cloned();
            let fill = parse_hex_to_iced_color(&config.color_start, t.accent_purple);
            thumb_or_color_tile(
              mode,
              thumb,
              fill,
              selected,
              name.clone(),
              Message::SelectGradientPreset(preset.clone()),
            )
          }
        })
        .collect();
      let grid = chunk_rows(tiles, 7);
      if app.raw_screenshot.is_none() {
        column![
          grid,
          text("Colors adapt automatically once a screenshot is loaded.")
            .size(10)
            .color(t.text_muted),
        ]
        .spacing(6)
        .into()
      } else {
        grid
      }
    }

    WallpaperTab::System => {
      if app.system_wallpapers.is_empty() {
        return container(
          column![
            text("No system wallpapers found")
              .size(11)
              .color(t.text_secondary),
            text("Upload a custom image above to use as wallpaper.")
              .size(10)
              .color(t.text_muted),
          ]
          .spacing(4),
        )
        .padding(10)
        .width(Length::Fill)
        .style(move |_| container::Style {
          background: Some(t.subcard_bg.into()),
          border: Border {
            color: t.border,
            width: 1.0,
            radius: Radius::from(8.0),
          },
          ..Default::default()
        })
        .into();
      }
      let tiles: Vec<Element<'a, Message>> = app
        .system_wallpapers
        .iter()
        .take(56)
        .map(|wp| {
          let wp_path = PathBuf::from(&wp.file_path);
          let selected = matches!(
            &app.background_selection,
            BackgroundSelection::Wallpaper { file_path, .. } if file_path == &wp_path
          );
          let thumb = app.system_wallpaper_thumbs.get(&wp_path).cloned();
          thumb_or_color_tile(
            mode,
            thumb,
            t.control_bg,
            selected,
            wp.name.clone(),
            Message::SelectSystemWallpaper(wp_path),
          )
        })
        .collect();
      container(chunk_rows(tiles, 7)).align_x(Alignment::Start).into()
    }
  }
}
