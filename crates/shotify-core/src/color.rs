use palette::{Hsl, IntoColor, Srgb};
use serde::{Deserialize, Serialize};
use tiny_skia::{Color as SkiaColor, PremultipliedColorU8};

use crate::math::clamp_f64;

/// Parsed color with 8-bit RGB channels and `[0.0, 1.0]` alpha.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: f32,
}

impl Rgba {
  pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0.0 };
  pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 1.0 };
  pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 1.0 };

  #[inline]
  pub fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
    Self {
      r,
      g,
      b,
      a: if a.is_finite() { a.clamp(0.0, 1.0) } else { 1.0 },
    }
  }

  #[inline]
  pub fn with_alpha_mul(self, alpha_mul: f32) -> Self {
    let mul = if alpha_mul.is_finite() { alpha_mul } else { 0.0 };
    Self::new(self.r, self.g, self.b, (self.a * mul).clamp(0.0, 1.0))
  }

  #[inline]
  pub fn to_hex(self) -> String {
    rgb_to_hex(f64::from(self.r), f64::from(self.g), f64::from(self.b))
  }

  #[inline]
  pub fn to_rgba_css(self) -> String {
    let a = (f64::from(self.a) * 1000.0).round() / 1000.0;
    format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, trim_float(a))
  }

  #[inline]
  pub fn to_skia_color(self) -> SkiaColor {
    SkiaColor::from_rgba8(
      self.r,
      self.g,
      self.b,
      (self.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
  }

  #[inline]
  pub fn to_premultiplied_u8(self) -> PremultipliedColorU8 {
    let a_u8 = (self.a.clamp(0.0, 1.0) * 255.0).round() as u8;
    tiny_skia::ColorU8::from_rgba(self.r, self.g, self.b, a_u8).premultiply()
  }

  #[inline]
  pub fn to_srgb_f32(self) -> Srgb<f32> {
    Srgb::new(
      f32::from(self.r) / 255.0,
      f32::from(self.g) / 255.0,
      f32::from(self.b) / 255.0,
    )
  }
}

fn trim_float(v: f64) -> String {
  if (v - v.round()).abs() < 1e-6 {
    format!("{}", v.round() as i64)
  } else {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
  }
}

/// Convert R, G, B (0..255) to an uppercase hex string `#AABBCC`.
pub fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
  let rr = clamp_f64(r.round(), 0.0, 255.0) as u8;
  let gg = clamp_f64(g.round(), 0.0, 255.0) as u8;
  let bb = clamp_f64(b.round(), 0.0, 255.0) as u8;
  format!("#{rr:02X}{gg:02X}{bb:02X}")
}

/// Parse `#RGB`, `RGB`, `#RRGGBB`, or `RRGGBB` into `(r, g, b)`.
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
  let trimmed = hex.trim();
  let s = trimmed.strip_prefix('#').unwrap_or(trimmed);
  if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
    return None;
  }
  match s.len() {
    3 => {
      let bytes = s.as_bytes();
      let r = u8::from_str_radix(&format!("{}{}", bytes[0] as char, bytes[0] as char), 16).ok()?;
      let g = u8::from_str_radix(&format!("{}{}", bytes[1] as char, bytes[1] as char), 16).ok()?;
      let b = u8::from_str_radix(&format!("{}{}", bytes[2] as char, bytes[2] as char), 16).ok()?;
      Some((r, g, b))
    }
    6 => {
      let r = u8::from_str_radix(&s[0..2], 16).ok()?;
      let g = u8::from_str_radix(&s[2..4], 16).ok()?;
      let b = u8::from_str_radix(&s[4..6], 16).ok()?;
      Some((r, g, b))
    }
    _ => None,
  }
}

/// Check if an arbitrary string looks like a hex color (1..8 hex digits, optional `#`).
/// Port of `utils/color/colorInput.js::isProbablyHexColor`.
pub fn is_probably_hex_color(input: &str) -> bool {
  let raw = input.trim();
  if raw.is_empty() {
    return false;
  }
  let s = raw.strip_prefix('#').unwrap_or(raw);
  if s.is_empty() || s.len() > 8 {
    return false;
  }
  s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Normalize a user-entered hex color to `#RRGGBB` (uppercase).
/// Port of `utils/color/colorInput.js::normalizeHexColorInput`.
pub fn normalize_hex_color_input(input: &str) -> String {
  let raw = input.trim();
  let s = raw.strip_prefix('#').unwrap_or(raw);
  if s.is_empty() || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
    return "#000000".to_string();
  }

  let normalized = match s.len() {
    3 => {
      let mut out = String::with_capacity(6);
      for ch in s.chars() {
        out.push(ch);
        out.push(ch);
      }
      out
    }
    4 => {
      let mut out = String::with_capacity(6);
      for ch in s.chars().take(3) {
        out.push(ch);
        out.push(ch);
      }
      out
    }
    6 => s.to_string(),
    8 => s[0..6].to_string(),
    _ => {
      let mut padded = format!("{s}000000");
      padded.truncate(6);
      padded
    }
  };

  if normalized.len() != 6 {
    "#000000".to_string()
  } else {
    format!("#{}", normalized.to_ascii_uppercase())
  }
}

/// Convert HSL (`h` in `[0, 360)`, `s` and `l` in `[0, 1]`) to `(r, g, b)`.
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
  let hh = ((h % 360.0) + 360.0) % 360.0;
  let ss = clamp_f64(s, 0.0, 1.0);
  let ll = clamp_f64(l, 0.0, 1.0);

  let rgb: Srgb<f32> = Hsl::new(hh as f32, ss as f32, ll as f32).into_color();
  (
    (rgb.red * 255.0).round().clamp(0.0, 255.0) as u8,
    (rgb.green * 255.0).round().clamp(0.0, 255.0) as u8,
    (rgb.blue * 255.0).round().clamp(0.0, 255.0) as u8,
  )
}

fn parse_rgb_func(css: &str) -> Option<Rgba> {
  let lower = css.trim();
  let inner = if let Some(rest) = lower.strip_prefix("rgba(") {
    rest.strip_suffix(')')?
  } else if let Some(rest) = lower.strip_prefix("rgb(") {
    rest.strip_suffix(')')?
  } else {
    return None;
  };

  let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
  if parts.len() != 3 && parts.len() != 4 {
    return None;
  }

  let r = clamp_f64(parts[0].parse::<f64>().ok()?, 0.0, 255.0).round() as u8;
  let g = clamp_f64(parts[1].parse::<f64>().ok()?, 0.0, 255.0).round() as u8;
  let b = clamp_f64(parts[2].parse::<f64>().ok()?, 0.0, 255.0).round() as u8;
  let a = if parts.len() == 4 {
    clamp_f64(parts[3].parse::<f64>().ok()?, 0.0, 1.0) as f32
  } else {
    1.0
  };
  Some(Rgba::new(r, g, b, a))
}

fn parse_hsl_func(css: &str) -> Option<Rgba> {
  let lower = css.trim();
  let inner = if let Some(rest) = lower.strip_prefix("hsla(") {
    rest.strip_suffix(')')?
  } else if let Some(rest) = lower.strip_prefix("hsl(") {
    rest.strip_suffix(')')?
  } else {
    return None;
  };

  let body = inner.trim();
  if body.is_empty() {
    return None;
  }

  // Support both comma-separated and CSS4 space/slash syntax.
  let normalized = body.replace('/', " / ").replace(',', " ");
  let parts: Vec<&str> = normalized.split_whitespace().collect();
  if parts.len() < 3 {
    return None;
  }

  let slash_idx = parts.iter().position(|&p| p == "/");
  let h_raw = parts[0].strip_suffix("deg").or_else(|| parts[0].strip_suffix("DEG")).unwrap_or(parts[0]);
  let s_raw = parts[1].strip_suffix('%').unwrap_or(parts[1]);
  let l_raw = parts[2].strip_suffix('%').unwrap_or(parts[2]);

  let a_raw = match slash_idx {
    Some(idx) => parts.get(idx + 1).copied(),
    None => parts.get(3).copied(),
  };

  let h: f64 = h_raw.parse().ok()?;
  let sp: f64 = s_raw.parse().ok()?;
  let lp: f64 = l_raw.parse().ok()?;
  if !h.is_finite() || !sp.is_finite() || !lp.is_finite() {
    return None;
  }

  let (r, g, b) = hsl_to_rgb(h, sp / 100.0, lp / 100.0);
  let a = match a_raw {
    Some(raw) => clamp_f64(raw.parse::<f64>().ok()?, 0.0, 1.0) as f32,
    None => 1.0,
  };
  Some(Rgba::new(r, g, b, a))
}

/// Deterministic CSS color parser supporting `#RGB`, `#RRGGBB`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, and `transparent`.
/// Port of `utils/color/cssColor.js::parseCssColor` plus `transparent` handling.
pub fn parse_css_color(input: &str) -> Option<Rgba> {
  let s = input.trim();
  if s.is_empty() {
    return None;
  }
  if s.eq_ignore_ascii_case("transparent") {
    return Some(Rgba::TRANSPARENT);
  }
  if s.starts_with('#') || ((s.len() == 3 || s.len() == 6) && s.bytes().all(|b| b.is_ascii_hexdigit())) {
    let (r, g, b) = hex_to_rgb(s)?;
    return Some(Rgba::new(r, g, b, 1.0));
  }

  let lower = s.to_ascii_lowercase();
  if lower.starts_with("rgb(") || lower.starts_with("rgba(") {
    return parse_rgb_func(&lower);
  }
  if lower.starts_with("hsl(") || lower.starts_with("hsla(") {
    return parse_hsl_func(&lower);
  }

  // Fallback for standard CSS named colors via `palette::named`
  if let Some(named) = palette::named::from_str(&lower) {
    return Some(Rgba::new(named.red, named.green, named.blue, 1.0));
  }

  None
}

/// Convert a CSS color string to `rgba(r, g, b, a)`.
/// Port of `utils/color/cssColor.js::rgbaString`.
pub fn rgba_string(color_or_css: &str, alpha_override: Option<f64>) -> String {
  let parsed = parse_css_color(color_or_css);
  let a = match alpha_override {
    Some(ov) => clamp_f64(ov, 0.0, 1.0),
    None => parsed.map_or(1.0, |c| f64::from(c.a)),
  };
  match parsed {
    Some(c) => format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, trim_float(a)),
    None => format!("rgba(0, 0, 0, {})", trim_float(a)),
  }
}

/// Convert a CSS color string with an alpha multiplier to `rgba(r,g,b,a)`.
/// Port of `utils/color/color.js::toCanvasRgba`.
pub fn to_canvas_rgba(color: &str, alpha_mul: f64) -> String {
  let raw = color.trim();
  if raw.is_empty() || raw.eq_ignore_ascii_case("transparent") {
    return "rgba(0,0,0,0)".to_string();
  }
  let mul = if alpha_mul.is_finite() { alpha_mul } else { 0.0 };
  if let Some(parsed) = parse_css_color(raw) {
    let a = clamp_f64(f64::from(parsed.a) * mul, 0.0, 1.0);
    return format!("rgba({},{},{},{})", parsed.r, parsed.g, parsed.b, trim_float(a));
  }
  raw.to_string()
}

/// Multiply alpha values of any `rgba(r, g, b, a)` substrings inside a CSS string.
/// Port of `utils/image/imageDisplayHelpers.js::boostRgbaAlphaInCssValue`.
pub fn boost_rgba_alpha_in_css_value(css_value: &str, factor: f64) -> String {
  if !factor.is_finite() || (factor - 1.0).abs() < 0.0001 {
    return css_value.to_string();
  }

  let mut out = String::with_capacity(css_value.len());
  let mut rest = css_value;
  while let Some(pos) = rest.find("rgba(") {
    out.push_str(&rest[..pos]);
    let after = &rest[pos + 5..];
    if let Some(end) = after.find(')') {
      let inside = &after[..end];
      let parts: Vec<&str> = inside.split(',').map(str::trim).collect();
      if parts.len() == 4 {
        if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
          parts[0].parse::<u32>(),
          parts[1].parse::<u32>(),
          parts[2].parse::<u32>(),
          parts[3].parse::<f64>(),
        ) {
          let alpha = clamp_f64(a * factor, 0.0, 1.0);
          let rounded = (alpha * 1000.0).round() / 1000.0;
          out.push_str(&format!("rgba({r}, {g}, {b}, {})", trim_float(rounded)));
          rest = &after[end + 1..];
          continue;
        }
      }
    }
    out.push_str("rgba(");
    rest = after;
  }
  out.push_str(rest);
  out
}

/// Deterministic PRNG (Mulberry32) matching JS `utils/color/color.js::mulberry32`.
#[derive(Debug, Clone, Copy)]
pub struct Mulberry32 {
  state: u32,
}

impl Mulberry32 {
  /// Create directly from a raw `u32` seed (matching `mulberry32(a >>> 0)`).
  #[inline]
  pub fn new(seed: u32) -> Self {
    Self { state: seed }
  }

  /// Create from optional/JS-truthy seed (`0` becomes `1`, matching `(Number(seed) || 1) >>> 0`).
  #[inline]
  pub fn from_nonzero_seed(seed: u32) -> Self {
    Self {
      state: if seed == 0 { 1 } else { seed },
    }
  }

  /// Generate the next uniform pseudo-random number in `[0.0, 1.0)`.
  #[inline]
  pub fn next_f64(&mut self) -> f64 {
    self.state = self.state.wrapping_add(0x6D2B_79F5);
    let mut x = (self.state ^ (self.state >> 15)).wrapping_mul(1 | self.state);
    x ^= x.wrapping_add((x ^ (x >> 7)).wrapping_mul(61 | x));
    f64::from(x ^ (x >> 14)) / 4_294_967_296.0
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hex_and_rgb_roundtrip() {
    assert_eq!(rgb_to_hex(255.0, 0.0, 128.0), "#FF0080");
    assert_eq!(hex_to_rgb("#abc"), Some((170, 187, 204)));
    assert_eq!(hex_to_rgb("112233"), Some((17, 34, 51)));
    assert_eq!(hex_to_rgb("#zzzzzz"), None);
  }

  #[test]
  fn normalize_hex_matches_js() {
    assert!(is_probably_hex_color("#fff"));
    assert!(!is_probably_hex_color("123456789"));
    assert_eq!(normalize_hex_color_input("#f0a"), "#FF00AA");
    assert_eq!(normalize_hex_color_input("#abcd"), "#AABBCC");
    assert_eq!(normalize_hex_color_input("12"), "#120000");
    assert_eq!(normalize_hex_color_input("#11223344"), "#112233");
  }

  #[test]
  fn css_color_parsing_matches_formats() {
    assert_eq!(parse_css_color("transparent"), Some(Rgba::TRANSPARENT));
    assert_eq!(parse_css_color("rgba(10, 20, 30, 0.5)"), Some(Rgba::new(10, 20, 30, 0.5)));
    let hsl = parse_css_color("hsla(0, 100%, 50%, 0.75)").unwrap();
    assert_eq!((hsl.r, hsl.g, hsl.b), (255, 0, 0));
    assert!((hsl.a - 0.75).abs() < 1e-4);

    let css4 = parse_css_color("hsl(240 100% 50% / 0.25)").unwrap();
    assert_eq!((css4.r, css4.g, css4.b), (0, 0, 255));
    assert!((css4.a - 0.25).abs() < 1e-4);
  }

  #[test]
  fn mulberry32_is_deterministic() {
    let mut a = Mulberry32::from_nonzero_seed(0);
    let mut b = Mulberry32::from_nonzero_seed(1);
    for _ in 0..10 {
      assert_eq!(a.next_f64(), b.next_f64());
    }
  }
}
