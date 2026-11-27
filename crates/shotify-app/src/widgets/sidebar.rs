use iced::border::Radius;
use iced::widget::{
  button, checkbox, column, container, horizontal_space, image as iced_image, pick_list, row,
  scrollable, svg, text, text_input, tooltip,
};
use iced::{Alignment, Border, Color, ContentFit, Element, Length};
use shotify_core::{AspectMode, AspectRatioPreset, RATIO_OPTIONS};

use crate::message::{ColorPickerTarget, Message, ShadowFieldUpdate};
use crate::state::ShotifyApp;
use crate::theme::{
  checkbox_style, chrome_sidebar_style, ghost_button_style, input_style,
  minimal_scrollbar_direction, parse_hex_to_iced_color, pick_list_menu_style, pick_list_style,
  pill_button_style, readout_chip_style, sidebar_section_card_style, subtle_scrollable_style,
  svg_tint_style, swatch_button_style, thumb_tile_button_style, tokens, validated_input_style,
  ThemeMode,
};
use crate::widgets::backgrounds_panel::backgrounds_panel;
use crate::widgets::common::{card_section_header, divider_h, styled_tooltip};
use crate::widgets::icons::{
  chevron_down_svg, chevron_right_svg, mockup_mac_dark_handle, mockup_mac_light_handle,
  mockup_safari_dark_handle, mockup_win_dark_handle, mockup_win_light_handle,
  rotate_ccw_muted_svg, shadow_ambient_handle, shadow_custom_handle, shadow_heavy_handle,
  shadow_hug_handle,
};
use crate::widgets::slider::{inline_f64_slider, inline_f64_slider_with_readout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WatermarkPosOption {
  value: &'static str,
  label: &'static str,
}

impl std::fmt::Display for WatermarkPosOption {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.label)
  }
}

const WM_POSITIONS: &[WatermarkPosOption] = &[
  WatermarkPosOption {
    value: "inside",
    label: "Inside screenshot",
  },
  WatermarkPosOption {
    value: "center-bottom",
    label: "Below screenshot",
  },
  WatermarkPosOption {
    value: "top-left",
    label: "Top left",
  },
  WatermarkPosOption {
    value: "top-right",
    label: "Top right",
  },
  WatermarkPosOption {
    value: "center-top",
    label: "Top center",
  },
  WatermarkPosOption {
    value: "bottom-left",
    label: "Bottom left",
  },
  WatermarkPosOption {
    value: "bottom-right",
    label: "Bottom right",
  },
];

fn ratio_preview_rect<'a>(
  mode: ThemeMode,
  w: u32,
  h: u32,
  is_auto: bool,
  is_custom: bool,
  selected: bool,
) -> Element<'a, Message> {
  let t = tokens(mode);
  let stroke_col = if selected { Color::WHITE } else { t.text_muted };
  let fill_col = if selected {
    Color::from_rgba(1.0, 1.0, 1.0, 0.45)
  } else {
    Color {
      a: 0.25,
      ..t.text_primary
    }
  };

  if is_auto || is_custom {
    return container(text(""))
      .width(14)
      .height(14)
      .style(move |_| container::Style {
        background: None,
        border: Border {
          color: stroke_col,
          width: 1.5,
          radius: Radius::from(2.0),
        },
        ..Default::default()
      })
      .into();
  }

  let max_w = 22.0_f32;
  let max_h = 14.0_f32;
  let aspect = (w as f32) / (h.max(1) as f32);
  let cont_aspect = max_w / max_h;
  let (bw, bh) = if aspect >= cont_aspect {
    (max_w, (max_w / aspect).max(4.0))
  } else {
    ((max_h * aspect).max(4.0), max_h)
  };

  container(text(""))
    .width(bw)
    .height(bh)
    .style(move |_| container::Style {
      background: Some(fill_col.into()),
      border: Border {
        radius: Radius::from(2.0),
        ..Default::default()
      },
      ..Default::default()
    })
    .into()
}

fn opt_to_aspect_mode(opt: &AspectRatioPreset) -> AspectMode {
  if opt.value == "auto" {
    AspectMode::Auto
  } else if opt.value == "custom" {
    AspectMode::Custom
  } else {
    AspectMode::Preset
  }
}

fn sidebar_card<'a>(mode: ThemeMode, content: Element<'a, Message>) -> Element<'a, Message> {
  container(content)
    .padding([12, 11])
    .width(Length::Fill)
    .style(sidebar_section_card_style(mode))
    .into()
}

/// Floating left 270px inspector sidebar organized in Claude modular design language:
/// quiet hairline-framed cards (`subcard_bg`), editorial sentence-case titles with status chips,
/// concentric radii (`18 -> 12 -> 8`), pill-readout sliders, and single-level disclosures.
pub fn sidebar<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  // 1) Frame & layout card (Ratio + inline selector + Padding + Inset + AutoCrop)
  let sel_ratio = app.current_aspect_option();
  let ratio_trigger = button(
    row![
      ratio_preview_rect(
        mode,
        sel_ratio.w,
        sel_ratio.h,
        sel_ratio.value == "auto",
        sel_ratio.value == "custom",
        app.ratio_menu_open,
      ),
      text(sel_ratio.label).size(12).color(if app.ratio_menu_open {
        Color::WHITE
      } else {
        t.text_primary
      }),
      svg(chevron_down_svg())
        .width(13)
        .height(13)
        .style(svg_tint_style(if app.ratio_menu_open {
          Color::WHITE
        } else {
          t.text_secondary
        })),
    ]
    .spacing(7)
    .align_y(Alignment::Center),
  )
  .on_press(Message::ToggleRatioMenu)
  .padding([5, 9])
  .style(pill_button_style(mode, app.ratio_menu_open, 8.0));

  let mut aspect_col = column![row![
    text("Ratio")
      .size(12)
      .color(t.text_secondary)
      .width(Length::Fill),
    ratio_trigger,
  ]
  .align_y(Alignment::Center)
  .width(Length::Fill)]
  .spacing(8);

  if app.ratio_menu_open {
    let mut ratio_list = column![].spacing(2);
    for opt in RATIO_OPTIONS {
      let selected = opt.value == sel_ratio.value;
      let is_auto = opt.value == "auto";
      let is_custom = opt.value == "custom";
      let opt_mode = opt_to_aspect_mode(opt);
      let val_owned = opt.value.to_string();

      ratio_list = ratio_list.push(
        button(
          row![
            ratio_preview_rect(mode, opt.w, opt.h, is_auto, is_custom, selected),
            text(opt.label).size(11).color(if selected {
              Color::WHITE
            } else {
              t.text_secondary
            }),
          ]
          .spacing(8)
          .align_y(Alignment::Center),
        )
        .on_press(Message::AspectModeSelected(opt_mode, val_owned))
        .padding([6, 10])
        .width(Length::Fill)
        .style(pill_button_style(mode, selected, 6.0)),
      );
    }

    aspect_col = aspect_col.push(
      container(
        scrollable(ratio_list)
          .height(Length::Shrink)
          .direction(minimal_scrollbar_direction())
          .style(subtle_scrollable_style(mode)),
      )
      .max_height(210)
      .padding(5)
      .width(Length::Fill)
      .style(move |_| container::Style {
        background: Some(t.panel_bg.into()),
        border: Border {
          color: t.border,
          width: 1.0,
          radius: Radius::from(8.0),
        },
        ..Default::default()
      }),
    );
  }

  if app.options.aspect_mode == AspectMode::Custom {
    let w_valid = app
      .custom_ratio_w_input
      .trim()
      .parse::<u32>()
      .ok()
      .filter(|v| (1..=4000).contains(v))
      .is_some();
    let h_valid = app
      .custom_ratio_h_input
      .trim()
      .parse::<u32>()
      .ok()
      .filter(|v| (1..=4000).contains(v))
      .is_some();

    aspect_col = aspect_col.push(
      row![
        text("Custom")
          .size(11)
          .color(t.text_muted)
          .width(Length::Fill),
        container(
          text_input("16", &app.custom_ratio_w_input)
            .on_input(Message::CustomRatioWChanged)
            .padding([4, 8])
            .size(12)
            .style(validated_input_style(mode, w_valid)),
        )
        .width(58),
        text(":").size(12).color(t.text_secondary),
        container(
          text_input("9", &app.custom_ratio_h_input)
            .on_input(Message::CustomRatioHChanged)
            .padding([4, 8])
            .size(12)
            .style(validated_input_style(mode, h_valid)),
        )
        .width(58),
      ]
      .spacing(6)
      .align_y(Alignment::Center),
    );
  }

  let padding_slider = inline_f64_slider(
    mode,
    "Padding",
    app.options.padding_size,
    0.0..=200.0,
    2.0,
    format!("{}px", app.options.padding_size.round() as i32),
    98.0,
    Message::PaddingChanged,
  );

  let inset_slider = inline_f64_slider(
    mode,
    "Inset",
    app.options.inset.padding,
    0.0..=100.0,
    1.0,
    format!("{}px", app.options.inset.padding.round() as i32),
    98.0,
    Message::InsetChanged,
  );

  let autocrop_row = button(
    row![
      column![
        text("AutoCrop").size(12).color(t.text_primary),
        text("Balance screenshot margins")
          .size(10)
          .color(t.text_muted),
      ]
      .spacing(1)
      .width(Length::Fill),
      checkbox("", app.options.balance.enabled)
        .on_toggle(Message::AutoCropToggled)
        .size(15)
        .style(checkbox_style(mode)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .width(Length::Fill),
  )
  .on_press(Message::AutoCropToggled(!app.options.balance.enabled))
  .padding([5, 6])
  .width(Length::Fill)
  .style(ghost_button_style(mode));

  let frame_card = sidebar_card(
    mode,
    column![
      card_section_header(mode, "Frame & layout", Some((sel_ratio.label, false))),
      aspect_col,
      padding_slider,
      inset_slider,
      divider_h(mode),
      autocrop_row,
    ]
    .spacing(9)
    .into(),
  );

  // 2) Appearance card (Shadow style + optional inline custom shadow + Curve + Border + Border color)
  let shadows = [
    ("Hug", "Hug", shadow_hug_handle()),
    ("Heavy", "Heavy", shadow_heavy_handle()),
    ("Ambient", "Ambient", shadow_ambient_handle()),
    ("Custom", "Custom", shadow_custom_handle()),
  ];

  let mut shadow_grid = row![].spacing(6).width(Length::Fill);
  for (id, label, thumb) in shadows {
    let active = app.options.shadow.eq_ignore_ascii_case(id);
    let next = if active { "None" } else { id };
    let thumb_btn = button(
      container(
        iced_image(thumb)
          .width(Length::Fill)
          .height(Length::Fill)
          .content_fit(ContentFit::Cover),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .clip(true),
    )
    .on_press(Message::SelectShadowStyle(next.to_string()))
    .padding(3)
    .width(Length::Fill)
    .height(48)
    .style(thumb_tile_button_style(mode, active, 9.0));

    shadow_grid = shadow_grid.push(
      column![
        thumb_btn,
        text(label)
          .size(10)
          .color(if active { t.text_primary } else { t.text_muted }),
      ]
      .spacing(4)
      .align_x(Alignment::Center)
      .width(Length::FillPortion(1)),
    );
  }

  let shadow_badge_label = app.options.shadow.as_str();
  let shadow_is_active = !shadow_badge_label.eq_ignore_ascii_case("None");
  let mut appearance_col = column![
    card_section_header(
      mode,
      "Appearance",
      Some((shadow_badge_label, shadow_is_active)),
    ),
    text("Shadow style").size(11).color(t.text_muted),
    shadow_grid,
  ]
  .spacing(9);

  if app.options.shadow.eq_ignore_ascii_case("Custom") {
    let adv = &app.options.advanced_shadow;
    let shadow_col = parse_hex_to_iced_color(&adv.color, Color::BLACK);
    appearance_col = appearance_col.push(divider_h(mode)).push(
      column![
        row![
          text("Custom shadow")
            .size(11)
            .color(t.text_secondary)
            .width(Length::Fill),
          tooltip(
            button(container(text("")).width(20).height(20))
              .on_press(Message::OpenColorPicker(ColorPickerTarget::CustomShadowColor))
              .padding(0)
              .width(20)
              .height(20)
              .style(swatch_button_style(mode, shadow_col, false, 999.0)),
            styled_tooltip(mode, "Pick shadow color"),
            tooltip::Position::Top,
          ),
        ]
        .align_y(Alignment::Center),
        inline_f64_slider_with_readout(
          mode,
          "X",
          adv.offset_x,
          -80.0..=80.0,
          1.0,
          format!("{}px", adv.offset_x.round() as i32),
          94.0,
          |v| Message::UpdateCustomShadow(ShadowFieldUpdate::OffsetX(v)),
        ),
        inline_f64_slider_with_readout(
          mode,
          "Y",
          adv.offset_y,
          -80.0..=120.0,
          1.0,
          format!("{}px", adv.offset_y.round() as i32),
          94.0,
          |v| Message::UpdateCustomShadow(ShadowFieldUpdate::OffsetY(v)),
        ),
        inline_f64_slider_with_readout(
          mode,
          "Blur",
          adv.blur,
          0.0..=160.0,
          2.0,
          format!("{}px", adv.blur.round() as i32),
          94.0,
          |v| Message::UpdateCustomShadow(ShadowFieldUpdate::Blur(v)),
        ),
        inline_f64_slider_with_readout(
          mode,
          "Spread",
          adv.spread,
          -80.0..=80.0,
          1.0,
          format!("{}px", adv.spread.round() as i32),
          94.0,
          |v| Message::UpdateCustomShadow(ShadowFieldUpdate::Spread(v)),
        ),
        inline_f64_slider_with_readout(
          mode,
          "Opacity",
          adv.opacity,
          0.0..=1.0,
          0.05,
          format!("{}%", (adv.opacity * 100.0).round() as i32),
          94.0,
          |v| Message::UpdateCustomShadow(ShadowFieldUpdate::Opacity(v)),
        ),
      ]
      .spacing(8),
    );
  }

  let curve_slider = inline_f64_slider(
    mode,
    "Curve",
    f64::from(app.options.corner_radius),
    0.0..=80.0,
    1.0,
    format!("{}px", app.options.corner_radius),
    98.0,
    |v| Message::CurveChanged(v.round() as u32),
  );
  let border_slider = inline_f64_slider(
    mode,
    "Border",
    app.options.border_width,
    0.0..=30.0,
    1.0,
    format!("{}px", app.options.border_width.round() as i32),
    98.0,
    Message::BorderWidthChanged,
  );
  let border_col = parse_hex_to_iced_color(
    app.options.border_color.as_deref().unwrap_or("#ffffff"),
    Color::WHITE,
  );
  let border_color_row = row![
    text("Border color")
      .size(12)
      .color(t.text_secondary)
      .width(Length::Fill),
    tooltip(
      button(container(text("")).width(20).height(20))
        .on_press(Message::OpenColorPicker(ColorPickerTarget::BorderColor))
        .padding(0)
        .width(20)
        .height(20)
        .style(swatch_button_style(mode, border_col, false, 999.0)),
      styled_tooltip(mode, "Pick border color"),
      tooltip::Position::Top,
    ),
  ]
  .align_y(Alignment::Center);

  appearance_col = appearance_col
    .push(divider_h(mode))
    .push(curve_slider)
    .push(border_slider)
    .push(border_color_row);

  let appearance_card = sidebar_card(mode, appearance_col.into());

  // 3) Browser frame card (OS Mockup + Reset + Safari text + UI scale)
  let mockup_items = [
    ("mac-light", "Mac Light", mockup_mac_light_handle()),
    ("mac-dark", "Mac Dark", mockup_mac_dark_handle()),
    ("windows-light", "Windows Light", mockup_win_light_handle()),
    ("windows-dark", "Windows Dark", mockup_win_dark_handle()),
    ("safari-dark", "Safari", mockup_safari_dark_handle()),
  ];

  let mut mockup_grid = row![].spacing(4).width(Length::Fill);
  for (id, label, thumb) in mockup_items {
    let active = app.options.browser_bar == id;
    let next = if active { "none" } else { id };
    let thumb_btn = tooltip(
      button(
        container(
          iced_image(thumb)
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Cover),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true),
      )
      .on_press(Message::SelectMockup(next.to_string()))
      .padding(3)
      .width(Length::Fill)
      .height(38)
      .style(thumb_tile_button_style(mode, active, 8.0)),
      styled_tooltip(mode, label),
      tooltip::Position::Top,
    );
    mockup_grid = mockup_grid.push(container(thumb_btn).width(Length::FillPortion(1)));
  }

  let has_mockup = app.options.browser_bar != "none";
  let mut mockup_col = column![
    row![
      text("Browser frame")
        .size(12)
        .color(t.text_primary)
        .width(Length::Fill),
      button(
        row![
          svg(rotate_ccw_muted_svg())
            .width(11)
            .height(11)
            .style(svg_tint_style(if has_mockup {
              t.text_muted
            } else {
              t.text_disabled
            })),
          text("Reset").size(11).color(if has_mockup {
            t.text_muted
          } else {
            t.text_disabled
          }),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
      )
      .on_press_maybe(has_mockup.then_some(Message::SelectMockup("none".to_string())))
      .padding([2, 6])
      .style(ghost_button_style(mode)),
    ]
    .align_y(Alignment::Center),
    mockup_grid,
  ]
  .spacing(9);

  if app.options.browser_bar == "safari-dark" {
    let url_val = app.options.browser_url.as_deref().unwrap_or("");
    mockup_col = mockup_col.push(
      text_input("Safari text", url_val)
        .on_input(Message::SafariUrlChanged)
        .padding([5, 8])
        .size(12)
        .style(input_style(mode)),
    );
  }

  let ui_scale_slider = inline_f64_slider(
    mode,
    "UI scale",
    app.options.browser_frame.scale,
    0.5..=2.0,
    0.1,
    format!("{:.1}x", app.options.browser_frame.scale),
    98.0,
    Message::UiScaleChanged,
  );
  mockup_col = mockup_col.push(ui_scale_slider);

  let mockup_card = sidebar_card(mode, mockup_col.into());

  // 4) Watermark card (interactive row + expandable details when enabled)
  let current_wm_pos = WM_POSITIONS
    .iter()
    .copied()
    .find(|p| p.value.eq_ignore_ascii_case(&app.options.watermark.position))
    .unwrap_or(WM_POSITIONS[0]);
  let current_pfx = app.options.watermark.prefix.as_deref().unwrap_or("none");

  let watermark_header_row = button(
    row![
      column![
        text("Watermark").size(12).color(t.text_primary),
        text(if app.options.watermark.enabled {
          "Overlay badge active"
        } else {
          "Show custom handle or brand"
        })
        .size(10)
        .color(t.text_muted),
      ]
      .spacing(1)
      .width(Length::Fill),
      checkbox("", app.options.watermark.enabled)
        .on_toggle(Message::WatermarkEnabledToggled)
        .size(15)
        .style(checkbox_style(mode)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .width(Length::Fill),
  )
  .on_press(Message::WatermarkEnabledToggled(!app.options.watermark.enabled))
  .padding([4, 4])
  .width(Length::Fill)
  .style(ghost_button_style(mode));

  let mut watermark_col = column![watermark_header_row].spacing(9);

  if app.options.watermark.enabled {
    watermark_col = watermark_col
      .push(divider_h(mode))
      .push(
        row![
          text("Position")
            .size(12)
            .color(t.text_secondary)
            .width(Length::Fill),
          pick_list(WM_POSITIONS, Some(current_wm_pos), |opt| {
            Message::WatermarkPositionSelected(opt.value.to_string())
          })
          .padding([5, 8])
          .text_size(11)
          .style(pick_list_style(mode))
          .menu_style(pick_list_menu_style(mode)),
        ]
        .align_y(Alignment::Center),
      )
      .push(
        text_input("Made with shotify", &app.options.watermark.text)
          .on_input(Message::WatermarkTextChanged)
          .padding([5, 8])
          .size(12)
          .style(input_style(mode)),
      )
      .push(
        row![
          container(
            checkbox("Twitter", current_pfx.eq_ignore_ascii_case("twitter"))
              .on_toggle(|on| {
                Message::WatermarkPrefixSelected(if on { "twitter" } else { "none" }.to_string())
              })
              .size(14)
              .spacing(6)
              .text_size(11)
              .style(checkbox_style(mode)),
          )
          .width(Length::FillPortion(1)),
          container(
            checkbox("Email", current_pfx.eq_ignore_ascii_case("mail"))
              .on_toggle(|on| {
                Message::WatermarkPrefixSelected(if on { "mail" } else { "none" }.to_string())
              })
              .size(14)
              .spacing(6)
              .text_size(11)
              .style(checkbox_style(mode)),
          )
          .width(Length::FillPortion(1)),
        ]
        .spacing(6),
      )
      .push(
        row![
          container(
            checkbox("YouTube", current_pfx.eq_ignore_ascii_case("youtube"))
              .on_toggle(|on| {
                Message::WatermarkPrefixSelected(if on { "youtube" } else { "none" }.to_string())
              })
              .size(14)
              .spacing(6)
              .text_size(11)
              .style(checkbox_style(mode)),
          )
          .width(Length::FillPortion(1)),
          container(
            checkbox("Verified", app.options.watermark.show_verified)
              .on_toggle(Message::WatermarkVerifiedToggled)
              .size(14)
              .spacing(6)
              .text_size(11)
              .style(checkbox_style(mode)),
          )
          .width(Length::FillPortion(1)),
        ]
        .spacing(6),
      );
  }

  let watermark_card = sidebar_card(mode, watermark_col.into());

  // 5) Collapsible "More position settings" Claude disclosure card (single-level card + hairline divider)
  let more_chevron = if app.more_position_open {
    chevron_down_svg()
  } else {
    chevron_right_svg()
  };

  let has_3d_offsets = app.options.tilt.x.abs() > 0.1
    || app.options.tilt.y.abs() > 0.1
    || app.options.position.rotate.abs() > 0.1
    || app.options.position.x.abs() > 0.1
    || app.options.position.y.abs() > 0.1;

  let more_header = button(
    row![
      text("Position & tilt")
        .size(12)
        .color(t.text_primary)
        .width(Length::Fill),
      container(
        text(if app.more_position_open {
          "Open"
        } else if has_3d_offsets {
          "Modified"
        } else {
          "3D & offsets"
        })
        .size(10)
        .color(if has_3d_offsets {
          t.text_primary
        } else {
          t.text_muted
        }),
      )
      .padding([2, 7])
      .style(readout_chip_style(mode, has_3d_offsets)),
      svg(more_chevron)
        .width(14)
        .height(14)
        .style(svg_tint_style(t.text_secondary)),
    ]
    .spacing(6)
    .align_y(Alignment::Center),
  )
  .on_press(Message::ToggleMorePosition)
  .padding([4, 4])
  .width(Length::Fill)
  .style(ghost_button_style(mode));

  let mut more_col = column![more_header].spacing(9);
  if app.more_position_open {
    let wm_x_valid = app.watermark_offset_x_input.trim().parse::<f64>().is_ok();
    let wm_y_valid = app.watermark_offset_y_input.trim().parse::<f64>().is_ok();
    let pos_x_valid = app.card_pos_x_input.trim().parse::<f64>().is_ok();
    let pos_y_valid = app.card_pos_y_input.trim().parse::<f64>().is_ok();

    more_col = more_col.push(divider_h(mode)).push(
      column![
        row![
          column![
            text("Watermark X").size(11).color(t.text_secondary),
            text_input("0", &app.watermark_offset_x_input)
              .on_input(Message::WatermarkOffsetXInputChanged)
              .padding([4, 8])
              .size(12)
              .style(validated_input_style(mode, wm_x_valid)),
          ]
          .spacing(4)
          .width(Length::FillPortion(1)),
          column![
            text("Watermark Y").size(11).color(t.text_secondary),
            text_input("0", &app.watermark_offset_y_input)
              .on_input(Message::WatermarkOffsetYInputChanged)
              .padding([4, 8])
              .size(12)
              .style(validated_input_style(mode, wm_y_valid)),
          ]
          .spacing(4)
          .width(Length::FillPortion(1)),
        ]
        .spacing(10),
        inline_f64_slider(
          mode,
          "Tilt X",
          app.options.tilt.x,
          -50.0..=50.0,
          1.0,
          format!("{}°", app.options.tilt.x.round() as i32),
          94.0,
          Message::TiltXChanged,
        ),
        inline_f64_slider(
          mode,
          "Tilt Y",
          app.options.tilt.y,
          -50.0..=50.0,
          1.0,
          format!("{}°", app.options.tilt.y.round() as i32),
          94.0,
          Message::TiltYChanged,
        ),
        inline_f64_slider(
          mode,
          "Rotation",
          app.options.position.rotate,
          -50.0..=50.0,
          1.0,
          format!("{}°", app.options.position.rotate.round() as i32),
          94.0,
          Message::RotationChanged,
        ),
        row![
          row![
            text("X").size(11).color(t.text_secondary),
            container(
              text_input("0", &app.card_pos_x_input)
                .on_input(Message::CardPositionXInputChanged)
                .padding([4, 8])
                .size(12)
                .style(validated_input_style(mode, pos_x_valid)),
            )
            .width(64),
          ]
          .spacing(6)
          .align_y(Alignment::Center),
          horizontal_space(),
          row![
            text("Y").size(11).color(t.text_secondary),
            container(
              text_input("0", &app.card_pos_y_input)
                .on_input(Message::CardPositionYInputChanged)
                .padding([4, 8])
                .size(12)
                .style(validated_input_style(mode, pos_y_valid)),
            )
            .width(64),
          ]
          .spacing(6)
          .align_y(Alignment::Center),
        ]
        .align_y(Alignment::Center),
      ]
      .spacing(9),
    );
  }

  let more_card = sidebar_card(mode, more_col.into());

  let body = column![
    backgrounds_panel(app),
    frame_card,
    appearance_card,
    mockup_card,
    watermark_card,
    more_card,
  ]
  .spacing(10)
  .padding(iced::Padding {
    top: 12.0,
    right: 12.0,
    bottom: 20.0,
    left: 12.0,
  })
  .width(Length::Fill);

  container(
    scrollable(body)
      .height(Length::Fill)
      .direction(minimal_scrollbar_direction())
      .style(subtle_scrollable_style(mode)),
  )
  .width(270)
  .height(Length::Fill)
  .style(chrome_sidebar_style(mode))
  .into()
}
