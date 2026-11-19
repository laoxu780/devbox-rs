//! SSH credentials, keys and port forwarding, ported from `ssh_service.go`.
//!
//! Supported forward modes:
//! * `local` / empty: local TCP listener -> `direct-tcpip` channel
//! * `socks`: local SOCKS5 server whose traffic exits through SSH
//! * `http`: local HTTP/CONNECT proxy with the same SSH exit

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex as AsyncMutex, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::store;
use crate::util::{display_address, new_id, now_rfc3339};

// ---------------------------------------------------------------------------
// Persisted types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshProfile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: i64,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub auth_type: String,
    #[serde(default)]
    pub private_key_path: String,
    #[serde(default)]
    pub strict_host_key_checking: bool,
    /// Password or key passphrase. Never rendered in the profile list.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub secret: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshProfileView {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub private_key_path: String,
    pub strict_host_key_checking: bool,
    pub has_secret: bool,
}

impl From<&SshProfile> for SshProfileView {
    fn from(profile: &SshProfile) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            host: profile.host.clone(),
            port: profile.port.max(0) as u16,
            username: profile.username.clone(),
            auth_type: profile.auth_type.clone(),
            private_key_path: profile.private_key_path.clone(),
            strict_host_key_checking: profile.strict_host_key_checking,
            has_secret: !profile.secret.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshForwardRule {
    #[serde(default)]
    pub id: String,
    // The Go version wrote `profileID`; keep accepting both spellings.
    #[serde(default, rename = "profileID", alias = "profileId")]
    pub profile_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub local_host: String,
    #[serde(default)]
    pub local_port: i64,
    #[serde(default)]
    pub remote_host: String,
    #[serde(default)]
    pub remote_port: i64,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub system_proxy: bool,
    #[serde(default)]
    pub position: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshForwardStatus {
    pub id: String,
    #[serde(default, rename = "profileID", alias = "profileId")]
    pub profile_id: String,
    pub name: String,
    pub local_address: String,
    pub remote_address: String,
    pub status: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub started_at: String,
    pub connections: u64,
    pub reconnects: u64,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    #[serde(default)]
    pub mode: String,
    pub system_proxy: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub name: String,
    pub path: String,
    pub r#type: String,
    pub bits: u32,
    pub fingerprint: String,
    pub has_public: bool,
    pub encrypted: bool,
    pub size: u64,
    pub mod_time: String,
    pub source: String,
    pub in_use: bool,
}

#[derive(Serialize, Deserialize)]
struct ProfilesFile {
    #[serde(default)]
    profiles: Vec<SshProfile>,
}

#[derive(Serialize, Deserialize)]
struct ForwardsFile {
    #[serde(default)]
    forwards: Vec<SshForwardRule>,
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ForwardRuntimeState {
    status: String,
    error: String,
    started_at: String,
    connections: u64,
    reconnects: u64,
    bound_port: u16,
    /// Per-connection tasks are tracked so disabling a rule also tears down
    /// sockets and SSH channels that are already open.
    connection_tasks: Vec<JoinHandle<()>>,
}

struct ForwardRuntime {
    shutdown: Option<oneshot::Sender<()>>,
    state: Arc<Mutex<ForwardRuntimeState>>,
    task: Option<JoinHandle<()>>,
}

struct SshState {
    profiles: BTreeMap<String, SshProfile>,
    rules: BTreeMap<String, SshForwardRule>,
    forwards: HashMap<String, ForwardRuntime>,
    /// Startup failures that never produced a runtime, so the UI can explain
    /// why a restored rule is not running.
    failed: HashMap<String, String>,
    profiles_path: PathBuf,
    forwards_path: PathBuf,
    system_proxy_owner: Option<String>,
}

#[derive(Clone)]
pub struct SshService {
    inner: Arc<SshInner>,
}

struct SshInner {
    state: Mutex<SshState>,
    handle: tokio::runtime::Handle,
}

impl SshService {
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self::new_with_paths(
            handle,
            store::config_path("ssh_profiles.json"),
            store::config_path("ssh_forwards.json"),
        )
    }

    /// Testable constructor that keeps the real config files untouched.
    pub fn new_with_paths(
        handle: tokio::runtime::Handle,
        profiles_path: PathBuf,
        forwards_path: PathBuf,
    ) -> Self {
        let mut profiles = BTreeMap::new();
        if let Ok(Some(stored)) = store::read_json::<ProfilesFile>(&profiles_path) {
            for mut profile in stored.profiles {
                if profile.id.is_empty() {
                    continue;
                }
                // The Go version persisted `private_key`; normalise it so
                // existing profiles keep authenticating with their key.
                if profile.auth_type == "private_key" {
                    profile.auth_type = "key".into();
                }
                profiles.insert(profile.id.clone(), profile);
            }
        }
        let mut rules = BTreeMap::new();
        let mut position = 1;
        if let Ok(Some(stored)) = store::read_json::<ForwardsFile>(&forwards_path) {
            for mut rule in stored.forwards {
                if rule.id.is_empty() {
                    continue;
                }
                if rule.position == 0 {
                    rule.position = position;
                }
                position = position.max(rule.position + 1);
                rules.insert(rule.id.clone(), rule);
            }
        }
        let service = Self {
            inner: Arc::new(SshInner {
                state: Mutex::new(SshState {
                    profiles,
                    rules,
                    forwards: HashMap::new(),
                    failed: HashMap::new(),
                    profiles_path,
                    forwards_path,
                    system_proxy_owner: None,
                }),
                handle,
            }),
        };
        service.restore_forwards();
        service
    }

    fn restore_forwards(&self) {
        let rules: Vec<SshForwardRule> = self
            .inner
            .state
            .lock()
            .unwrap()
            .rules
            .values()
            .filter(|rule| !rule.disabled)
            .cloned()
            .collect();
        for rule in rules {
            if let Err(error) = self.start_forward_inner(rule.clone()) {
                self.inner
                    .state
                    .lock()
                    .unwrap()
                    .failed
                    .insert(rule.id.clone(), error);
            }
        }
    }

    fn persist_profiles(state: &SshState) -> Result<(), String> {
        let profiles: Vec<_> = state.profiles.values().cloned().collect();
        store::write_json(&state.profiles_path, &ProfilesFile { profiles })
            .map_err(|error| format!("保存 SSH 凭证失败：{error}"))
    }

    fn persist_forwards(state: &SshState) -> Result<(), String> {
        let mut forwards: Vec<_> = state.rules.values().cloned().collect();
        forwards.sort_by_key(|rule| (rule.position, rule.id.clone()));
        store::write_json(&state.forwards_path, &ForwardsFile { forwards })
            .map_err(|error| format!("保存 SSH 转发失败：{error}"))
    }

    // -- profiles ----------------------------------------------------------

    pub fn list_profiles(&self) -> Vec<SshProfileView> {
        self.inner
            .state
            .lock()
            .unwrap()
            .profiles
            .values()
            .map(SshProfileView::from)
            .collect()
    }

    pub fn profile(&self, id: &str) -> Option<SshProfile> {
        self.inner.state.lock().unwrap().profiles.get(id).cloned()
    }

    pub fn save_profile(&self, mut input: SshProfile) -> Result<SshProfileView, String> {
        input.name = input.name.trim().to_string();
        input.host = input.host.trim().to_string();
        input.username = input.username.trim().to_string();
        input.private_key_path = input.private_key_path.trim().to_string();
        if input.host.is_empty() {
            return Err("主机地址不能为空".into());
        }
        if input.username.is_empty() {
            return Err("用户名不能为空".into());
        }
        if input.port <= 0 || input.port > 65535 {
            input.port = 22;
        }
        if input.auth_type == "private_key" {
            input.auth_type = "key".into();
        }
        if input.auth_type != "key" {
            input.auth_type = "password".into();
        }
        if input.auth_type == "key" && input.private_key_path.is_empty() {
            return Err("请选择私钥文件".into());
        }
        let mut state = self.inner.state.lock().unwrap();
        if input.id.is_empty() {
            input.id = new_id();
        } else if let Some(existing) = state.profiles.get(&input.id) {
            if input.secret.is_empty() {
                input.secret = existing.secret.clone();
            }
        }
        if input.name.is_empty() {
            input.name = format!("{}@{}", input.username, input.host);
        }
        let view = SshProfileView::from(&input);
        state.profiles.insert(input.id.clone(), input);
        Self::persist_profiles(&state)?;
        Ok(view)
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), String> {
        let affected: Vec<String> = self
            .inner
            .state
            .lock()
            .unwrap()
            .rules
            .values()
            .filter(|rule| rule.profile_id == id)
            .map(|rule| rule.id.clone())
            .collect();
        for forward_id in affected {
            self.stop_forward(&forward_id);
        }
        let mut state = self.inner.state.lock().unwrap();
        state
            .profiles
            .remove(id)
            .ok_or_else(|| "SSH 凭证不存在".to_string())?;
        state.rules.retain(|_, rule| rule.profile_id != id);
        Self::persist_profiles(&state)?;
        Self::persist_forwards(&state)?;
        Ok(())
    }

    /// Connect + authenticate + disconnect. Used by the "测试连接" button.
    pub fn test_profile(&self, id: &str) -> Result<(), String> {
        let profile = self.profile(id).ok_or_else(|| "SSH 凭证不存在".to_string())?;
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner.handle.spawn(async move {
            let result = connect_session(&profile).await.map(|_| ());
            let _ = tx.send(result.map_err(|error| error.to_string()));
        });
        rx.recv_timeout(Duration::from_secs(20))
            .map_err(|_| "连接超时".to_string())?
    }

    // -- forwards ----------------------------------------------------------

    pub fn list_forwards(&self) -> Vec<SshForwardStatus> {
        let state = self.inner.state.lock().unwrap();
        let mut rules: Vec<_> = state.rules.values().cloned().collect();
        rules.sort_by_key(|rule| (rule.position, rule.id.clone()));
        rules
            .into_iter()
            .map(|rule| {
                let mut status = SshForwardStatus {
                    id: rule.id.clone(),
                    profile_id: rule.profile_id.clone(),
                    name: rule.name.clone(),
                    local_address: display_address(&rule.local_host, rule.local_port.max(0) as u16)
                        .trim_start_matches("http://")
                        .to_string(),
                    remote_address: if rule.mode == "socks" || rule.mode == "http" {
                        "SSH 出口".into()
                    } else {
                        format!("{}:{}", rule.remote_host, rule.remote_port)
                    },
                    status: "stopped".into(),
                    local_host: rule.local_host.clone(),
                    local_port: rule.local_port.max(0) as u16,
                    remote_host: rule.remote_host.clone(),
                    remote_port: rule.remote_port.max(0) as u16,
                    mode: rule.mode.clone(),
                    system_proxy: rule.system_proxy,
                    ..Default::default()
                };
                if let Some(runtime) = state.forwards.get(&rule.id) {
                    let runtime_state = runtime.state.lock().unwrap();
                    status.status = runtime_state.status.clone();
                    status.error = runtime_state.error.clone();
                    status.started_at = runtime_state.started_at.clone();
                    status.connections = runtime_state.connections;
                    status.reconnects = runtime_state.reconnects;
                    if runtime_state.bound_port > 0 {
                        status.local_port = runtime_state.bound_port;
                        status.local_address = display_address(
                            &rule.local_host,
                            runtime_state.bound_port,
                        )
                        .trim_start_matches("http://")
                        .to_string();
                    }
                } else if let Some(message) = state.failed.get(&rule.id) {
                    status.status = "error".into();
                    status.error = message.clone();
                }
                status
            })
            .collect()
    }

    pub fn start_forward(&self, mut request: SshForwardRule) -> Result<SshForwardStatus, String> {
        validate_forward(&mut request)?;
        let mut state = self.inner.state.lock().unwrap();
        if request.id.is_empty() {
            request.id = new_id();
            request.position = state
                .rules
                .values()
                .map(|rule| rule.position)
                .max()
                .unwrap_or(0)
                + 1;
        } else if let Some(old) = state.rules.get(&request.id) {
            request.position = old.position;
        }
        request.disabled = false;
        if !state.profiles.contains_key(&request.profile_id) {
            return Err("请选择有效的 SSH 凭证".into());
        }
        state.rules.insert(request.id.clone(), request.clone());
        Self::persist_forwards(&state)?;
        drop(state);
        self.stop_forward(&request.id);
        self.start_forward_inner(request.clone())?;
        self.list_forwards()
            .into_iter()
            .find(|status| status.id == request.id)
            .ok_or_else(|| "SSH 转发不存在".to_string())
    }

    pub fn stop_forward(&self, id: &str) {
        let runtime = self.inner.state.lock().unwrap().forwards.remove(id);
        if let Some(mut runtime) = runtime {
            if let Some(shutdown) = runtime.shutdown.take() {
                let _ = shutdown.send(());
            }
            if let Some(task) = runtime.task.take() {
                task.abort();
            }
            let connections = std::mem::take(
                &mut runtime.state.lock().unwrap().connection_tasks,
            );
            for task in connections {
                task.abort();
            }
        }
    }

    pub fn toggle_forward(&self, id: &str, enabled: bool) -> Result<String, String> {
        let rule = {
            let mut state = self.inner.state.lock().unwrap();
            let Some(rule) = state.rules.get_mut(id) else {
                return Err("SSH 转发不存在".into());
            };
            rule.disabled = !enabled;
            let rule = rule.clone();
            Self::persist_forwards(&state)?;
            rule
        };
        if !enabled {
            self.stop_forward(id);
            self.maybe_clear_system_proxy(id);
            self.inner.state.lock().unwrap().failed.remove(id);
            return Ok("SSH 转发已关闭".into());
        }
        self.start_forward_inner(rule)?;
        Ok("SSH 转发已开启".into())
    }

    pub fn delete_forward(&self, id: &str) -> Result<(), String> {
        self.stop_forward(id);
        self.maybe_clear_system_proxy(id);
        let mut state = self.inner.state.lock().unwrap();
        state
            .rules
            .remove(id)
            .ok_or_else(|| "SSH 转发不存在".to_string())?;
        state.failed.remove(id);
        Self::persist_forwards(&state)
    }

    pub fn close(&self) {
        let ids: Vec<String> = self
            .inner
            .state
            .lock()
            .unwrap()
            .forwards
            .keys()
            .cloned()
            .collect();
        for id in ids {
            self.stop_forward(&id);
        }
        self.clear_system_proxy();
    }

    fn start_forward_inner(&self, rule: SshForwardRule) -> Result<(), String> {
        let id = rule.id.clone();
        let result = self.start_forward_inner_impl(rule);
        if let Err(error) = &result {
            self.inner
                .state
                .lock()
                .unwrap()
                .failed
                .insert(id, error.clone());
        }
        result
    }

    fn start_forward_inner_impl(&self, rule: SshForwardRule) -> Result<(), String> {
        let profile = {
            let state = self.inner.state.lock().unwrap();
            state
                .profiles
                .get(&rule.profile_id)
                .cloned()
                .ok_or_else(|| "SSH 凭证不存在".to_string())?
        };

        let bind_host = if rule.local_host.trim().is_empty() {
            "127.0.0.1".to_string()
        } else {
            rule.local_host.trim().to_string()
        };
        let requested = if (0..=65535).contains(&rule.local_port) {
            rule.local_port as u16
        } else {
            return Err("本地端口无效".into());
        };
        let std_listener = std::net::TcpListener::bind((bind_host.as_str(), requested))
            .map_err(|error| format!("监听 {} 失败: {error}", display_address(&bind_host, requested)))?;
        std_listener
            .set_nonblocking(true)
            .map_err(|error| format!("配置监听失败: {error}"))?;
        let bound_port = std_listener
            .local_addr()
            .map_err(|error| format!("读取监听端口失败: {error}"))?
            .port();
        // Restore happens on the UI thread, outside any entered Tokio context.
        let listener = {
            let _guard = self.inner.handle.enter();
            TcpListener::from_std(std_listener)
                .map_err(|error| format!("启动监听失败: {error}"))?
        };

        let runtime_state = Arc::new(Mutex::new(ForwardRuntimeState {
            status: "connecting".into(),
            started_at: now_rfc3339(),
            bound_port,
            ..Default::default()
        }));
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        {
            let mut state = self.inner.state.lock().unwrap();
            if let Some(stored) = state.rules.get_mut(&rule.id) {
                if stored.disabled {
                    return Err("转发已关闭".into());
                }
                if stored.local_port == 0 {
                    stored.local_port = bound_port as i64;
                    let _ = Self::persist_forwards(&state);
                }
            }
            state.failed.remove(&rule.id);
            state.forwards.insert(
                rule.id.clone(),
                ForwardRuntime {
                    shutdown: Some(shutdown_tx),
                    state: runtime_state.clone(),
                    task: None,
                },
            );
        }

        let profile_for_task = profile.clone();
        let rule_for_task = rule.clone();
        let task_state = runtime_state.clone();
        let handle = self.inner.handle.clone();
        let task = self.inner.handle.spawn(async move {
            forward_loop(
                listener,
                profile_for_task,
                rule_for_task,
                task_state,
                shutdown_rx,
                handle,
            )
            .await;
        });
        if let Some(runtime) = self
            .inner
            .state
            .lock()
            .unwrap()
            .forwards
            .get_mut(&rule.id)
        {
            runtime.task = Some(task);
        }
        Ok(())
    }

    // -- system proxy ------------------------------------------------------

    pub fn maybe_apply_system_proxy(&self, id: &str) -> Result<(), String> {
        let (rule, port) = {
            let state = self.inner.state.lock().unwrap();
            let rule = state
                .rules
                .get(id)
                .cloned()
                .ok_or_else(|| "SSH 转发不存在".to_string())?;
            let port = state
                .forwards
                .get(id)
                .map(|runtime| runtime.state.lock().unwrap().bound_port)
                .unwrap_or(rule.local_port.max(0) as u16);
            (rule, port)
        };
        if !rule.system_proxy {
            return Ok(());
        }
        apply_system_proxy(port, &rule.mode)?;
        self.inner.state.lock().unwrap().system_proxy_owner = Some(id.to_string());
        Ok(())
    }

    fn maybe_clear_system_proxy(&self, id: &str) {
        let owned = {
            let mut state = self.inner.state.lock().unwrap();
            if state.system_proxy_owner.as_deref() == Some(id) {
                state.system_proxy_owner = None;
                true
            } else {
                false
            }
        };
        if owned {
            clear_system_proxy();
        }
    }

    fn clear_system_proxy(&self) {
        let owned = {
            let mut state = self.inner.state.lock().unwrap();
            let owned = state.system_proxy_owner.is_some();
            state.system_proxy_owner = None;
            owned
        };
        if owned {
            clear_system_proxy();
        }
    }

    // -- keys --------------------------------------------------------------

    pub fn list_keys(&self) -> Vec<SshKeyInfo> {
        let in_use: HashSet<String> = self
            .inner
            .state
            .lock()
            .unwrap()
            .profiles
            .values()
            .map(|profile| profile.private_key_path.clone())
            .collect();
        let mut keys = Vec::new();
        for (dir, source) in [
            (home_ssh_dir(), "ssh"),
            (store::config_path("keys"), "app"),
        ] {
            collect_keys(&dir, source, &in_use, &mut keys);
        }
        keys.sort_by(|a, b| a.name.cmp(&b.name));
        keys
    }

    pub fn read_public_key(&self, path: &str) -> Result<String, String> {
        let public_path = PathBuf::from(format!("{path}.pub"));
        if let Ok(text) = std::fs::read_to_string(&public_path) {
            return Ok(text.trim().to_string());
        }
        let output = Command::new("ssh-keygen")
            .args(["-y", "-f", path])
            .output()
            .map_err(|error| format!("读取公钥失败：{error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn generate_key(
        &self,
        key_type: &str,
        name: &str,
        passphrase: &str,
    ) -> Result<SshKeyInfo, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("请输入密钥名称".into());
        }
        if name.contains('/') || name.contains('\\') {
            return Err("密钥名称不能包含路径分隔符".into());
        }
        let dir = store::config_path("keys");
        std::fs::create_dir_all(&dir).map_err(|error| format!("创建密钥目录失败：{error}"))?;
        let path = dir.join(name);
        if path.exists() {
            return Err("同名密钥已存在".into());
        }
        let algorithm = if key_type == "rsa" { "rsa" } else { "ed25519" };
        let mut command = Command::new("ssh-keygen");
        command.args(["-t", algorithm, "-f"]).arg(&path);
        if algorithm == "rsa" {
            command.args(["-b", "4096"]);
        }
        command.args(["-N", passphrase, "-C", "devbox"]);
        let output = command
            .output()
            .map_err(|error| format!("生成密钥失败：{error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let in_use = HashSet::new();
        let mut keys = Vec::new();
        collect_keys(&dir, "app", &in_use, &mut keys);
        keys.into_iter()
            .find(|key| key.path == path.to_string_lossy())
            .ok_or_else(|| "密钥生成后未找到".to_string())
    }

    pub fn delete_key(&self, path: &str) -> Result<(), String> {
        let path = PathBuf::from(path);
        let allowed = [
            home_ssh_dir(),
            store::config_path("keys"),
        ];
        if !allowed.iter().any(|dir| path.starts_with(dir)) {
            return Err("不允许删除该路径的密钥".into());
        }
        if !path.exists() {
            return Err("密钥文件不存在".into());
        }
        std::fs::remove_file(&path).map_err(|error| format!("删除密钥失败：{error}"))?;
        let _ = std::fs::remove_file(format!("{}.pub", path.display()));
        Ok(())
    }
}

fn validate_forward(rule: &mut SshForwardRule) -> Result<(), String> {
    rule.name = rule.name.trim().to_string();
    rule.local_host = rule.local_host.trim().to_string();
    rule.remote_host = rule.remote_host.trim().to_string();
    if rule.local_host.is_empty() {
        rule.local_host = "127.0.0.1".into();
    }
    if !(0..=65535).contains(&rule.local_port) {
        return Err("本地端口无效".into());
    }
    let mode = rule.mode.trim().to_ascii_lowercase();
    rule.mode = match mode.as_str() {
        "" | "local" => String::new(),
        "socks" => "socks".into(),
        "http" => "http".into(),
        _ => return Err("不支持的转发模式".into()),
    };
    if rule.mode.is_empty() {
        if rule.remote_host.is_empty() {
            return Err("远端主机不能为空".into());
        }
        if rule.remote_port <= 0 || rule.remote_port > 65535 {
            return Err("远端端口无效".into());
        }
    }
    if rule.name.is_empty() {
        rule.name = if rule.mode.is_empty() {
            format!(
                "{}:{} → {}:{}",
                rule.local_host, rule.local_port, rule.remote_host, rule.remote_port
            )
        } else {
            format!("SSH {} 代理 :{}", rule.mode.to_uppercase(), rule.local_port)
        };
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SSH connection
// ---------------------------------------------------------------------------

struct ClientHandler;

impl russh::client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // Dev tool: host keys are accepted. `strict_host_key_checking` is
        // surfaced in the UI but does not block connections yet.
        Ok(true)
    }
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().into_owned();
    }
    path.to_string()
}

async fn connect_session(profile: &SshProfile) -> Result<russh::client::Handle<ClientHandler>> {
    let mut config = russh::client::Config::default();
    config.inactivity_timeout = None;
    config.keepalive_interval = Some(Duration::from_secs(30));
    config.keepalive_max = 3;
    config.nodelay = true;
    let config = Arc::new(config);
    let mut session = russh::client::connect(
        config,
        (profile.host.as_str(), profile.port.max(1) as u16),
        ClientHandler,
    )
    .await
    .map_err(|error| anyhow!("连接 {} 失败：{error}", profile.host))?;

    let result = if profile.auth_type == "key" {
        let key = russh::keys::load_secret_key(
            &expand_home(&profile.private_key_path),
            if profile.secret.is_empty() {
                None
            } else {
                Some(profile.secret.as_str())
            },
        )
        .map_err(|error| anyhow!("读取私钥失败：{error}"))?;
        // RSA keys need the hash the server accepts (SHA-1 is often disabled).
        let rsa_hash = session
            .best_supported_rsa_hash()
            .await
            .map_err(|error| anyhow!("协商 RSA 签名算法失败：{error}"))?
            .flatten();
        let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), rsa_hash);
        session
            .authenticate_publickey(&profile.username, key)
            .await
            .map_err(|error| anyhow!("认证失败：{error}"))?
    } else {
        session
            .authenticate_password(&profile.username, &profile.secret)
            .await
            .map_err(|error| anyhow!("认证失败：{error}"))?
    };
    if !matches!(result, russh::client::AuthResult::Success) {
        return Err(anyhow!("SSH 认证被拒绝"));
    }
    Ok(session)
}

async fn forward_loop(
    listener: TcpListener,
    profile: SshProfile,
    rule: SshForwardRule,
    state: Arc<Mutex<ForwardRuntimeState>>,
    mut shutdown: oneshot::Receiver<()>,
    handle: tokio::runtime::Handle,
) {
    let (fail_tx, mut fail_rx) = mpsc::channel::<String>(1);
    loop {
        {
            let mut state = state.lock().unwrap();
            state.status = if state.reconnects == 0 {
                "connecting".into()
            } else {
                "reconnecting".into()
            };
            state.error.clear();
        }
        let session = match connect_session(&profile).await {
            Ok(session) => session,
            Err(error) => {
                {
                    let mut state = state.lock().unwrap();
                    state.status = "error".into();
                    state.error = error.to_string();
                }
                tokio::select! {
                    _ = &mut shutdown => return,
                    _ = tokio::time::sleep(Duration::from_secs(3)) => continue,
                }
            }
        };
        let session = Arc::new(AsyncMutex::new(session));
        {
            let mut state = state.lock().unwrap();
            state.status = "running".into();
            state.error.clear();
            state.started_at = now_rfc3339();
        }
        if rule.system_proxy {
            if let Err(error) = apply_system_proxy(
                state.lock().unwrap().bound_port,
                &rule.mode,
            ) {
                state.lock().unwrap().error = error;
            }
        }

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    if rule.system_proxy {
                        clear_system_proxy();
                    }
                    return;
                }
                failure = fail_rx.recv() => {
                    if let Some(message) = failure {
                        let mut state = state.lock().unwrap();
                        state.status = "reconnecting".into();
                        state.error = message;
                        state.reconnects += 1;
                        drop(state);
                    }
                    break;
                }
                accepted = listener.accept() => {
                    let Ok((stream, _)) = accepted else { return };
                    {
                        let mut state = state.lock().unwrap();
                        state.connections += 1;
                    }
                    let session = session.clone();
                    let rule = rule.clone();
                    let fail = fail_tx.clone();
                    let state_for_conn = state.clone();
                    let task = handle.spawn(async move {
                        if let Err(error) = handle_connection(stream, session, &rule).await {
                            let _ = fail.send(error.to_string()).await;
                        }
                        let mut state = state_for_conn.lock().unwrap();
                        state.connections = state.connections.saturating_sub(1);
                    });
                    state.lock().unwrap().connection_tasks.push(task);
                }
            }
        }
        tokio::select! {
            _ = &mut shutdown => {
                if rule.system_proxy {
                    clear_system_proxy();
                }
                return;
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }
}

async fn open_channel(
    session: &Arc<AsyncMutex<russh::client::Handle<ClientHandler>>>,
    host: &str,
    port: u16,
) -> Result<russh::Channel<russh::client::Msg>> {
    session
        .lock()
        .await
        .channel_open_direct_tcpip(host, port as u32, "127.0.0.1", 0)
        .await
        .map_err(|error| anyhow!("打开 SSH 通道失败：{error}"))
}

async fn handle_connection(
    mut stream: TcpStream,
    session: Arc<AsyncMutex<russh::client::Handle<ClientHandler>>>,
    rule: &SshForwardRule,
) -> Result<()> {
    match rule.mode.as_str() {
        "socks" => handle_socks(stream, session).await,
        "http" => handle_http_proxy(stream, session).await,
        _ => {
            let channel = open_channel(&session, &rule.remote_host, rule.remote_port as u16).await?;
            let mut channel_stream = channel.into_stream();
            tokio::io::copy_bidirectional(&mut stream, &mut channel_stream).await?;
            Ok(())
        }
    }
}

async fn handle_socks(
    mut stream: TcpStream,
    session: Arc<AsyncMutex<russh::client::Handle<ClientHandler>>>,
) -> Result<()> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;
    if header[0] != 5 {
        return Ok(());
    }
    let mut methods = vec![0u8; header[1] as usize];
    stream.read_exact(&mut methods).await?;
    stream.write_all(&[5, 0]).await?;

    let mut request = [0u8; 4];
    stream.read_exact(&mut request).await?;
    if request[0] != 5 {
        return Ok(());
    }
    if request[1] != 1 {
        stream.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        return Ok(());
    }
    let host = match request[3] {
        1 => {
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
            std::net::Ipv4Addr::from(addr).to_string()
        }
        3 => {
            let mut length = [0u8; 1];
            stream.read_exact(&mut length).await?;
            let mut name = vec![0u8; length[0] as usize];
            stream.read_exact(&mut name).await?;
            String::from_utf8_lossy(&name).to_string()
        }
        4 => {
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
            std::net::Ipv6Addr::from(addr).to_string()
        }
        _ => {
            stream.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Ok(());
        }
    };
    let mut port = [0u8; 2];
    stream.read_exact(&mut port).await?;
    let port = u16::from_be_bytes(port);
    match open_channel(&session, &host, port).await {
        Ok(channel) => {
            stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            let mut channel_stream = channel.into_stream();
            tokio::io::copy_bidirectional(&mut stream, &mut channel_stream).await?;
        }
        Err(_) => {
            stream.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        }
    }
    Ok(())
}

async fn handle_http_proxy(
    mut stream: TcpStream,
    session: Arc<AsyncMutex<russh::client::Handle<ClientHandler>>>,
) -> Result<()> {
    let mut buffer = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") || buffer.len() > 64 * 1024 {
            break;
        }
    }
    let head_end = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .unwrap_or(buffer.len());
    let head = String::from_utf8_lossy(&buffer[..head_end]).to_string();
    let leftover = buffer[head_end..].to_vec();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");
    let version = parts.next().unwrap_or("HTTP/1.1");

    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = split_host_port(target).unwrap_or((target.to_string(), 443));
        let channel = open_channel(&session, &host, port).await?;
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        let mut channel_stream = channel.into_stream();
        if !leftover.is_empty() {
            channel_stream.write_all(&leftover).await?;
        }
        tokio::io::copy_bidirectional(&mut stream, &mut channel_stream).await?;
        return Ok(());
    }

    let parsed = if target.starts_with("http://") || target.starts_with("https://") {
        url::Url::parse(target).ok()
    } else {
        None
    };
    let (host, port, path) = match parsed {
        Some(url) => (
            url.host_str().unwrap_or_default().to_string(),
            url.port_or_known_default().unwrap_or(80),
            format!(
                "{}{}",
                url.path(),
                url.query().map(|query| format!("?{query}")).unwrap_or_default()
            ),
        ),
        None => {
            let host = lines
                .clone()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("host").then(|| value.trim().to_string())
                })
                .unwrap_or_default();
            let (host, port) = split_host_port(&host).unwrap_or((host, 80));
            (host, port, target.to_string())
        }
    };
    let channel = open_channel(&session, &host, port).await?;
    let mut channel_stream = channel.into_stream();
    let rewritten = rewrite_http_request_head(lines, method, &path, version);
    channel_stream.write_all(rewritten.as_bytes()).await?;
    if !leftover.is_empty() {
        channel_stream.write_all(&leftover).await?;
    }
    tokio::io::copy_bidirectional(&mut stream, &mut channel_stream).await?;
    Ok(())
}

/// Rebuild an HTTP request line for a proxied request. `lines` is the header
/// iterator *after* the request line has been consumed; splitting a head that
/// ends in `\r\n\r\n` yields trailing empty strings, which must be skipped or
/// the blank line would terminate the headers before `Connection: close`.
fn rewrite_http_request_head<'a>(
    lines: impl Iterator<Item = &'a str>,
    method: &str,
    path: &str,
    version: &str,
) -> String {
    let mut rewritten = format!("{method} {path} {version}\r\n");
    for line in lines {
        if line.is_empty() || line.to_ascii_lowercase().starts_with("connection:") {
            continue;
        }
        rewritten.push_str(line);
        rewritten.push_str("\r\n");
    }
    rewritten.push_str("Connection: close\r\n\r\n");
    rewritten
}

fn split_host_port(value: &str) -> Option<(String, u16)> {
    if let Some(rest) = value.strip_prefix('[') {
        let (host, rest) = rest.split_once(']')?;
        let port = rest.strip_prefix(':').and_then(|port| port.parse().ok())?;
        return Some((host.to_string(), port));
    }
    match value.rsplit_once(':') {
        Some((host, port)) => port
            .parse()
            .ok()
            .map(|port| (host.to_string(), port)),
        None => None,
    }
}

// ---------------------------------------------------------------------------
// Key discovery
// ---------------------------------------------------------------------------

fn home_ssh_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".ssh")
}

fn key_bits(public: &russh::keys::PublicKey) -> u32 {
    use russh::keys::EcdsaCurve;
    use russh::keys::ssh_key::public::KeyData;
    match public.key_data() {
        KeyData::Ed25519(_) => 256,
        KeyData::Ecdsa(key) => match key.curve() {
            EcdsaCurve::NistP256 => 256,
            EcdsaCurve::NistP384 => 384,
            EcdsaCurve::NistP521 => 521,
        },
        KeyData::Rsa(key) => key.key_size(),
        _ => 0,
    }
}

fn collect_keys(dir: &Path, source: &str, in_use: &HashSet<String>, out: &mut Vec<SshKeyInfo>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with('.') || name.ends_with(".pub") {
            continue;
        }
        if matches!(
            name,
            "known_hosts" | "known_hosts.old" | "config" | "authorized_keys" | "agent.sock"
        ) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let key_type = if text.contains("OPENSSH PRIVATE KEY") {
            "OPENSSH"
        } else if text.contains("RSA PRIVATE KEY") {
            "RSA"
        } else if text.contains("EC PRIVATE KEY") {
            "ECDSA"
        } else if text.contains("PRIVATE KEY") {
            "PKCS#8"
        } else {
            continue;
        };
        let mut fingerprint = String::new();
        let mut bits = 0u32;
        let mut encrypted = false;
        let mut parsed_type = key_type.to_string();
        match russh::keys::load_secret_key(&path, None) {
            Ok(key) => {
                parsed_type = key.algorithm().to_string();
                let public = key.public_key();
                fingerprint = public.fingerprint(russh::keys::HashAlg::Sha256).to_string();
                bits = key_bits(public);
            }
            Err(_) => {
                encrypted = true;
                if let Ok(public) = std::fs::read_to_string(format!("{}.pub", path.display())) {
                    if let Ok(public) = russh::keys::PublicKey::from_openssh(public.trim()) {
                        fingerprint = public.fingerprint(russh::keys::HashAlg::Sha256).to_string();
                        bits = key_bits(&public);
                        parsed_type = public.algorithm().to_string();
                    }
                }
            }
        }
        out.push(SshKeyInfo {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            r#type: parsed_type,
            bits,
            fingerprint,
            has_public: PathBuf::from(format!("{}.pub", path.display())).exists(),
            encrypted,
            size: metadata.len(),
            mod_time: metadata
                .modified()
                .ok()
                .map(|time| {
                    chrono::DateTime::<chrono::Local>::from(time)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
                .unwrap_or_default(),
            source: source.to_string(),
            in_use: in_use.contains(&path.to_string_lossy().to_string()),
        });
    }
}

// ---------------------------------------------------------------------------
// macOS system proxy
// ---------------------------------------------------------------------------

fn apply_system_proxy(port: u16, mode: &str) -> Result<(), String> {
    if port == 0 {
        return Err("本地端口尚未就绪".into());
    }
    #[cfg(target_os = "macos")]
    {
        for service in network_services() {
            if mode == "socks" {
                run_networksetup(&["-setsocksfirewallproxy", &service, "127.0.0.1", &port.to_string()])?;
                run_networksetup(&["-setsocksfirewallproxystate", &service, "on"])?;
            } else {
                run_networksetup(&["-setwebproxy", &service, "127.0.0.1", &port.to_string()])?;
                run_networksetup(&["-setsecurewebproxy", &service, "127.0.0.1", &port.to_string()])?;
                run_networksetup(&["-setwebproxystate", &service, "on"])?;
                run_networksetup(&["-setsecurewebproxystate", &service, "on"])?;
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (port, mode);
    }
    Ok(())
}

fn clear_system_proxy() {
    #[cfg(target_os = "macos")]
    {
        for service in network_services() {
            let _ = run_networksetup(&["-setwebproxystate", &service, "off"]);
            let _ = run_networksetup(&["-setsecurewebproxystate", &service, "off"]);
            let _ = run_networksetup(&["-setsocksfirewallproxystate", &service, "off"]);
        }
    }
}

#[cfg(target_os = "macos")]
fn network_services() -> Vec<String> {
    let Ok(output) = Command::new("networksetup")
        .args(["-listallnetworkservices"])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('*'))
        .map(str::to_string)
        .collect()
}

#[cfg(target_os = "macos")]
fn run_networksetup(args: &[&str]) -> Result<(), String> {
    let output = Command::new("networksetup")
        .args(args)
        .output()
        .map_err(|error| format!("执行 networksetup 失败：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_legacy_go_config_fields() {
        let rule: SshForwardRule =
            serde_json::from_str(r#"{"id":"f","profileID":"p","localPort":8080}"#).unwrap();
        assert_eq!(rule.profile_id, "p");
        assert_eq!(rule.local_port, 8080);
        let rule: SshForwardRule = serde_json::from_str(r#"{"profileId":"p2"}"#).unwrap();
        assert_eq!(rule.profile_id, "p2");
        let profile: SshProfile =
            serde_json::from_str(r#"{"id":"p","authType":"private_key"}"#).unwrap();
        assert_eq!(profile.auth_type, "private_key");
    }

    #[test]
    fn validates_local_forward() {
        let mut rule = SshForwardRule {
            profile_id: "p".into(),
            local_port: 18080,
            remote_host: "db.internal".into(),
            remote_port: 5432,
            ..Default::default()
        };
        validate_forward(&mut rule).unwrap();
        assert_eq!(rule.local_host, "127.0.0.1");
        assert!(rule.name.contains("db.internal"));
    }

    #[test]
    fn validates_proxy_modes() {
        let mut rule = SshForwardRule {
            profile_id: "p".into(),
            local_port: 1080,
            mode: "socks".into(),
            ..Default::default()
        };
        validate_forward(&mut rule).unwrap();
        assert_eq!(rule.mode, "socks");

        let mut rule = SshForwardRule {
            mode: "socks".into(),
            local_port: 1080,
            ..Default::default()
        };
        validate_forward(&mut rule).unwrap();
        assert!(rule.name.contains("SOCKS"));
    }

    #[test]
    fn rewrites_http_request_head_without_extra_blank_lines() {
        let head = "GET http://example.com/path?q=1 HTTP/1.1\r\nHost: example.com\r\nUser-Agent: test\r\nConnection: keep-alive\r\n\r\n";
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap();
        let _target = parts.next().unwrap();
        let version = parts.next().unwrap();
        let rewritten = rewrite_http_request_head(lines, method, "/path?q=1", version);
        assert_eq!(
            rewritten,
            "GET /path?q=1 HTTP/1.1\r\nHost: example.com\r\nUser-Agent: test\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn splits_host_and_port() {
        assert_eq!(
            split_host_port("example.com:8080"),
            Some(("example.com".into(), 8080))
        );
        assert_eq!(split_host_port("example.com"), None);
        assert_eq!(
            split_host_port("[::1]:443"),
            Some(("::1".into(), 443))
        );
    }
}
