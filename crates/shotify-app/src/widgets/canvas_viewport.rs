use std::sync::LazyLock;

use iced::border::Radius;
use iced::widget::{
  button, column, container, horizontal_space, image as iced_image, mouse_area, responsive, row,
  stack, text, text_input,
};
use iced::{Alignment, Border, Color, Element, Length, Size};

use crate::message::{Message, ShortcutFieldKey};
use crate::state::ShotifyApp;
use crate::theme::{
  empty_card_style, ghost_button_style, input_style, modal_panel_style, pill_button_style,
  purple_primary_button_style, tokens,
};
use crate::widgets::editor_canvas::AnnotationCanvasProgram;

pub static TEXT_PROMPT_INPUT_ID: LazyLock<text_input::Id> =
  LazyLock::new(|| text_input::Id::new("annotation-text-prompt-input"));

fn fit_dimensions(doc_w: f32, doc_h: f32, max_w: f32, max_h: f32) -> (f32, f32) {
  if doc_w <= 0.0 || doc_h <= 0.0 || max_w <= 10.0 || max_h <= 10.0 {
    return (320.0, 200.0);
  }
  let scale = (max_w / doc_w).min(max_h / doc_h).min(1.0).max(0.05);
  ((doc_w * scale).round(), (doc_h * scale).round())
}

fn shortcut_empty_row<'a>(
  app: &'a ShotifyApp,
  field: ShortcutFieldKey,
  label: &'a str,
  combo: &'a str,
) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);
  let is_recording = app.recording_shortcut == Some(field);

  let chip = container(
    text(if is_recording { "Press keys…" } else { combo })
      .size(14)
      .color(if is_recording {
        Color::WHITE
      } else {
        t.text_primary
      }),
  )
  .padding([5, 11])
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
        t.border_strong
      },
      width: 1.0,
      radius: Radius::from(999.0),
    },
    ..Default::default()
  });

  let btn = button(text(if is_recording { "Cancel" } else { "Change" }).size(12))
    .on_press(if is_recording {
      Message::CancelRecordingShortcut
    } else {
      Message::StartRecordingShortcut(field)
    })
    .padding([5, 12])
    .height(28)
    .style(pill_button_style(mode, is_recording, 6.0));

  row![
    row![text(label).size(15).color(t.text_secondary), chip]
      .spacing(10)
      .align_y(Alignment::Center)
      .width(Length::Fill),
    btn,
  ]
  .spacing(12)
  .align_y(Alignment::Center)
  .width(Length::Fill)
  .into()
}

/// Center viewport rendering either `ui/src/CenterPreview.jsx` empty shortcut card or
/// the composed screenshot + interactive annotations.
pub fn canvas_viewport<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  if app.raw_screenshot.is_none() {
    let sc = &app.config.shortcuts;
    let paste_label = if cfg!(target_os = "macos") {
      "Paste (Cmd+V)"
    } else {
      "Paste (Ctrl+V)"
    };

    let empty_card = container(
      column![
        shortcut_empty_row(
          app,
          ShortcutFieldKey::ScreenshotRegion,
          "Take screenshot with",
          &sc.screenshot_region,
        ),
        shortcut_empty_row(
          app,
          ShortcutFieldKey::ScreenshotRegionCopy,
          "Take screenshot and copy result with",
          &sc.screenshot_region_copy,
        ),
        container(
          button(text("Take screenshot").size(13))
            .on_press(Message::StartRegionCaptureRequested)
            .padding([6, 18])
            .height(32)
            .style(purple_primary_button_style(mode, 6.0)),
        )
        .padding(iced::Padding {
          top: 6.0,
          right: 0.0,
          bottom: 0.0,
          left: 0.0,
        })
        .width(Length::Fill)
        .center_x(Length::Fill),
        container(
          row![
            text("or drop image here ·").size(12).color(t.text_muted),
            button(text("Open image…").size(12))
              .on_press(Message::OpenImageDialogRequested)
              .padding([4, 10])
              .style(pill_button_style(mode, false, 6.0)),
            button(text(paste_label).size(12))
              .on_press(Message::PasteFromClipboardRequested)
              .padding([4, 10])
              .style(pill_button_style(mode, false, 6.0)),
          ]
          .spacing(8)
          .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .center_x(Length::Fill),
      ]
      .spacing(16)
      .width(Length::Fill),
    )
    .padding(iced::Padding {
      top: 24.0,
      right: 26.0,
      bottom: 24.0,
      left: 26.0,
    })
    .max_width(820)
    .width(Length::Fill)
    .style(empty_card_style(mode));

    return container(empty_card)
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .into();
  }

  let Some(handle) = app.preview_handle.clone() else {
    return container(text("Rendering composition…").size(13).color(t.text_muted))
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .into();
  };

  let (doc_w, doc_h) = app
    .composed_image
    .as_ref()
    .map(|img| (f64::from(img.width()), f64::from(img.height())))
    .unwrap_or((1200.0, 675.0));

  container(responsive(move |avail: Size| {
    let (fit_w, fit_h) = fit_dimensions(
      doc_w as f32,
      doc_h as f32,
      (avail.width - 24.0).max(100.0),
      (avail.height - 16.0).max(100.0),
    );

    let base_img = iced_image(handle.clone())
      .width(Length::Fixed(fit_w))
      .height(Length::Fixed(fit_h));

    let overlay_canvas = iced::widget::canvas(AnnotationCanvasProgram {
      document: &app.annotations.document,
      active_tool: app.annotations.active_tool,
      selection: app.annotations.selection,
      card_dragging: app.card_dragging,
      doc_w,
      doc_h,
    })
    .width(Length::Fixed(fit_w))
    .height(Length::Fixed(fit_h));

    let mut stage = stack![base_img, overlay_canvas];

    if let Some(ref prompt) = app.inline_text_prompt {
      let prompt_card = mouse_area(
        container(
          column![
            text(if prompt.edit_index.is_some() {
              "Edit Annotation Text"
            } else {
              "Add Text Annotation"
            })
            .size(12)
            .color(t.text_primary),
            text_input("Type annotation text and press Enter…", &prompt.draft)
              .id(TEXT_PROMPT_INPUT_ID.clone())
              .on_input(Message::TextPromptChanged)
              .on_submit(Message::TextPromptCommit)
              .padding(8)
              .size(13)
              .style(input_style(mode)),
            row![
              horizontal_space(),
              button(text("Cancel (Esc)").size(11))
                .on_press(Message::TextPromptCancel)
                .padding([5, 10])
                .style(ghost_button_style(mode)),
              button(text("Apply (Enter)").size(11))
                .on_press(Message::TextPromptCommit)
                .padding([5, 12])
                .style(purple_primary_button_style(mode, 6.0)),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .width(Length::Fill),
          ]
          .spacing(10)
          .width(280),
        )
        .padding(14)
        .style(modal_panel_style(mode, 12.0)),
      )
      .on_press(Message::NoOp);

      let backdrop = mouse_area(
        container(prompt_card)
          .width(Length::Fixed(fit_w))
          .height(Length::Fixed(fit_h))
          .center_x(Length::Fill)
          .center_y(Length::Fill)
          .style(|_| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.35).into()),
            ..Default::default()
          }),
      )
      .on_press(Message::TextPromptCancel);
      stage = stage.push(backdrop);
    }

    container(stage)
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .into()
  }))
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}
