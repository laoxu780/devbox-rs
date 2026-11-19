//! Hosts file environment manager.
//!
//! Profiles are stored in `DevBox/hosts_profiles.json`. Applying a profile
//! replaces only the DevBox-managed block inside the system hosts file so
//! manually added entries are preserved.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::store;

pub const BLOCK_START: &str = "# >>> DevBox hosts start >>>";
pub const BLOCK_END: &str = "# <<< DevBox hosts end <<<";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsEntry {
    pub ip: String,
    pub hosts: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsProfile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub entries: Vec<HostsEntry>,
    #[serde(default)]
    pub position: i64,
}

#[derive(Clone, Debug, Default)]
pub struct HostsProfileView {
    pub id: String,
    pub name: String,
    pub entries: Vec<HostsEntry>,
    pub position: i64,
    pub active: bool,
    pub applied: bool,
}

impl From<(&HostsProfile, bool, bool)> for HostsProfileView {
    fn from((profile, active, applied): (&HostsProfile, bool, bool)) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            entries: profile.entries.clone(),
            position: profile.position,
            active,
            applied,
        }
    }
}

#[derive(Clone)]
pub struct HostsService {
    path: PathBuf,
    state: Arc<std::sync::Mutex<HostsState>>,
}

struct HostsState {
    profiles: BTreeMap<String, HostsProfile>,
    active_id: String,
    applied: bool,
}

/// Parse `ip host1 host2` lines. Blank lines and `#` comments are kept so the
/// generated block stays readable.
pub fn parse_entries(input: &str) -> Vec<HostsEntry> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            if line.starts_with('#') {
                return Some(HostsEntry {
                    ip: String::new(),
                    hosts: line.to_string(),
                    enabled: true,
                });
            }
            let mut parts = line.split_whitespace();
            let ip = parts.next()?.to_string();
            let hosts: Vec<&str> = parts.collect();
            if hosts.is_empty() {
                return None;
            }
            Some(HostsEntry {
                ip,
                hosts: hosts.join(" "),
                enabled: true,
            })
        })
        .collect()
}

pub fn render_entries(entries: &[HostsEntry]) -> String {
    entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| {
            if entry.ip.is_empty() {
                entry.hosts.clone()
            } else {
                format!("{}\t{}", entry.ip, entry.hosts)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_block(entries: &[HostsEntry]) -> String {
    format!("{BLOCK_START}\n{}\n{BLOCK_END}", render_entries(entries))
}

/// Replace (or append) the managed block, preserving everything else.
pub fn apply_block(content: &str, block: &str) -> String {
    let mut result = String::new();
    let mut inside = false;
    for line in content.lines() {
        if line.trim() == BLOCK_START {
            inside = true;
            result.push_str(block);
            result.push('\n');
            continue;
        }
        if inside {
            if line.trim() == BLOCK_END {
                inside = false;
            }
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    if !result.contains(BLOCK_START) {
        if !result.trim().is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(block);
        result.push('\n');
    }
    result
}

#[cfg(target_os = "windows")]
fn system_hosts_path() -> PathBuf {
    PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
}

#[cfg(not(target_os = "windows"))]
fn system_hosts_path() -> PathBuf {
    PathBuf::from("/etc/hosts")
}

impl HostsService {
    pub fn new() -> Self {
        Self::with_path(store::config_path("hosts_profiles.json"))
    }

    pub fn with_path(path: PathBuf) -> Self {
        let mut profiles = BTreeMap::new();
        let mut active_id = String::new();
        let mut applied = false;
        if let Ok(Some(stored)) = store::read_json::<HostsProfilesFileWithActive>(&path) {
            active_id = stored.active_id;
            applied = stored.applied;
            for mut profile in stored.profiles {
                if profile.id.is_empty() {
                    continue;
                }
                if profile.position == 0 {
                    profile.position = profiles.len() as i64 + 1;
                }
                profiles.insert(profile.id.clone(), profile);
            }
        }
        Self {
            path,
            state: Arc::new(std::sync::Mutex::new(HostsState {
                profiles,
                active_id,
                applied,
            })),
        }
    }

    fn persist(state: &HostsState, path: &Path) -> Result<(), String> {
        let mut profiles: Vec<_> = state.profiles.values().cloned().collect();
        profiles.sort_by_key(|profile| (profile.position, profile.id.clone()));
        store::write_json(
            path,
            &HostsProfilesFileWithActive {
                profiles,
                active_id: state.active_id.clone(),
                applied: state.applied,
            },
        )
        .map_err(|error| format!("保存 hosts 环境失败：{error}"))
    }

    pub fn list(&self) -> Vec<HostsProfileView> {
        let state = self.state.lock().unwrap();
        let mut profiles: Vec<_> = state.profiles.values().collect();
        profiles.sort_by_key(|profile| (profile.position, profile.id.clone()));
        profiles
            .into_iter()
            .map(|profile| {
                HostsProfileView::from((
                    profile,
                    profile.id == state.active_id,
                    state.applied && profile.id == state.active_id,
                ))
            })
            .collect()
    }

    pub fn profile(&self, id: &str) -> Option<HostsProfile> {
        self.state.lock().unwrap().profiles.get(id).cloned()
    }

    pub fn save_profile(&self, mut profile: HostsProfile) -> Result<HostsProfileView, String> {
        profile.name = profile.name.trim().to_string();
        if profile.name.is_empty() {
            return Err("环境名称不能为空".into());
        }
        let mut state = self.state.lock().unwrap();
        if profile.id.is_empty() {
            profile.id = crate::util::new_id();
            profile.position = state.profiles.values().map(|item| item.position).max().unwrap_or(0) + 1;
        }
        state.profiles.insert(profile.id.clone(), profile.clone());
        Self::persist(&state, &self.path)?;
        let active = profile.id == state.active_id;
        let applied = active && state.applied;
        Ok(HostsProfileView::from((&profile, active, applied)))
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        state.profiles.remove(id).ok_or_else(|| "环境不存在".to_string())?;
        if state.active_id == id {
            state.active_id.clear();
            state.applied = false;
        }
        Self::persist(&state, &self.path)
    }

    pub fn active_id(&self) -> String {
        self.state.lock().unwrap().active_id.clone()
    }

    pub fn system_content(&self) -> String {
        fs::read_to_string(system_hosts_path()).unwrap_or_else(|_| "无法读取系统 hosts 文件".into())
    }

    /// Generate the new hosts file contents for a profile without writing it.
    pub fn preview(&self, id: &str) -> Result<String, String> {
        let profile = self.profile(id).ok_or_else(|| "环境不存在".to_string())?;
        let current = fs::read_to_string(system_hosts_path()).unwrap_or_default();
        Ok(apply_block(&current, &render_block(&profile.entries)))
    }

    pub fn apply(&self, id: &str) -> Result<String, String> {
        let content = self.preview(id)?;
        write_system_hosts(&content)?;
        {
            let mut state = self.state.lock().unwrap();
            state.active_id = id.to_string();
            state.applied = true;
            Self::persist(&state, &self.path)?;
        }
        Ok("hosts 环境已切换".into())
    }

    pub fn clear(&self) -> Result<String, String> {
        let current = fs::read_to_string(system_hosts_path()).unwrap_or_default();
        let cleaned = remove_block(&current);
        write_system_hosts(&cleaned)?;
        {
            let mut state = self.state.lock().unwrap();
            state.active_id.clear();
            state.applied = false;
            Self::persist(&state, &self.path)?;
        }
        Ok("已移除 DevBox 管理的 hosts 配置".into())
    }
}

#[derive(Serialize, Deserialize)]
struct HostsProfilesFileWithActive {
    #[serde(default)]
    profiles: Vec<HostsProfile>,
    #[serde(default)]
    active_id: String,
    #[serde(default)]
    applied: bool,
}

pub fn remove_block(content: &str) -> String {
    let mut result = String::new();
    let mut inside = false;
    for line in content.lines() {
        if line.trim() == BLOCK_START {
            inside = true;
            continue;
        }
        if inside {
            if line.trim() == BLOCK_END {
                inside = false;
            }
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

fn write_system_hosts(content: &str) -> Result<(), String> {
    let staging = store::config_path("hosts_staging.txt");
    fs::write(&staging, content).map_err(|error| format!("写入临时文件失败：{error}"))?;

    #[cfg(target_os = "macos")]
    {
        // Ask macOS for administrator rights via a native authorisation dialog.
        let script = format!(
            "do shell script \"cp {} {}\" with administrator privileges",
            apple_quote(&staging.to_string_lossy()),
            apple_quote(&system_hosts_path().to_string_lossy())
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| format!("请求管理员权限失败：{error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        fs::write(system_hosts_path(), content)
            .map_err(|error| format!("写入系统 hosts 失败（请以管理员身份运行）：{error}"))
    }
}

#[cfg(target_os = "macos")]
fn apple_quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_renders_entries() {
        let entries = parse_entries("# comment\n127.0.0.1 localhost\n::1 localhost\n");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[1].ip, "127.0.0.1");
        let block = render_block(&entries);
        assert!(block.contains(BLOCK_START));
        assert!(block.contains("127.0.0.1\tlocalhost"));
    }

    #[test]
    fn apply_block_preserves_manual_entries() {
        let original = "127.0.0.1 localhost\n192.168.1.1 manual.example\n";
        let updated = apply_block(original, &render_block(&parse_entries("10.0.0.1 dev.local")));
        assert!(updated.contains("192.168.1.1 manual.example"));
        assert!(updated.contains("10.0.0.1\tdev.local"));
        let again = apply_block(&updated, &render_block(&parse_entries("10.0.0.2 dev.local")));
        assert_eq!(again.matches(BLOCK_START).count(), 1);
        assert!(!again.contains("10.0.0.1"));
        assert!(remove_block(&again).contains("192.168.1.1 manual.example"));
    }

    #[test]
    fn persists_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let service = HostsService::with_path(path.clone());
        service
            .save_profile(HostsProfile {
                name: "dev".into(),
                entries: parse_entries("10.0.0.1 dev.local"),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(service.list().len(), 1);
        let reloaded = HostsService::with_path(path);
        assert_eq!(reloaded.list()[0].name, "dev");
    }
}
