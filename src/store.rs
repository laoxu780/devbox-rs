//! Persistence helpers. Everything lives under the same `DevBox` config
//! directory the Go version used, so existing rules keep working.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn config_dir() -> PathBuf {
    // Tests (and power users) can redirect all persisted state with this
    // variable instead of touching the real DevBox config directory.
    if let Ok(dir) = std::env::var("DEVBOX_CONFIG_DIR")
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DevBox")
}

pub fn config_path(name: &str) -> PathBuf {
    config_dir().join(name)
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read(path) {
        Ok(data) => {
            let value = serde_json::from_slice(&data)
                .with_context(|| format!("解析 {} 失败", path.display()))?;
            Ok(Some(value))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("读取 {} 失败", path.display())),
    }
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建目录 {} 失败", parent.display()))?;
    }
    let data = serde_json::to_vec_pretty(value).context("编码 JSON 失败")?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("写入 {} 失败", tmp.display()))?;
        file.write_all(&data)?;
        file.sync_all().ok();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path).with_context(|| format!("替换 {} 失败", path.display()))?;
    Ok(())
}

/// UI preferences that used to live in the webview's localStorage.
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub theme: String,
    pub hidden_tools: Vec<String>,
    pub window_width: f32,
    pub window_height: f32,
    /// Last selected tool, restored when the main window is reopened.
    pub last_tool: String,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            hidden_tools: Vec::new(),
            window_width: 1240.0,
            window_height: 820.0,
            last_tool: "dashboard".into(),
        }
    }
}

impl UiSettings {
    pub fn load() -> Self {
        read_json(&config_path("ui.json"))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = write_json(&config_path("ui.json"), self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.json");
        let settings = UiSettings {
            theme: "dark".into(),
            hidden_tools: vec!["json".into()],
            ..Default::default()
        };
        write_json(&path, &settings).unwrap();
        let loaded: UiSettings = read_json(&path).unwrap().unwrap();
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.hidden_tools, vec!["json".to_string()]);
    }

    #[test]
    fn legacy_settings_default_last_tool() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.json");
        fs::write(&path, r#"{"theme":"dark"}"#).unwrap();
        let settings: UiSettings = read_json(&path).unwrap().unwrap();
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.last_tool, "dashboard");
    }
}
