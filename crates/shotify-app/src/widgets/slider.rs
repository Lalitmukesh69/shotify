use std::ops::RangeInclusive;

use iced::widget::{container, row, slider as iced_slider, text, tooltip};
use iced::{Alignment, Element, Length};

use crate::message::Message;
use crate::theme::{dark_slider_style, readout_chip_style, tokens, ThemeMode};
use crate::widgets::common::styled_tooltip;

fn render_inline_slider<'a>(
  mode: ThemeMode,
  label: &'a str,
  label_size: u16,
  value: f64,
  range: RangeInclusive<f64>,
  step: f64,
  value_text: String,
  slider_width: f32,
  on_change: impl Fn(f64) -> Message + 'a,
) -> Element<'a, Message> {
  let t = tokens(mode);

  let track = tooltip(
    iced_slider(range, value, on_change)
      .step(step)
      .on_release(Message::SliderReleased)
      .style(dark_slider_style(mode))
      .width(Length::Fixed(slider_width)),
    styled_tooltip(mode, value_text.clone()),
    tooltip::Position::Top,
  );

  let value_pill = container(text(value_text).size(10).color(t.text_secondary))
    .padding([1, 6])
    .style(readout_chip_style(mode, false));

  row![
    row![
      text(label).size(label_size).color(t.text_secondary),
      value_pill,
    ]
    .spacing(5)
    .align_y(Alignment::Center)
    .width(Length::Fill),
    track,
  ]
  .width(Length::Fill)
  .align_y(Alignment::Center)
  .spacing(6)
  .into()
}

/// Horizontal inline slider control (`Label` + muted readout on left, purple slider on right)
/// with live drag/hover value readout matching `ui/src/components/RangeWithTooltip.jsx`.
pub fn inline_f64_slider<'a>(
  mode: ThemeMode,
  label: &'a str,
  value: f64,
  range: RangeInclusive<f64>,
  step: f64,
  value_text: String,
  slider_width: f32,
  on_change: impl Fn(f64) -> Message + 'a,
) -> Element<'a, Message> {
  render_inline_slider(
    mode,
    label,
    12,
    value,
    range,
    step,
    value_text,
    slider_width,
    on_change,
  )
}

/// Narrow inline slider showing compact numeric readout beside the label (for Custom shadow box).
pub fn inline_f64_slider_with_readout<'a>(
  mode: ThemeMode,
  label: &'a str,
  value: f64,
  range: RangeInclusive<f64>,
  step: f64,
  value_text: String,
  slider_width: f32,
  on_change: impl Fn(f64) -> Message + 'a,
) -> Element<'a, Message> {
  render_inline_slider(
    mode,
    label,
    11,
    value,
    range,
    step,
    value_text,
    slider_width,
    on_change,
  )
}
