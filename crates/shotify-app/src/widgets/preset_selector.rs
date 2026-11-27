use iced::widget::{button, container, pick_list, row, text, tooltip};
use iced::{Alignment, Element};

use crate::message::Message;
use crate::state::ShotifyApp;
use crate::theme::{
  danger_pill_button_style, pick_list_menu_style, pick_list_style, pill_button_style, tokens,
};
use crate::widgets::common::styled_tooltip;

/// Compact top-right preset dropdown matching `ui/src/PresetSelectorTopRight.jsx`.
pub fn preset_selector<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  let mut options: Vec<String> = vec!["Default".to_string(), "Current".to_string()];
  for p in &app.user_presets {
    options.push(p.name.clone());
  }
  options.push("Save current as preset…".to_string());

  let active_user_preset = app
    .user_presets
    .iter()
    .find(|p| p.id == app.active_preset_id);

  let selected_label = if app.active_preset_id == "builtin:default"
    || app.active_preset_id == "default"
  {
    Some("Default".to_string())
  } else if app.active_preset_id == "current" {
    Some("Current".to_string())
  } else {
    active_user_preset
      .map(|p| p.name.clone())
      .or_else(|| Some("Current".to_string()))
  };

  let picker = pick_list(options, selected_label, Message::PresetDropdownSelected)
    .padding([6, 12])
    .width(136)
    .text_size(12)
    .style(pick_list_style(mode))
    .menu_style(pick_list_menu_style(mode));

  let mut items = row![picker].spacing(4).align_y(Alignment::Center);

  if let Some(preset) = active_user_preset {
    if app.confirm_delete_preset_id.as_deref() == Some(preset.id.as_str()) {
      items = items
        .push(tooltip(
          button(text("Delete?").size(11))
            .on_press(Message::DeletePreset(preset.id.clone()))
            .padding([5, 8])
            .style(danger_pill_button_style(mode)),
          styled_tooltip(mode, "Confirm permanent preset deletion"),
          tooltip::Position::Bottom,
        ))
        .push(
          button(text("Cancel").size(11).color(t.text_secondary))
            .on_press(Message::CancelDeletePreset)
            .padding([5, 8])
            .style(pill_button_style(mode, false, 6.0)),
        );
    } else {
      items = items
        .push(tooltip(
          button(text("Rename").size(11).color(t.text_secondary))
            .on_press(Message::OpenRenamePresetDialog(preset.id.clone()))
            .padding([5, 8])
            .style(pill_button_style(mode, false, 6.0)),
          styled_tooltip(mode, "Rename active preset"),
          tooltip::Position::Bottom,
        ))
        .push(tooltip(
          button(text("×").size(13).color(t.accent_error))
            .on_press(Message::RequestDeletePreset(preset.id.clone()))
            .padding([4, 8])
            .style(pill_button_style(mode, false, 6.0)),
          styled_tooltip(mode, "Delete active preset"),
          tooltip::Position::Bottom,
        ));
    }
  }

  container(items).into()
}
