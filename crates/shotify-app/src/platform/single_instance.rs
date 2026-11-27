use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use tokio::sync::mpsc::UnboundedSender;

use crate::config::app_config_dir;
use crate::platform::PlatformCommand;

/// CLI action requested when launching `shotify-rust`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchCommand {
  Show,
  CaptureRegion,
  CaptureRegionCopy,
}

impl LaunchCommand {
  pub fn wire_token(self) -> &'static str {
    match self {
      Self::Show => "SHOW",
      Self::CaptureRegion => "CAPTURE",
      Self::CaptureRegionCopy => "CAPTURE_COPY",
    }
  }

  pub fn from_wire_token(token: &str) -> Option<PlatformCommand> {
    match token.trim() {
      "SHOW" => Some(PlatformCommand::ShowWindow),
      "CAPTURE" => Some(PlatformCommand::CaptureRegion),
      "CAPTURE_COPY" => Some(PlatformCommand::CaptureRegionCopy),
      "SETTINGS" => Some(PlatformCommand::OpenSettings),
      _ => None,
    }
  }
}

/// Parse CLI arguments into a single-instance wake action.
pub fn parse_cli_launch_command<I, S>(args: I) -> LaunchCommand
where
  I: IntoIterator<Item = S>,
  S: AsRef<str>,
{
  let mut out = LaunchCommand::Show;
  for arg in args {
    match arg.as_ref().trim() {
      "--capture-copy" => return LaunchCommand::CaptureRegionCopy,
      "--capture" => out = LaunchCommand::CaptureRegion,
      _ => {}
    }
  }
  out
}

/// Return true if user explicitly passed `--allow-multiple` to bypass single-instance lock.
pub fn should_allow_multiple<I, S>(args: I) -> bool
where
  I: IntoIterator<Item = S>,
  S: AsRef<str>,
{
  args.into_iter().any(|a| a.as_ref().trim() == "--allow-multiple")
}

/// Return true if user passed `--minimized` or `--hidden` on startup.
pub fn start_hidden_flag<I, S>(args: I) -> bool
where
  I: IntoIterator<Item = S>,
  S: AsRef<str>,
{
  args
    .into_iter()
    .any(|a| matches!(a.as_ref().trim(), "--minimized" | "--hidden"))
}

/// Guard holding the primary instance socket/lock endpoint until clean shutdown.
pub struct SingleInstanceGuard {
  endpoint_path: PathBuf,
}

impl Drop for SingleInstanceGuard {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.endpoint_path);
  }
}

/// Outcome of attempting to acquire the primary instance lock.
pub enum SingleInstanceOutcome {
  /// This process won the lock and now hosts the IPC wake listener.
  Primary(SingleInstanceGuard),
  /// An existing instance is active and received `LaunchCommand`.
  ForwardedToPrimary,
}

/// Compute default IPC socket/port path for `shotify-rust`.
pub fn default_ipc_endpoint_path() -> PathBuf {
  #[cfg(unix)]
  {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
      let trimmed = runtime.trim();
      if !trimmed.is_empty() {
        return PathBuf::from(trimmed).join("shotify-rust.sock");
      }
    }
    if let Some(cfg) = app_config_dir() {
      return cfg.join("instance.sock");
    }
    std::env::temp_dir().join("shotify-rust.sock")
  }

  #[cfg(not(unix))]
  {
    if let Some(cfg) = app_config_dir() {
      return cfg.join("instance.port");
    }
    std::env::temp_dir().join("shotify-rust.port")
  }
}

/// Try connecting to an existing primary `shotify-rust` instance or become primary at `endpoint_path`.
pub fn acquire_or_forward_at(
  endpoint_path: &Path,
  cmd: LaunchCommand,
  tx: UnboundedSender<PlatformCommand>,
) -> Result<SingleInstanceOutcome, String> {
  if let Some(parent) = endpoint_path.parent() {
    let _ = fs::create_dir_all(parent);
  }

  #[cfg(unix)]
  {
    use std::os::unix::net::{UnixListener, UnixStream};

    if let Ok(mut stream) = UnixStream::connect(endpoint_path) {
      let _ = writeln!(stream, "{}", cmd.wire_token());
      let _ = stream.flush();
      return Ok(SingleInstanceOutcome::ForwardedToPrimary);
    }

    if endpoint_path.exists() {
      let _ = fs::remove_file(endpoint_path);
    }

    let listener = UnixListener::bind(endpoint_path)
      .map_err(|e| format!("Failed binding instance socket {}: {e}", endpoint_path.display()))?;

    let _ = std::thread::Builder::new()
      .name("shotify-ipc".to_string())
      .spawn(move || {
        for incoming in listener.incoming() {
          let Ok(stream) = incoming else {
            continue;
          };
          let mut reader = BufReader::new(stream);
          let mut line = String::new();
          if reader.read_line(&mut line).is_ok() {
            if let Some(cmd) = LaunchCommand::from_wire_token(&line) {
              let _ = tx.send(cmd);
            }
          }
        }
      });

    Ok(SingleInstanceOutcome::Primary(SingleInstanceGuard {
      endpoint_path: endpoint_path.to_path_buf(),
    }))
  }

  #[cfg(not(unix))]
  {
    use std::net::{TcpListener, TcpStream};

    if let Ok(port_raw) = fs::read_to_string(endpoint_path) {
      if let Ok(port) = port_raw.trim().parse::<u16>() {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
          let _ = writeln!(stream, "{}", cmd.wire_token());
          let _ = stream.flush();
          return Ok(SingleInstanceOutcome::ForwardedToPrimary);
        }
      }
    }

    let listener = TcpListener::bind("127.0.0.1:0")
      .map_err(|e| format!("Failed binding loopback IPC listener: {e}"))?;
    let port = listener
      .local_addr()
      .map_err(|e| format!("Cannot read local port: {e}"))?
      .port();
    fs::write(endpoint_path, port.to_string())
      .map_err(|e| format!("Failed writing instance port file: {e}"))?;

    let _ = std::thread::Builder::new()
      .name("shotify-ipc".to_string())
      .spawn(move || {
        for incoming in listener.incoming() {
          let Ok(stream) = incoming else {
            continue;
          };
          let mut reader = BufReader::new(stream);
          let mut line = String::new();
          if reader.read_line(&mut line).is_ok() {
            if let Some(cmd) = LaunchCommand::from_wire_token(&line) {
              let _ = tx.send(cmd);
            }
          }
        }
      });

    Ok(SingleInstanceOutcome::Primary(SingleInstanceGuard {
      endpoint_path: endpoint_path.to_path_buf(),
    }))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::{SystemTime, UNIX_EPOCH};

  fn temp_sock_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos();
    std::env::temp_dir().join(format!("shotify_rust_{tag}_{nanos}.sock"))
  }

  #[test]
  fn cli_argument_parsing_matches_flags() {
    assert_eq!(
      parse_cli_launch_command(["shotify-rust"]),
      LaunchCommand::Show
    );
    assert_eq!(
      parse_cli_launch_command(["shotify-rust", "--capture"]),
      LaunchCommand::CaptureRegion
    );
    assert_eq!(
      parse_cli_launch_command(["shotify-rust", "--capture", "--capture-copy"]),
      LaunchCommand::CaptureRegionCopy
    );
    assert!(should_allow_multiple(["shotify-rust", "--allow-multiple"]));
    assert!(start_hidden_flag(["shotify-rust", "--minimized"]));
  }

  #[test]
  fn second_instance_forwards_command_to_primary() {
    let sock = temp_sock_path("ipc_forward");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let first = acquire_or_forward_at(&sock, LaunchCommand::Show, tx.clone()).unwrap();
    let SingleInstanceOutcome::Primary(guard) = first else {
      panic!("First instance should acquire primary lock");
    };

    let second = acquire_or_forward_at(&sock, LaunchCommand::CaptureRegionCopy, tx).unwrap();
    assert!(matches!(second, SingleInstanceOutcome::ForwardedToPrimary));

    // Wait briefly for the background IPC thread to deliver the message.
    std::thread::sleep(std::time::Duration::from_millis(40));
    assert_eq!(rx.try_recv().ok(), Some(PlatformCommand::CaptureRegionCopy));

    drop(guard);
    assert!(!sock.exists());
  }
}
