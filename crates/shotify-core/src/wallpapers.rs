use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// System wallpaper entry discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemWallpaperItem {
  pub name: String,
  pub file_path: String,
}

/// Check whether `path` is a valid system wallpaper file (skipping thumbnail folders/suffixes).
pub fn is_wallpaper_image(path: &Path) -> bool {
  let ext = path
    .extension()
    .and_then(|s| s.to_str())
    .unwrap_or("")
    .to_lowercase();
  let fname = path
    .file_name()
    .and_then(|s| s.to_str())
    .unwrap_or("")
    .to_lowercase();
  let path_str = path.to_string_lossy().to_lowercase();

  #[cfg(target_os = "windows")]
  {
    let windows_special = fname == "transcodedwallpaper"
      || fname.starts_with("transcoded_")
      || fname.starts_with("cachedimage_");
    if windows_special {
      return true;
    }
  }

  let in_macos_thumbnails = path
    .components()
    .any(|c| c.as_os_str().to_string_lossy().to_lowercase() == ".thumbnails");

  let valid_ext = {
    #[cfg(target_os = "macos")]
    {
      matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "heic" | "heif" | "avif" | "mov" | "mp4" | "m4v"
      )
    }
    #[cfg(target_os = "linux")]
    {
      matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "heic" | "heif" | "avif" | "svg"
      )
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "linux")))]
    {
      matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "heic" | "heif" | "avif"
      )
    }
  };

  let is_thumb = (fname.contains("thumb") || fname.contains("thumbnail") || path_str.contains("/thumb"))
    && !in_macos_thumbnails;

  valid_ext && !is_thumb
}

/// Recursively scan `root` for wallpaper images up to `max` items.
pub fn collect_images_recursive(root: &Path, out: &mut Vec<PathBuf>, max: usize) {
  if out.len() >= max || !root.exists() {
    return;
  }

  let max_depth: u8 = 10;
  let max_dirs: usize = 2_500;
  let mut visited_dirs: usize = 0;
  let mut stack: Vec<(PathBuf, u8)> = vec![(root.to_path_buf(), 0)];

  while let Some((dir, depth)) = stack.pop() {
    if out.len() >= max || visited_dirs >= max_dirs {
      break;
    }
    visited_dirs += 1;

    let Ok(entries) = std::fs::read_dir(&dir) else {
      continue;
    };

    for entry in entries.flatten() {
      if out.len() >= max {
        break;
      }
      let path = entry.path();
      let Ok(ftype) = entry.file_type() else {
        continue;
      };
      if ftype.is_symlink() {
        continue;
      }
      if ftype.is_dir() {
        #[cfg(target_os = "macos")]
        {
          let dir_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
          if dir_name == ".thumbnails" || dir_name == ".wallpapers" {
            continue;
          }
        }
        if depth < max_depth {
          stack.push((path, depth + 1));
        }
        continue;
      }

      if ftype.is_file() && is_wallpaper_image(&path) {
        out.push(path);
      }
    }
  }
}

/// Discover system wallpapers from the provided roots and direct paths, deduplicated and sorted.
pub fn list_system_wallpapers_from_paths(
  roots: &[PathBuf],
  direct_paths: &[PathBuf],
  max_items: u32,
) -> Vec<SystemWallpaperItem> {
  let max = max_items.clamp(1, 500) as usize;
  let mut paths: Vec<PathBuf> = Vec::new();
  let mut seen: HashSet<String> = HashSet::new();

  for root in roots {
    let before = paths.len();
    collect_images_recursive(root, &mut paths, max);
    if paths.len() != before {
      let mut deduped = Vec::with_capacity(paths.len());
      for p in paths.drain(..) {
        if seen.insert(p.to_string_lossy().to_lowercase()) {
          deduped.push(p);
        }
      }
      paths = deduped;
    }
    if paths.len() >= max {
      break;
    }
  }

  for p in direct_paths {
    if paths.len() >= max {
      break;
    }
    if p.is_file() && is_wallpaper_image(p) && seen.insert(p.to_string_lossy().to_lowercase()) {
      paths.push(p.clone());
    }
  }

  paths.sort_by(|a, b| {
    let an = a.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let bn = b.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    an.cmp(&bn)
  });

  paths
    .into_iter()
    .take(max)
    .map(|p| SystemWallpaperItem {
      name: p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Wallpaper")
        .to_string(),
      file_path: p.to_string_lossy().to_string(),
    })
    .collect()
}

/// Collect standard platform wallpaper root directories for the current OS.
pub fn default_os_wallpaper_roots() -> Vec<PathBuf> {
  let mut roots = Vec::new();

  #[cfg(target_os = "macos")]
  {
    roots.push(PathBuf::from("/System/Library/Desktop Pictures/.wallpapers"));
    roots.push(PathBuf::from("/System/Library/Desktop Pictures/.thumbnails"));
    roots.push(PathBuf::from("/System/Library/Desktop Pictures"));
    roots.push(PathBuf::from("/Library/Desktop Pictures"));
  }

  #[cfg(target_os = "windows")]
  {
    roots.push(PathBuf::from(r"C:\Windows\Web\Wallpaper"));
    roots.push(PathBuf::from(r"C:\Windows\Web\Screen"));
    roots.push(PathBuf::from(r"C:\Windows\Web\4K\Wallpaper"));
    if let Ok(appdata) = std::env::var("APPDATA") {
      roots.push(PathBuf::from(appdata).join("Microsoft").join("Windows").join("Themes"));
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
      roots.push(PathBuf::from(localappdata).join("Microsoft").join("Windows").join("Themes"));
    }
  }

  #[cfg(target_os = "linux")]
  {
    roots.push(PathBuf::from("/usr/share/backgrounds"));
    roots.push(PathBuf::from("/usr/share/wallpapers"));
    roots.push(PathBuf::from("/usr/local/share/backgrounds"));
    roots.push(PathBuf::from("/usr/local/share/wallpapers"));
    if let Ok(home) = std::env::var("HOME") {
      roots.push(PathBuf::from(&home).join(".local").join("share").join("backgrounds"));
      roots.push(PathBuf::from(&home).join(".local").join("share").join("wallpapers"));
    }
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
      roots.push(PathBuf::from(&xdg_data_home).join("backgrounds"));
      roots.push(PathBuf::from(&xdg_data_home).join("wallpapers"));
    }
  }

  roots
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn filters_wallpapers_and_excludes_thumbs() {
    assert!(is_wallpaper_image(Path::new("/a/b/c.jpg")));
    assert!(is_wallpaper_image(Path::new("/a/b/c.png")));
    assert!(!is_wallpaper_image(Path::new("/a/b/thumbs/c.jpg")));
    assert!(!is_wallpaper_image(Path::new("/a/b/c_thumb.png")));
    assert!(!is_wallpaper_image(Path::new("/a/b/c.txt")));
  }
}
