use serde::Deserialize;

pub const GITHUB_RELEASES_API_URL: &str =
  "https://api.github.com/repos/Lalitmukesh69/shotify/releases/latest";
pub const GITHUB_REPO_WEB_URL: &str = "https://github.com/Lalitmukesh69/shotify";
pub const GITHUB_ISSUES_URL: &str = "https://github.com/Lalitmukesh69/shotify/issues";

/// Update checker state rendered in Settings -> About.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateCheckStatus {
  #[default]
  Idle,
  Checking,
  UpToDate {
    latest: String,
  },
  UpdateAvailable {
    version: String,
    url: String,
    notes: String,
  },
  Failed(String),
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseDto {
  tag_name: Option<String>,
  html_url: Option<String>,
  body: Option<String>,
}

fn parse_semver_tuple(raw: &str) -> Option<(u64, u64, u64)> {
  let clean = raw.trim().trim_start_matches(['v', 'V']);
  let core = clean.split(['-', '+']).next().unwrap_or(clean);
  let mut parts = core.split('.');
  let major = parts.next()?.parse::<u64>().ok()?;
  let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
  let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
  Some((major, minor, patch))
}

/// Compare two version strings and return true when `latest` is strictly newer than `current`.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
  match (parse_semver_tuple(latest), parse_semver_tuple(current)) {
    (Some(l), Some(c)) => l > c,
    _ => false,
  }
}

/// Parse a GitHub `/releases/latest` JSON payload into an `UpdateCheckStatus`.
pub fn parse_github_release_json(
  body: &str,
  current_version: &str,
) -> Result<UpdateCheckStatus, String> {
  let dto: GitHubReleaseDto =
    serde_json::from_str(body).map_err(|e| format!("Invalid GitHub release payload: {e}"))?;
  let raw_tag = dto
    .tag_name
    .ok_or_else(|| "Latest release response missing tag_name".to_string())?;
  let clean_ver = raw_tag.trim().trim_start_matches(['v', 'V']).to_string();
  let url = dto
    .html_url
    .filter(|u| !u.trim().is_empty())
    .unwrap_or_else(|| format!("{GITHUB_REPO_WEB_URL}/releases/latest"));

  if is_newer_version(&clean_ver, current_version) {
    let notes = dto
      .body
      .unwrap_or_default()
      .lines()
      .take(4)
      .collect::<Vec<_>>()
      .join(" ");
    Ok(UpdateCheckStatus::UpdateAvailable {
      version: clean_ver,
      url,
      notes,
    })
  } else {
    Ok(UpdateCheckStatus::UpToDate { latest: clean_ver })
  }
}

/// Check GitHub Releases asynchronously without blocking the GUI thread.
pub async fn check_for_updates_async(current_version: String) -> UpdateCheckStatus {
  let client = match reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(8))
    .user_agent(format!("shotify-rust/{current_version}"))
    .build()
  {
    Ok(c) => c,
    Err(err) => return UpdateCheckStatus::Failed(format!("HTTP client error: {err}")),
  };

  let resp = match client
    .get(GITHUB_RELEASES_API_URL)
    .header("Accept", "application/vnd.github+json")
    .send()
    .await
  {
    Ok(r) => r,
    Err(err) => return UpdateCheckStatus::Failed(format!("Network error: {err}")),
  };

  let status = resp.status();
  if status == reqwest::StatusCode::NOT_FOUND {
    return UpdateCheckStatus::UpToDate {
      latest: current_version,
    };
  }
  if !status.is_success() {
    return UpdateCheckStatus::Failed(format!("GitHub API returned HTTP {status}"));
  }

  let body = match resp.text().await {
    Ok(b) => b,
    Err(err) => return UpdateCheckStatus::Failed(format!("Failed reading response: {err}")),
  };

  match parse_github_release_json(&body, &current_version) {
    Ok(outcome) => outcome,
    Err(err) => UpdateCheckStatus::Failed(err),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn semver_comparison_handles_v_prefix_and_prerelease() {
    assert!(is_newer_version("v1.0.1", "1.0.0"));
    assert!(is_newer_version("1.2.0", "1.0.9"));
    assert!(!is_newer_version("v1.0.0", "1.0.0"));
    assert!(!is_newer_version("0.9.9", "1.0.0"));
    assert!(is_newer_version("v1.1.0-rc.1", "1.0.0"));
  }

  #[test]
  fn parses_github_release_json_correctly() {
    let json_newer = r#"{
      "tag_name": "v1.2.0",
      "html_url": "https://github.com/Lalitmukesh69/shotify/releases/tag/v1.2.0",
      "body": "Rust edition speed boost"
    }"#;
    let res = parse_github_release_json(json_newer, "1.0.0").unwrap();
    assert_eq!(
      res,
      UpdateCheckStatus::UpdateAvailable {
        version: "1.2.0".to_string(),
        url: "https://github.com/Lalitmukesh69/shotify/releases/tag/v1.2.0".to_string(),
        notes: "Rust edition speed boost".to_string(),
      }
    );

    let json_same = r#"{"tag_name": "v1.0.0", "html_url": "https://example.com"}"#;
    let res_same = parse_github_release_json(json_same, "1.0.0").unwrap();
    assert_eq!(
      res_same,
      UpdateCheckStatus::UpToDate {
        latest: "1.0.0".to_string(),
      }
    );
  }
}
