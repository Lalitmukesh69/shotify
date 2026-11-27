use std::sync::LazyLock;

use iced::widget::{
  button, column, container, horizontal_space, mouse_area, row, text, text_input,
};
use iced::{Alignment, Color, Element, Length};

use crate::message::Message;
use crate::state::ShotifyApp;
use crate::theme::{
  footer_action_button_style, ghost_button_style, input_style, modal_panel_style, tokens,
};

pub static PRESET_NAME_INPUT_ID: LazyLock<text_input::Id> =
  LazyLock::new(|| text_input::Id::new("preset-name-input"));

/// Modal dialog for saving or renaming a preset matching `ui/src/PresetSaveDialog.jsx`.
pub fn preset_save_dialog<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);
  let is_rename = app.preset_dialog_renaming_id.is_some();
  let can_submit = !app.preset_name_draft.trim().is_empty();

  let modal = container(
    column![
      text(if is_rename {
        "Rename Preset"
      } else {
        "Save Preset"
      })
      .size(13)
      .color(t.text_primary),
      text(if is_rename {
        "Choose a new name for this preset."
      } else {
        "Give your preset a name to save the current settings."
      })
      .size(11)
      .color(t.text_muted),
      text_input("My preset", &app.preset_name_draft)
        .id(PRESET_NAME_INPUT_ID.clone())
        .on_input(Message::PresetNameDraftChanged)
        .on_submit(Message::ConfirmSavePreset)
        .padding([8, 12])
        .size(13)
        .style(input_style(mode)),
      row![
        horizontal_space(),
        button(text("Cancel").size(12))
          .on_press(Message::CloseSavePresetDialog)
          .padding([7, 14])
          .style(ghost_button_style(mode)),
        button(text(if is_rename { "Rename" } else { "Save" }).size(12))
          .on_press_maybe(can_submit.then_some(Message::ConfirmSavePreset))
          .padding([7, 16])
          .style(footer_action_button_style(mode, true)),
      ]
      .spacing(8)
      .align_y(Alignment::Center)
      .width(Length::Fill),
    ]
    .spacing(12)
    .width(320),
  )
  .padding(20)
  .style(modal_panel_style(mode, 12.0));

  mouse_area(
    container(mouse_area(modal).on_press(Message::NoOp))
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .style(|_| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.50).into()),
        ..Default::default()
      }),
  )
  .on_press(Message::CloseSavePresetDialog)
  .into()
}
