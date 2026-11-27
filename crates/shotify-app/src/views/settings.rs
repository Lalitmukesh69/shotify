use iced::border::Radius;
use iced::widget::{
  button, checkbox, column, container, horizontal_space, mouse_area, pick_list, responsive, row,
  scrollable, svg, text, tooltip,
};
use iced::{Alignment, Border, Color, Element, Length, Size};

use crate::config::Language;
use crate::message::{GeneralToggleKey, Message, ShortcutFieldKey};
use crate::platform::updater::{UpdateCheckStatus, GITHUB_ISSUES_URL};
use crate::platform::GlobalShortcutBackend;
use crate::state::ShotifyApp;
use crate::theme::{
  checkbox_style, edition_badge_style, ghost_button_style, minimal_scrollbar_direction,
  modal_panel_style, pick_list_menu_style, pick_list_style, pill_button_style,
  purple_primary_button_style, settings_section_card_style, subtle_scrollable_style,
  svg_tint_style, tokens, ThemeMode,
};
use crate::widgets::common::{divider_h, styled_tooltip};
use crate::widgets::icons::rotate_ccw_muted_svg;

static RUST_LOGO_SVG: &[u8] = include_bytes!("../../../../assets/logo-rust.svg");

fn general_toggle_row<'a>(
  mode: ThemeMode,
  label: &'a str,
  checked: bool,
  key: GeneralToggleKey,
) -> Element<'a, Message> {
  let t = tokens(mode);
  button(
    row![
      text(label)
        .size(13)
        .color(t.text_secondary)
        .width(Length::Fill),
      checkbox("", checked)
        .on_toggle(move |v| Message::ToggleGeneralSetting(key, v))
        .size(16)
        .style(checkbox_style(mode)),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill),
  )
  .on_press(Message::ToggleGeneralSetting(key, !checked))
  .padding([7, 10])
  .width(Length::Fill)
  .style(ghost_button_style(mode))
  .into()
}

fn shortcut_setting_row<'a>(
  app: &'a ShotifyApp,
  field: ShortcutFieldKey,
  label: &'a str,
  scope: &'a str,
  combo: &'a str,
) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);
  let is_recording = app.recording_shortcut == Some(field);
  let chip_text = if combo.trim().is_empty() {
    "Unassigned"
  } else {
    combo
  };

  let chip = container(
    text(if is_recording {
      "Listening…"
    } else {
      chip_text
    })
    .size(11)
    .color(if is_recording {
      Color::WHITE
    } else {
      t.text_primary
    }),
  )
  .padding([4, 9])
  .style(move |_| container::Style {
    background: Some(
      if is_recording {
        t.accent_purple_tint
      } else {
        t.chip_bg
      }
      .into(),
    ),
    border: Border {
      color: if is_recording {
        t.accent_purple
      } else {
        t.border
      },
      width: 1.0,
      radius: Radius::from(6.0),
    },
    ..Default::default()
  });

  let change_btn = button(text(if is_recording { "Cancel" } else { "Change" }).size(11))
    .on_press(if is_recording {
      Message::CancelRecordingShortcut
    } else {
      Message::StartRecordingShortcut(field)
    })
    .padding([5, 9])
    .style(pill_button_style(mode, is_recording, 6.0));

  let reset_btn = tooltip(
    button(
      svg(rotate_ccw_muted_svg())
        .width(12)
        .height(12)
        .style(svg_tint_style(t.text_muted)),
    )
    .on_press(Message::ResetShortcutField(field))
    .padding([6, 7])
    .style(pill_button_style(mode, false, 6.0)),
    styled_tooltip(mode, "Reset to default"),
    tooltip::Position::Top,
  );

  row![
    column![
      text(label).size(12).color(t.text_primary),
      text(scope).size(11).color(t.text_muted),
    ]
    .spacing(1)
    .width(Length::Fill),
    row![chip, change_btn, reset_btn]
      .spacing(6)
      .align_y(Alignment::Center),
  ]
  .padding([6, 10])
  .align_y(Alignment::Center)
  .width(Length::Fill)
  .into()
}

/// Settings modal aligned 1:1 with `components/dialogs/SettingsModal.jsx` + compact Rust utility footer.
pub fn settings_modal<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  responsive(move |avail: Size| settings_modal_for_size(app, avail)).into()
}

fn settings_modal_for_size<'a>(app: &'a ShotifyApp, avail: Size) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);
  let gen_cfg = &app.config.general;
  let sc = &app.config.shortcuts;

  let mut general_rows = column![
    general_toggle_row(
      mode,
      "Hide app when unfocused",
      gen_cfg.hide_app_when_unfocused,
      GeneralToggleKey::HideAppWhenUnfocused,
    ),
    general_toggle_row(
      mode,
      "Keep app in tray",
      gen_cfg.keep_app_in_tray,
      GeneralToggleKey::KeepAppInTray,
    ),
    general_toggle_row(
      mode,
      "Hide window on launch",
      gen_cfg.hide_window_on_launch,
      GeneralToggleKey::HideWindowOnLaunch,
    ),
    general_toggle_row(
      mode,
      "Open at login",
      gen_cfg.open_at_login,
      GeneralToggleKey::OpenAtLogin,
    ),
  ]
  .spacing(2);

  if app.platform_caps.supports_dock_hide {
    general_rows = general_rows.push(general_toggle_row(
      mode,
      "Hide dock icon when app hidden",
      gen_cfg.hide_dock_icon_when_hidden,
      GeneralToggleKey::HideDockIconWhenHidden,
    ));
  }

  general_rows = general_rows
    .push(
      container(divider_h(mode)).padding(iced::Padding {
        top: 6.0,
        right: 0.0,
        bottom: 2.0,
        left: 0.0,
      }),
    )
    .push(
      container(text("Close window after").size(12).color(t.text_muted)).padding(iced::Padding {
        top: 6.0,
        right: 10.0,
        bottom: 2.0,
        left: 10.0,
      }),
    )
    .push(general_toggle_row(
      mode,
      "Copying image to clipboard",
      gen_cfg.close_after_copy,
      GeneralToggleKey::CloseAfterCopy,
    ))
    .push(general_toggle_row(
      mode,
      "Saving image",
      gen_cfg.close_after_save,
      GeneralToggleKey::CloseAfterSave,
    ))
    .push(general_toggle_row(
      mode,
      "Saving to folder",
      gen_cfg.close_after_save_to_folder,
      GeneralToggleKey::CloseAfterSaveToFolder,
    ));

  let left_section = column![
    text("General").size(13).color(t.text_muted),
    container(general_rows)
      .padding(18)
      .width(Length::Fill)
      .style(settings_section_card_style(mode)),
  ]
  .spacing(10)
  .width(Length::FillPortion(1));

  let mut shortcut_rows = column![
    shortcut_setting_row(app, ShortcutFieldKey::Copy, "Copy image", "In app", &sc.copy),
    shortcut_setting_row(app, ShortcutFieldKey::Save, "Save image", "In app", &sc.save),
    shortcut_setting_row(
      app,
      ShortcutFieldKey::SaveToFolder,
      "Save to folder",
      "In app",
      &sc.save_to_folder,
    ),
    shortcut_setting_row(
      app,
      ShortcutFieldKey::ScreenshotRegion,
      "Capture region",
      "Global",
      &sc.screenshot_region,
    ),
    shortcut_setting_row(
      app,
      ShortcutFieldKey::ScreenshotRegionCopy,
      "Capture + copy output",
      "Global",
      &sc.screenshot_region_copy,
    ),
    row![
      text("Press Esc while recording to cancel.")
        .size(11)
        .color(t.text_muted)
        .width(Length::Fill),
      button(text("Reset all").size(11))
        .on_press(Message::ResetAllShortcuts)
        .padding([5, 10])
        .style(pill_button_style(mode, false, 6.0)),
    ]
    .padding([6, 10])
    .align_y(Alignment::Center),
  ]
  .spacing(4);

  if let Some(ref err) = app.shortcut_error {
    shortcut_rows = shortcut_rows.push(
      container(text(err.as_str()).size(11).color(t.accent_error))
        .padding([8, 10])
        .width(Length::Fill)
        .style(move |_| container::Style {
          background: Some(Color { a: 0.12, ..t.accent_error }.into()),
          border: Border {
            color: Color { a: 0.35, ..t.accent_error },
            width: 1.0,
            radius: Radius::from(8.0),
          },
          ..Default::default()
        }),
    );
  }

  if app.platform_caps.is_wayland {
    let hint = match app.platform_caps.global_shortcuts_mode {
      GlobalShortcutBackend::WaylandPortal => {
        "Global shortcuts are registered via the Desktop GlobalShortcuts portal (or use shotify-rust --capture)."
      }
      _ => {
        "Global hotkeys are managed by your Wayland compositor. Bind compositor keys to `shotify-rust --capture` and `shotify-rust --capture-copy`."
      }
    };
    shortcut_rows = shortcut_rows.push(
      container(text(hint).size(11).color(t.text_secondary))
        .padding([8, 10])
        .width(Length::Fill)
        .style(move |_| container::Style {
          background: Some(t.chip_bg.into()),
          border: Border {
            color: t.border,
            width: 1.0,
            radius: Radius::from(8.0),
          },
          ..Default::default()
        }),
    );
  }

  let right_section = column![
    text("Shortcuts").size(13).color(t.text_muted),
    container(shortcut_rows)
      .padding(18)
      .width(Length::Fill)
      .style(settings_section_card_style(mode)),
  ]
  .spacing(10)
  .width(Length::FillPortion(1));

  // Compact Rust utility & update bar below the two SettingsModal cards
  let update_control: Element<'a, Message> = match &app.update_status {
    UpdateCheckStatus::Idle => button(text("Check Updates").size(11))
      .on_press(Message::CheckForUpdatesRequested)
      .padding([5, 10])
      .style(pill_button_style(mode, false, 6.0))
      .into(),
    UpdateCheckStatus::Checking => text("Checking…")
      .size(11)
      .color(t.text_muted)
      .into(),
    UpdateCheckStatus::UpToDate { latest } => button(text(format!("Up to date (v{latest})")).size(11))
      .on_press(Message::CheckForUpdatesRequested)
      .padding([5, 10])
      .style(pill_button_style(mode, false, 6.0))
      .into(),
    UpdateCheckStatus::UpdateAvailable { version, url, .. } => {
      button(text(format!("Download v{version}")).size(11))
        .on_press(Message::OpenExternalUrl(url.clone()))
        .padding([5, 10])
        .style(purple_primary_button_style(mode, 6.0))
        .into()
    }
    UpdateCheckStatus::Failed(err) => button(text(format!("Retry ({err})")).size(10))
      .on_press(Message::CheckForUpdatesRequested)
      .padding([5, 10])
      .style(pill_button_style(mode, false, 6.0))
      .into(),
  };

  let tray_status = if app.platform_caps.tray_available {
    "Tray: Active"
  } else {
    "Tray: Unavailable"
  };

  let rust_footer_strip = container(
    column![
      row![
        row![
          svg(svg::Handle::from_memory(RUST_LOGO_SVG))
            .width(20)
            .height(20),
          text(format!("shotify-rust v{} · {tray_status}", env!("CARGO_PKG_VERSION")))
            .size(12)
            .color(t.text_secondary),
          container(text("NATIVE RUST").size(9))
            .padding([2, 7])
            .style(edition_badge_style(mode)),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill),
        row![
          text("Language").size(11).color(t.text_muted),
          pick_list(Language::ALL, Some(gen_cfg.language), Message::SelectLanguage)
            .padding([4, 10])
            .text_size(11)
            .style(pick_list_style(mode))
            .menu_style(pick_list_menu_style(mode)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
      ]
      .align_y(Alignment::Center)
      .width(Length::Fill),
      row![
        row![
          text("Theme").size(11).color(t.text_muted),
          button(text("Dark").size(11))
            .on_press(Message::SelectThemeMode(ThemeMode::Dark))
            .padding([4, 10])
            .style(pill_button_style(mode, mode == ThemeMode::Dark, 6.0)),
          button(text("Light").size(11))
            .on_press(Message::SelectThemeMode(ThemeMode::Light))
            .padding([4, 10])
            .style(pill_button_style(mode, mode == ThemeMode::Light, 6.0)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        horizontal_space(),
        button(text("Re-import Tauri").size(11))
          .on_press(Message::ImportLegacyTauriRequested)
          .padding([5, 10])
          .style(pill_button_style(mode, false, 6.0)),
        update_control,
        button(text("Report Bug").size(11))
          .on_press(Message::OpenExternalUrl(GITHUB_ISSUES_URL.to_string()))
          .padding([5, 10])
          .style(pill_button_style(mode, false, 6.0)),
      ]
      .spacing(8)
      .align_y(Alignment::Center)
      .width(Length::Fill),
    ]
    .spacing(10),
  )
  .padding([12, 16])
  .width(Length::Fill)
  .style(settings_section_card_style(mode));

  let modal_body = scrollable(
    column![
      row![left_section, right_section]
        .spacing(16)
        .align_y(Alignment::Start)
        .width(Length::Fill),
      rust_footer_strip,
    ]
    .spacing(14)
    .padding(18),
  )
  .height(Length::Shrink)
  .direction(minimal_scrollbar_direction())
  .style(subtle_scrollable_style(mode));

  let header = container(
    row![
      text("Settings")
        .size(16)
        .color(t.text_primary)
        .width(Length::Fill),
      button(text("×").size(16).color(t.text_muted))
        .on_press(Message::CloseSettings)
        .padding([2, 8])
        .style(ghost_button_style(mode)),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill),
  )
  .padding([14, 20])
  .width(Length::Fill);

  let footer = container(
    row![
      horizontal_space(),
      button(text("Done").size(13))
        .on_press(Message::CloseSettings)
        .padding([6, 18])
        .height(30)
        .style(purple_primary_button_style(mode, 6.0)),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill),
  )
  .padding([12, 20])
  .width(Length::Fill);

  let modal_max_w = (avail.width - 40.0).clamp(480.0, 860.0);
  let modal_max_h = (avail.height - 40.0).clamp(320.0, 680.0);
  let body_max_h = (modal_max_h - 124.0).max(160.0);

  let modal = container(
    column![
      header,
      divider_h(mode),
      container(modal_body).max_height(body_max_h),
      divider_h(mode),
      footer,
    ]
    .width(Length::Fill),
  )
  .max_width(modal_max_w)
  .max_height(modal_max_h)
  .padding(8)
  .style(modal_panel_style(mode, 22.0));

  mouse_area(
    container(mouse_area(modal).on_press(Message::NoOp))
      .padding(20)
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .style(|_| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.65).into()),
        ..Default::default()
      }),
  )
  .on_press(Message::CloseSettings)
  .into()
}
