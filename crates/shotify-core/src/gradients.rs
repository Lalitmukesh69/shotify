use serde::{Deserialize, Serialize};

use crate::color::{to_canvas_rgba, Mulberry32};
use crate::defaults::{BlobGradientConfig, CustomTheme};
use crate::helpers::canvas_angle_deg_to_css_angle_deg;
use crate::math::clamp_f64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GradientPreset {
  Gradient {
    id: String,
    name: String,
    config: CustomTheme,
    #[serde(rename = "previewCss")]
    preview_css: String,
  },
  BlobGradient {
    id: String,
    name: String,
    config: BlobGradientConfig,
    #[serde(rename = "previewCss")]
    preview_css: String,
  },
}

struct LinearDef {
  id: &'static str,
  name: &'static str,
  color_start: &'static str,
  color_end: &'static str,
  angle_deg: f64,
}

const CURATED_SKY_LINEAR: &[LinearDef] = &[
  LinearDef { id: "cur:sky-lavender", name: "Sky Lavender", color_start: "#60A5FA", color_end: "#A78BFA", angle_deg: 135.0 },
  LinearDef { id: "cur:sunrise-sky", name: "Sunrise Sky", color_start: "#FDBA74", color_end: "#60A5FA", angle_deg: 135.0 },
  LinearDef { id: "cur:cotton-candy", name: "Cotton Candy", color_start: "#FBCFE8", color_end: "#BFDBFE", angle_deg: 135.0 },
  LinearDef { id: "cur:mint-breeze", name: "Mint Breeze", color_start: "#A7F3D0", color_end: "#BAE6FD", angle_deg: 135.0 },
  LinearDef { id: "cur:horizon-glow", name: "Horizon Glow", color_start: "#38BDF8", color_end: "#FDE68A", angle_deg: 135.0 },
  LinearDef { id: "cur:blue-hour", name: "Blue Hour", color_start: "#0EA5E9", color_end: "#6366F1", angle_deg: 135.0 },
  LinearDef { id: "cur:peach-cloud", name: "Peach Cloud", color_start: "#FED7AA", color_end: "#F5D0FE", angle_deg: 135.0 },
  LinearDef { id: "cur:seafoam", name: "Seafoam", color_start: "#5EEAD4", color_end: "#93C5FD", angle_deg: 135.0 },
  LinearDef { id: "cur:lemon-sky", name: "Lemon Sky", color_start: "#FDE68A", color_end: "#93C5FD", angle_deg: 135.0 },
  LinearDef { id: "cur:rose-mist", name: "Rose Mist", color_start: "#FDA4AF", color_end: "#C7D2FE", angle_deg: 135.0 },
  LinearDef { id: "cur:aqua-bloom", name: "Aqua Bloom", color_start: "#67E8F9", color_end: "#F9A8D4", angle_deg: 135.0 },
  LinearDef { id: "cur:spring-day", name: "Spring Day", color_start: "#86EFAC", color_end: "#93C5FD", angle_deg: 135.0 },
];

struct CssBlobPalette {
  name: &'static str,
  base: &'static str,
  colors: &'static [&'static str],
}

const CSS_BLOB_PALETTES: &[CssBlobPalette] = &[
  CssBlobPalette {
    name: "Blush Pop",
    base: "hsla(0,100%,50%,1)",
    colors: &["hsla(28,100%,74%,1)", "hsla(189,100%,56%,1)", "hsla(355,100%,93%,1)", "hsla(340,100%,76%,1)", "hsla(22,100%,77%,1)", "hsla(242,100%,70%,1)", "hsla(343,100%,76%,1)"],
  },
  CssBlobPalette {
    name: "Pink Confetti",
    base: "#ff99bb",
    colors: &["hsla(295,85%,65%,1)", "hsla(30,70%,75%,1)", "hsla(76,94%,67%,1)", "hsla(276,74%,63%,1)", "hsla(169,75%,74%,1)", "hsla(226,93%,60%,1)", "hsla(28,87%,67%,1)"],
  },
  CssBlobPalette {
    name: "Aqua Sprinkle",
    base: "#99ecff",
    colors: &["hsla(200,71%,65%,1)", "hsla(350,60%,70%,1)", "hsla(308,94%,62%,1)", "hsla(154,90%,61%,1)", "hsla(331,62%,72%,1)", "hsla(125,87%,71%,1)", "hsla(336,62%,60%,1)"],
  },
  CssBlobPalette {
    name: "Periwinkle Haze",
    base: "#99a3ff",
    colors: &["hsla(250,77%,62%,1)", "hsla(321,60%,62%,1)", "hsla(140,82%,74%,1)", "hsla(254,90%,70%,1)", "hsla(18,60%,69%,1)", "hsla(330,61%,78%,1)", "hsla(226,63%,69%,1)"],
  },
  CssBlobPalette {
    name: "Icy Carnival",
    base: "#99f0ff",
    colors: &["hsla(1,93%,78%,1)", "hsla(0,35%,78%,1)", "hsla(57,76%,78%,1)", "hsla(301,77%,66%,1)", "hsla(338,65%,62%,1)", "hsla(23,76%,66%,1)", "hsla(342,64%,74%,1)"],
  },
  CssBlobPalette {
    name: "Lavender Pool",
    base: "#9c99ff",
    colors: &["hsla(238,99%,74%,1)", "hsla(187,94%,62%,1)", "hsla(209,89%,76%,1)", "hsla(337,98%,60%,1)", "hsla(116,80%,68%,1)", "hsla(3,72%,71%,1)", "hsla(336,60%,72%,1)"],
  },
  CssBlobPalette {
    name: "Candy Bloom",
    base: "#ff99ce",
    colors: &["hsla(176,87%,72%,1)", "hsla(171,89%,62%,1)", "hsla(172,91%,74%,1)", "hsla(237,85%,60%,1)", "hsla(240,98%,72%,1)", "hsla(157,99%,65%,1)", "hsla(218,91%,75%,1)"],
  },
  CssBlobPalette {
    name: "Sky Petals",
    base: "#99afff",
    colors: &["hsla(158,73%,66%,1)", "hsla(218,62%,64%,1)", "hsla(307,66%,65%,1)", "hsla(149,78%,61%,1)", "hsla(175,96%,77%,1)", "hsla(317,82%,72%,1)", "hsla(166,65%,74%,1)"],
  },
  CssBlobPalette {
    name: "Peach Smoothie",
    base: "#ffc799",
    colors: &["hsla(138,81%,71%,1)", "hsla(116,94%,73%,1)", "hsla(36,82%,64%,1)", "hsla(319,62%,69%,1)", "hsla(47,74%,72%,1)", "hsla(131,70%,64%,1)", "hsla(111,78%,73%,1)"],
  },
  CssBlobPalette {
    name: "Apricot Mist",
    base: "#ffbd99",
    colors: &["hsla(207,76%,65%,1)", "hsla(350,69%,69%,1)", "hsla(269,69%,78%,1)", "hsla(157,97%,76%,1)", "hsla(231,71%,63%,1)", "hsla(242,93%,74%,1)", "hsla(276,80%,78%,1)"],
  },
  CssBlobPalette {
    name: "Lime Soda",
    base: "#dfff99",
    colors: &["hsla(99,98%,62%,1)", "hsla(190,99%,68%,1)", "hsla(342,83%,78%,1)", "hsla(301,83%,62%,1)", "hsla(220,72%,72%,1)", "hsla(357,97%,78%,1)", "hsla(182,73%,65%,1)"],
  },
];

fn custom_blob_presets() -> Vec<GradientPreset> {
  let defs = [
    (
      "blob:user-electric-sky",
      "Electric Sky",
      BlobGradientConfig {
        base_mode: "light".into(),
        base_a: Some("hsla(240,100%,59%,0.47)".into()),
        base_b: Some("hsla(240,100%,59%,0.47)".into()),
        colors: vec![
          "hsla(198,71%,73%,1)".into(),
          "hsla(283,100%,56%,1)".into(),
          "hsla(300,100%,50%,1)".into(),
          "hsla(189,100%,50%,1)".into(),
          "hsla(266,100%,50%,1)".into(),
        ],
        seed: 830001,
        blob_count: 9,
        intensity: 0.76,
        angle_deg: 135.0,
        ..BlobGradientConfig::default()
      },
    ),
    (
      "blob:user-neon-flamingo",
      "Neon Flamingo",
      BlobGradientConfig {
        base_mode: "light".into(),
        base_a: Some("hsla(324,100%,50%,1)".into()),
        base_b: Some("hsla(324,100%,50%,1)".into()),
        colors: vec![
          "hsla(235,100%,79%,1)".into(),
          "hsla(189,100%,56%,1)".into(),
          "hsla(355,100%,93%,1)".into(),
          "hsla(340,100%,50%,1)".into(),
          "hsla(0,100%,70%,1)".into(),
          "hsla(201,100%,67%,1)".into(),
          "hsla(312,100%,50%,1)".into(),
        ],
        seed: 830002,
        blob_count: 9,
        intensity: 0.76,
        angle_deg: 135.0,
        ..BlobGradientConfig::default()
      },
    ),
    (
      "blob:user-sunset-surge",
      "Sunset Surge",
      BlobGradientConfig {
        base_mode: "light".into(),
        base_a: Some("hsla(359,100%,68%,1)".into()),
        base_b: Some("hsla(359,100%,68%,1)".into()),
        colors: vec![
          "hsla(202,90%,65%,1)".into(),
          "hsla(300,100%,50%,1)".into(),
          "hsla(172,100%,50%,1)".into(),
          "hsla(239,100%,60%,1)".into(),
          "hsla(360,100%,50%,1)".into(),
          "hsla(275,85%,76%,1)".into(),
          "hsla(238,67%,70%,1)".into(),
        ],
        seed: 830003,
        blob_count: 9,
        intensity: 0.76,
        angle_deg: 135.0,
        ..BlobGradientConfig::default()
      },
    ),
  ];

  defs
    .into_iter()
    .map(|(id, name, cfg)| {
      let base_a = cfg.base_a.as_deref().unwrap_or("#ffffff");
      let base_b = cfg.base_b.as_deref().unwrap_or("#f3f4f6");
      let css_angle = canvas_angle_deg_to_css_angle_deg(cfg.angle_deg);
      GradientPreset::BlobGradient {
        id: id.into(),
        name: name.into(),
        preview_css: format!("linear-gradient({css_angle}deg, {base_a}, {base_b})"),
        config: cfg,
      }
    })
    .collect()
}

/// Generate the 56 deterministic gradient presets shown in the UI (`useGradients.js`).
pub fn generate_algorithm_gradients() -> Vec<GradientPreset> {
  let target_count = 56usize;
  let angles = [45.0, 90.0, 120.0, 135.0, 160.0, 200.0, 225.0, 260.0, 300.0, 315.0];

  let mut out: Vec<GradientPreset> = CURATED_SKY_LINEAR
    .iter()
    .map(|d| {
      let css_a = canvas_angle_deg_to_css_angle_deg(d.angle_deg);
      GradientPreset::Gradient {
        id: d.id.to_string(),
        name: d.name.to_string(),
        preview_css: format!("linear-gradient({css_a}deg, {}, {})", d.color_start, d.color_end),
        config: CustomTheme {
          color_start: d.color_start.to_string(),
          color_end: d.color_end.to_string(),
          angle_deg: d.angle_deg,
        },
      }
    })
    .collect();

  out.extend(custom_blob_presets());

  let half_generated = (target_count - CURATED_SKY_LINEAR.len()) / 2;
  let linear_target = half_generated.saturating_sub(14);
  for i in 0..linear_target {
    let h = (i * 37) % 360;
    let a = format!("hsl({h}, 90%, 60%)");
    let b = format!("hsl({}, 90%, 48%)", (h + 130 + (i % 3) * 25) % 360);
    let angle = angles[i % angles.len()];
    let css_a = canvas_angle_deg_to_css_angle_deg(angle);
    out.push(GradientPreset::Gradient {
      id: format!("lin:{i}"),
      name: format!("Gradient {}", CURATED_SKY_LINEAR.len() + i + 1),
      preview_css: format!("linear-gradient({css_a}deg, {a}, {b})"),
      config: CustomTheme {
        color_start: a,
        color_end: b,
        angle_deg: angle,
      },
    });
  }

  let mut blob_idx = 0usize;
  while out.len() < target_count {
    let p = &CSS_BLOB_PALETTES[blob_idx % CSS_BLOB_PALETTES.len()];
    let variant = blob_idx / CSS_BLOB_PALETTES.len();
    let seed = 500_001u32.wrapping_add((blob_idx as u32).wrapping_mul(9973));
    let name = if variant > 0 {
      format!("{} {}", p.name, variant + 1)
    } else {
      p.name.to_string()
    };
    let cfg = BlobGradientConfig {
      base_mode: "light".into(),
      base_a: Some(p.base.into()),
      base_b: Some(p.base.into()),
      colors: p.colors.iter().map(|s| (*s).to_string()).collect(),
      seed,
      blob_count: 9,
      intensity: 0.76,
      angle_deg: 135.0,
      ..BlobGradientConfig::default()
    };
    let css_a = canvas_angle_deg_to_css_angle_deg(cfg.angle_deg);
    out.push(GradientPreset::BlobGradient {
      id: format!("blob:{seed}"),
      name,
      preview_css: format!("linear-gradient({css_a}deg, {}, {})", p.base, p.base),
      config: cfg,
    });
    blob_idx += 1;
  }

  out.truncate(target_count);
  out
}

/// Generate Auto tab blob gradients derived from an extracted screenshot palette.
/// Port of `ui/src/hooks/useGradients.js` auto-gradients generator.
pub fn generate_auto_gradients(palette_colors: &[String], base_seed: u32) -> Vec<GradientPreset> {
  let fallback = [
    "#8B5CF6", "#EC4899", "#22C55E", "#06B6D4", "#F59E0B", "#0EA5E9", "#A855F7",
  ];
  let base: Vec<String> = if palette_colors.is_empty() {
    fallback.iter().map(|s| (*s).to_string()).collect()
  } else {
    palette_colors.iter().take(7).cloned().collect()
  };

  let mut rand = Mulberry32::new(base_seed);
  let pick_colors = |r: &mut Mulberry32, count: usize| -> Vec<String> {
    (0..count)
      .map(|_| {
        let idx = (r.next_f64() * (base.len() as f64)).floor() as usize;
        base[idx.min(base.len() - 1)].clone()
      })
      .collect()
  };

  (0..14)
    .map(|i| {
      let base_mode = if rand.next_f64() < 0.55 { "dark" } else { "light" };
      let colors = pick_colors(&mut rand, 8);
      let seed = base_seed.wrapping_add((i as u32).wrapping_mul(9973));
      let blob_count = 6 + (rand.next_f64() * 5.0).floor() as u32;
      let intensity = 0.70 + rand.next_f64() * 0.18;
      let angle_deg = (30.0 + rand.next_f64() * 300.0).floor();
      let base_a = if base_mode == "light" { "#ffffff" } else { "#05060A" };
      let base_b = if base_mode == "light" { "#f3f4f6" } else { "#111827" };
      let css_a = canvas_angle_deg_to_css_angle_deg(angle_deg);

      GradientPreset::BlobGradient {
        id: format!("auto:{seed}"),
        name: format!("Auto {}", i + 1),
        preview_css: format!("linear-gradient({css_a}deg, {base_a}, {base_b})"),
        config: BlobGradientConfig {
          colors,
          seed,
          blob_count,
          intensity,
          angle_deg,
          base_mode: base_mode.to_string(),
          base_a: None,
          base_b: None,
          blend: None,
          corner_bias: None,
          blob_blur_px: None,
        },
      }
    })
    .collect()
}

/// Generate a CSS `background-image` preview string approximating a blob gradient.
/// Port of `utils/image/blobGradientPreview.js::blobGradientPreviewCss`.
pub fn blob_gradient_preview_css(cfg: &BlobGradientConfig) -> String {
  let base_mode = if cfg.base_mode.is_empty() { "dark" } else { &cfg.base_mode };
  let base_a = cfg
    .base_a
    .as_deref()
    .unwrap_or(if base_mode == "light" { "#ffffff" } else { "#05060A" });
  let base_b = cfg
    .base_b
    .as_deref()
    .unwrap_or(if base_mode == "light" { "#f3f4f6" } else { "#111827" });
  let css_angle_deg = canvas_angle_deg_to_css_angle_deg(cfg.angle_deg);
  let intensity = clamp_f64(cfg.intensity, 0.0, 1.0);
  let blob_count = cfg.blob_count.clamp(3, 10);

  let mut rand = Mulberry32::from_nonzero_seed(cfg.seed);
  let pick = |idx: usize, fallback: &str| -> String {
    if cfg.colors.is_empty() {
      fallback.to_string()
    } else {
      cfg.colors[idx % cfg.colors.len()].clone()
    }
  };

  let a0 = if base_mode == "light" { intensity * 0.30 } else { intensity * 0.70 };
  let a1 = if base_mode == "light" { intensity * 0.18 } else { intensity * 0.42 };

  let mut layers = Vec::with_capacity(1 + (blob_count as usize) * 2);
  let base = format!("linear-gradient({css_angle_deg}deg, {base_a}, {base_b})");
  layers.push(base);

  for i in 0..blob_count as usize {
    for j in 0..2usize {
      let x = (6.0 + rand.next_f64() * 88.0).round();
      let y = (6.0 + rand.next_f64() * 88.0).round();
      let stop = (42.0 + rand.next_f64() * 30.0).round();
      let col = pick(i + j, if i % 2 == 1 { "#ec4899" } else { "#8b5cf6" });
      let alpha = (if i == 0 { a0 } else { a1 }) * (if j == 0 { 1.0 } else { 0.55 });
      layers.push(format!(
        "radial-gradient(circle at {x}% {y}%, {} 0%, {} {stop}%)",
        to_canvas_rgba(&col, alpha),
        to_canvas_rgba(&col, 0.0),
      ));
    }
  }

  layers.join(", ")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn generates_56_algorithm_presets() {
    let presets = generate_algorithm_gradients();
    assert_eq!(presets.len(), 56);
  }

  #[test]
  fn generates_14_deterministic_auto_presets() {
    let pal = vec!["#FF0000".to_string(), "#00FF00".to_string(), "#0000FF".to_string()];
    let a = generate_auto_gradients(&pal, 42);
    let b = generate_auto_gradients(&pal, 42);
    assert_eq!(a.len(), 14);
    assert_eq!(a, b);
  }
}
