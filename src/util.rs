//! Small helpers shared by the services and the UI.

use std::time::Duration;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use uuid::Uuid;

/// Characters escaped for the `X-DevBox-Env` / `X-DevBox-Upstream` hint
/// headers, mirroring Go's `url.QueryEscape` closely enough for ASCII hosts.
const HEADER_ESCAPE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b':')
    .remove(b'/');

pub fn new_id() -> String {
    Uuid::new_v4().simple().to_string()
}

pub fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn now_clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

pub fn escape_header(value: &str) -> String {
    utf8_percent_encode(value, HEADER_ESCAPE).to_string()
}

pub fn truncate_text(value: &str, max: usize) -> String {
    let mut out: String = value.chars().take(max).collect();
    if value.chars().count() > max {
        out.push('…');
    }
    out
}

pub fn format_duration(duration: Duration) -> String {
    if duration.as_millis() < 1 {
        format!("{}µs", duration.as_micros())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

/// Human readable byte count used by the key list.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Returns a display address, collapsing wildcard hosts to loopback.
pub fn display_address(host: &str, port: u16) -> String {
    let display = match host {
        "" | "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        other => other,
    };
    if display.contains(':') && !display.starts_with('[') {
        format!("http://[{display}]:{port}")
    } else {
        format!("http://{display}:{port}")
    }
}

/// Open a native file picker on Windows (PowerShell + WinForms). Returns an
/// empty string when the user cancels.
#[cfg(target_os = "windows")]
pub fn pick_file_path(title: &str, extensions: &[&str]) -> Result<String, String> {
    use std::process::Command;
    let pattern = extensions
        .iter()
        .map(|extension| format!("*.{extension}"))
        .collect::<Vec<_>>()
        .join(";");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = '{title}'; $d.Filter = '{pattern}|{pattern}|All files|*.*'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $d.FileName }}"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", &script])
        .output()
        .map_err(|error| format!("打开文件选择器失败：{error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_on_char_boundaries() {
        assert_eq!(truncate_text("你好世界", 2), "你好…");
        assert_eq!(truncate_text("abc", 10), "abc");
    }

    #[test]
    fn formats_addresses() {
        assert_eq!(display_address("0.0.0.0", 8787), "http://127.0.0.1:8787");
        assert_eq!(display_address("127.0.0.1", 80), "http://127.0.0.1:80");
    }

    #[test]
    fn humanizes_bytes() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
    }
}
