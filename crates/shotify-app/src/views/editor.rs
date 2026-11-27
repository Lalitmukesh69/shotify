use iced::border::Radius;
use iced::widget::{
  button, column, container, horizontal_space, row, stack, svg, text, tooltip, vertical_space,
};
use iced::{Alignment, Border, Element, Length};

use crate::message::Message;
use crate::state::ShotifyApp;
use crate::theme::{
  canvas_backdrop_style, footer_action_button_style, ghost_button_style, svg_tint_style,
  toast_style, tokens, ThemeMode,
};
use crate::views::settings::settings_modal;
use crate::widgets::annotation_toolbar::annotation_toolbar;
use crate::widgets::canvas_viewport::canvas_viewport;
use crate::widgets::color_picker::color_picker_overlay;
use crate::widgets::common::styled_tooltip;
use crate::widgets::export_footer::{export_drawer_popover, export_footer};
use crate::widgets::icons::settings_svg;
use crate::widgets::preset_dialog::preset_save_dialog;
use crate::widgets::preset_selector::preset_selector;
use crate::widgets::sidebar::sidebar;
use crate::widgets::window_controls::window_title_bar;

fn status_pill_or_spacer<'a>(app: &'a ShotifyApp, mode: ThemeMode) -> Element<'a, Message> {
  let label = if app.is_exporting {
    Some("Exporting…")
  } else if app.loading_wallpaper {
    Some("Loading wallpaper…")
  } else if app.is_rendering {
    Some("Rendering…")
  } else {
    None
  };

  let Some(msg) = label else {
    return horizontal_space().into();
  };

  let t = tokens(mode);
  container(
    row![
      container(text(""))
        .width(6)
        .height(6)
        .style(move |_| container::Style {
          background: Some(t.accent_purple.into()),
          border: Border {
            radius: Radius::from(999.0),
            ..Default::default()
          },
          ..Default::default()
        }),
      text(msg).size(11).color(t.text_secondary),
    ]
    .spacing(6)
    .align_y(Alignment::Center),
  )
  .padding([4, 10])
  .style(move |_| container::Style {
    background: Some(t.subcard_bg.into()),
    border: Border {
      color: t.border,
      width: 1.0,
      radius: Radius::from(999.0),
    },
    ..Default::default()
  })
  .into()
}

/// Primary workspace view matching `ui/src/App.jsx` layout:
/// Floating 270px left sidebar, top-center AnnotationBar, top-right PresetSelector,
/// center canvas preview/empty card, bottom-center ExportFooter, bottom-right Settings.
pub fn editor_view<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  let top_bar = row![
    container(status_pill_or_spacer(app, mode))
      .width(Length::FillPortion(1))
      .align_x(iced::alignment::Horizontal::Left),
    container(annotation_toolbar(app)).center_x(Length::Shrink),
    container(preset_selector(app))
      .width(Length::FillPortion(1))
      .align_x(iced::alignment::Horizontal::Right),
  ]
  .width(Length::Fill)
  .align_y(Alignment::Center)
  .spacing(10);

  let settings_btn = tooltip(
    button(
      row![
        svg(settings_svg())
          .width(14)
          .height(14)
          .style(svg_tint_style(t.text_secondary)),
        text("Settings").size(12).color(t.text_primary),
      ]
      .spacing(6)
      .align_y(Alignment::Center),
    )
    .on_press(Message::OpenSettings)
    .padding([5, 10])
    .height(30)
    .style(footer_action_button_style(mode, false)),
    styled_tooltip(mode, "Preferences & shortcuts"),
    tooltip::Position::Top,
  );

  let bottom_bar = row![
    horizontal_space().width(Length::FillPortion(1)),
    container(export_footer(app)).center_x(Length::Shrink),
    horizontal_space().width(Length::FillPortion(1)),
    settings_btn,
  ]
  .width(Length::Fill)
  .align_y(Alignment::Center)
  .spacing(8);

  let workspace = column![
    top_bar,
    container(canvas_viewport(app))
      .width(Length::Fill)
      .height(Length::Fill),
    bottom_bar,
  ]
  .spacing(12)
  .padding(iced::Padding {
    top: 16.0,
    right: 16.0,
    bottom: 16.0,
    left: 24.0,
  })
  .width(Length::Fill)
  .height(Length::Fill);

  let main_row = container(
    row![
      container(sidebar(app))
        .padding(iced::Padding {
          top: 16.0,
          right: 0.0,
          bottom: 16.0,
          left: 16.0,
        })
        .height(Length::Fill),
      workspace,
    ]
    .width(Length::Fill)
    .height(Length::Fill),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .style(canvas_backdrop_style(mode));

  let mut layers = stack![Element::from(main_row)];

  if app.export_drawer_open {
    layers = layers.push(export_drawer_popover(app));
  }
  if app.active_color_picker.is_some() {
    layers = layers.push(color_picker_overlay(app));
  }

  if !app.toasts.is_empty() {
    let mut toast_col = column![].spacing(6).align_x(Alignment::Center);
    for toast in &app.toasts {
      let tid = toast.id;
      let dot_color = if toast.is_error {
        t.accent_error
      } else if toast.is_success {
        t.accent_purple
      } else {
        t.text_muted
      };
      let status_dot = container(text(""))
        .width(6)
        .height(6)
        .style(move |_| container::Style {
          background: Some(dot_color.into()),
          border: Border {
            radius: Radius::from(999.0),
            ..Default::default()
          },
          ..Default::default()
        });

      toast_col = toast_col.push(
        container(
          row![
            status_dot,
            text(&toast.message).size(12).color(t.text_primary),
            button(text("×").size(13).color(t.text_muted))
              .on_press(Message::DismissToast(tid))
              .padding([2, 6])
              .style(ghost_button_style(mode)),
          ]
          .spacing(8)
          .align_y(Alignment::Center),
        )
        .max_width(420)
        .padding([9, 14])
        .style(toast_style(mode, toast.is_error, toast.is_success)),
      );
    }

    layers = layers.push(
      container(
        column![vertical_space(), toast_col]
          .width(Length::Fill)
          .height(Length::Fill)
          .align_x(Alignment::Center)
          .padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 64.0,
            left: 310.0,
          }),
      )
      .width(Length::Fill)
      .height(Length::Fill),
    );
  }

  if app.preset_dialog_open {
    layers = layers.push(preset_save_dialog(app));
  }
  if app.settings_open {
    layers = layers.push(settings_modal(app));
  }

  let content: Element<'a, Message> = layers.into();
  if cfg!(target_os = "macos") {
    content
  } else {
    column![
      window_title_bar(app),
      container(content).width(Length::Fill).height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
  }
}
