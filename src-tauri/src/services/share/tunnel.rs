//! A Cloudflare quick tunnel (`cloudflared tunnel --url`), which needs no
//! account. The public link is read from cloudflared's log.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::app::error::AppError;

pub struct Tunnel {
    child: Child,
    pub url: String,
}

/// The first trycloudflare.com link in a log line.
pub fn find_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '|' || c == '"')
        .unwrap_or(rest.len());
    let url = &rest[..end];
    let host = url.trim_start_matches("https://");
    (host.ends_with(".trycloudflare.com")
        && host.len() > ".trycloudflare.com".len()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.'))
    .then(|| url.to_string())
}

impl Tunnel {
    pub fn open(bin: &Path, port: u16) -> Result<Self, AppError> {
        let mut cmd = Command::new(bin);
        cmd.args([
            "tunnel",
            "--no-autoupdate",
            "--url",
            &format!("http://127.0.0.1:{port}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
        crate::app::lifecycle::hide_console_window(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::internal(format!("cloudflared did not start: {e}")))?;
        crate::app::lifecycle::tie_child_to_app(&child);
        let stderr = child.stderr.take().expect("piped stderr");
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut sent = false;
            let mut tail: Vec<String> = Vec::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !sent && let Some(url) = find_url(&line) {
                    let _ = tx.send(url);
                    sent = true;
                }
                if !sent {
                    tail.push(line);
                    if tail.len() > 6 {
                        tail.remove(0);
                    }
                }
            }
            if !sent {
                let _ = tx.send(format!("ERR:{}", tail.join(" ")));
            }
        });
        match rx.recv_timeout(Duration::from_secs(45)) {
            Ok(url) if !url.starts_with("ERR:") => Ok(Self { child, url }),
            Ok(err) => {
                let _ = child.kill();
                Err(AppError::new(
                    "TUNNEL_FAILED",
                    format!(
                        "The Cloudflare tunnel did not open. {}",
                        err.trim_start_matches("ERR:")
                            .chars()
                            .take(300)
                            .collect::<String>()
                    ),
                ))
            }
            Err(_) => {
                let _ = child.kill();
                Err(AppError::new(
                    "TUNNEL_FAILED",
                    "The Cloudflare tunnel did not open within 45 seconds. Check the internet connection.",
                ))
            }
        }
    }

    pub fn close(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_link_from_the_log() {
        let line = "2026-11-20T10:00:00Z INF |  https://silly-words-here.trycloudflare.com                                 |";
        assert_eq!(
            find_url(line).as_deref(),
            Some("https://silly-words-here.trycloudflare.com")
        );
        assert_eq!(
            find_url("INF Requesting new quick Tunnel on trycloudflare.com..."),
            None
        );
        assert_eq!(
            find_url("see https://www.cloudflare.com/website-terms/"),
            None
        );
        assert_eq!(find_url("https://evil.com/x.trycloudflare.com"), None);
    }
}
