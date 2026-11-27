use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length};

use crate::message::Message;
use crate::theme::{readout_chip_style, tokens, tooltip_container_style, ThemeMode};

/// Styled popover tooltip box with backdrop, hairline border, and drop shadow.
pub fn styled_tooltip<'a>(
  mode: ThemeMode,
  label: impl text::IntoFragment<'a>,
) -> Element<'a, Message> {
  let t = tokens(mode);
  container(text(label).size(11).color(t.text_primary))
    .padding([4, 8])
    .style(tooltip_container_style(mode))
    .into()
}

/// 1px horizontal hairline divider matching panel borders.
pub fn divider_h<'a>(mode: ThemeMode) -> Element<'a, Message> {
  let t = tokens(mode);
  container(text(""))
    .height(1)
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(t.border.into()),
      ..Default::default()
    })
    .into()
}

/// 1px vertical hairline divider for toolbar button groups.
pub fn divider_v<'a>(mode: ThemeMode, height: f32) -> Element<'a, Message> {
  let t = tokens(mode);
  container(text(""))
    .width(1)
    .height(height)
    .style(move |_| container::Style {
      background: Some(t.border_strong.into()),
      ..Default::default()
    })
    .into()
}

/// Editorial card title row with optional trailing hairline metadata pill.
pub fn card_section_header<'a>(
  mode: ThemeMode,
  title: &'a str,
  badge: Option<(&'a str, bool)>,
) -> Element<'a, Message> {
  let t = tokens(mode);
  let mut header = row![
    text(title)
      .size(12)
      .color(t.text_primary)
      .width(Length::Fill),
  ]
  .spacing(6)
  .align_y(Alignment::Center)
  .width(Length::Fill);

  if let Some((badge_text, accent)) = badge {
    header = header.push(
      container(text(badge_text).size(10).color(if accent {
        t.text_primary
      } else {
        t.text_muted
      }))
      .padding([2, 7])
      .style(readout_chip_style(mode, accent)),
    );
  }

  header.into()
}
