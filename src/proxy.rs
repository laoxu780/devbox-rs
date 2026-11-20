//! Runtime-switchable local HTTP reverse proxy.
//!
//! This is a direct port of `proxy_service.go`: rules are persisted to
//! `DevBox/api_proxy.json`, listeners can be toggled without restarting the
//! app, and every listener exposes a tiny control API under
//! `/__devbox__/` so scripts can switch environments too.

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::{StreamExt, TryStreamExt};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, BodyStream, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::store;
use crate::util::{display_address, escape_header, new_id, now_clock, now_rfc3339, truncate_text};

pub const ADMIN_PREFIX: &str = "/__devbox__/";
const MAX_LOGS: usize = 30;

const HOP_BY_HOP: [&str; 9] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "host",
];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTarget {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub insecure: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRoute {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_listen_host")]
    pub listen_host: String,
    #[serde(default)]
    pub listen_port: i64,
    #[serde(default)]
    pub strip_prefix: String,
    #[serde(default)]
    pub cors: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub active_target_id: String,
    #[serde(default)]
    pub targets: Vec<ProxyTarget>,
}

fn default_listen_host() -> String {
    "127.0.0.1".into()
}

impl Default for ProxyRoute {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            listen_host: default_listen_host(),
            listen_port: 8787,
            strip_prefix: String::new(),
            cors: false,
            disabled: false,
            position: 0,
            active_target_id: String::new(),
            targets: Vec::new(),
        }
    }
}

impl ProxyRoute {
    pub fn active_target(&self) -> ProxyTarget {
        self.targets
            .iter()
            .find(|target| target.id == self.active_target_id)
            .cloned()
            .or_else(|| self.targets.first().cloned())
            .unwrap_or_default()
    }

    pub fn local_address(&self, port: u16) -> String {
        display_address(&self.listen_host, port)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRequestLog {
    pub time: String,
    pub method: String,
    pub path: String,
    pub target: String,
    pub status: u16,
    pub duration_ms: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub id: String,
    pub name: String,
    pub listen_host: String,
    pub listen_port: u16,
    pub local_address: String,
    /// running | stopped | error
    pub status: String,
    #[serde(default)]
    pub error: String,
    pub disabled: bool,
    pub position: i64,
    pub strip_prefix: String,
    pub cors: bool,
    pub active_target_id: String,
    pub active_target: ProxyTarget,
    pub targets: Vec<ProxyTarget>,
    pub requests: u64,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub recent: Vec<ProxyRequestLog>,
}

#[derive(Serialize, Deserialize)]
struct ProxyRoutesFile {
    #[serde(default)]
    routes: Vec<ProxyRoute>,
}

#[derive(Default)]
struct RuntimeStatus {
    status: String,
    error: String,
    started_at: String,
    requests: u64,
    recent: Vec<ProxyRequestLog>,
    active: ProxyTarget,
    bound_port: u16,
}

struct RunningRoute {
    shutdown: Option<oneshot::Sender<()>>,
    runtime: Arc<Mutex<RuntimeStatus>>,
}

struct ProxyState {
    routes: BTreeMap<String, ProxyRoute>,
    running: HashMap<String, RunningRoute>,
    failed: HashMap<String, String>,
    path: PathBuf,
}

#[derive(Clone)]
pub struct ProxyService {
    inner: Arc<ProxyInner>,
}

struct ProxyInner {
    state: Mutex<ProxyState>,
    handle: tokio::runtime::Handle,
    secure: reqwest::Client,
    insecure: reqwest::Client,
}

impl ProxyService {
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self::new_with_path(handle, store::config_path("api_proxy.json"))
    }

    /// Testable constructor that keeps the real config file untouched.
    pub fn new_with_path(handle: tokio::runtime::Handle, path: PathBuf) -> Self {
        let mut routes = BTreeMap::new();
        let mut position = 1;
        if let Ok(Some(stored)) = store::read_json::<ProxyRoutesFile>(&path) {
            for mut route in stored.routes {
                if route.id.is_empty() || route.targets.is_empty() {
                    continue;
                }
                if route.position == 0 {
                    route.position = position;
                }
                if route.position >= position {
                    position = route.position + 1;
                }
                routes.insert(route.id.clone(), route);
            }
        }

        let secure = build_client(false);
        let insecure = build_client(true);
        let service = Self {
            inner: Arc::new(ProxyInner {
                state: Mutex::new(ProxyState {
                    routes,
                    running: HashMap::new(),
                    failed: HashMap::new(),
                    path,
                }),
                handle,
                secure,
                insecure,
            }),
        };

        // Restore enabled listeners. Binding is synchronous, so a busy port is
        // reported per route instead of blocking the whole app.
        let routes = service.ordered_routes();
        for route in routes {
            if route.disabled {
                continue;
            }
            if let Err(error) = service.start_route_inner(route.clone()) {
                service
                    .inner
                    .state
                    .lock()
                    .unwrap()
                    .failed
                    .insert(route.id, error);
            }
        }
        service
    }

    fn ordered_routes(&self) -> Vec<ProxyRoute> {
        let state = self.inner.state.lock().unwrap();
        let mut routes: Vec<_> = state.routes.values().cloned().collect();
        routes.sort_by_key(|route| (route.position, route.id.clone()));
        routes
    }

    fn next_position(state: &ProxyState) -> i64 {
        state
            .routes
            .values()
            .map(|route| route.position)
            .max()
            .unwrap_or(0)
            + 1
    }

    fn persist(state: &ProxyState) -> Result<(), String> {
        let mut routes: Vec<_> = state.routes.values().cloned().collect();
        routes.sort_by_key(|route| (route.position, route.id.clone()));
        store::write_json(&state.path, &ProxyRoutesFile { routes })
            .map_err(|error| format!("保存接口转发规则失败：{error}"))
    }

    pub fn list_routes(&self) -> Vec<ProxyStatus> {
        let state = self.inner.state.lock().unwrap();
        let mut routes: Vec<_> = state.routes.values().cloned().collect();
        routes.sort_by_key(|route| (route.position, route.id.clone()));
        routes
            .into_iter()
            .map(|route| {
                let mut status = ProxyStatus {
                    id: route.id.clone(),
                    name: route.name.clone(),
                    listen_host: route.listen_host.clone(),
                    listen_port: route.listen_port.max(0) as u16,
                    local_address: route.local_address(route.listen_port.max(0) as u16),
                    status: "stopped".into(),
                    disabled: route.disabled,
                    position: route.position,
                    strip_prefix: route.strip_prefix.clone(),
                    cors: route.cors,
                    active_target_id: route.active_target_id.clone(),
                    active_target: route.active_target(),
                    targets: route.targets.clone(),
                    ..Default::default()
                };
                if let Some(running) = state.running.get(&route.id) {
                    let runtime = running.runtime.lock().unwrap();
                    status.status = runtime.status.clone();
                    status.error = runtime.error.clone();
                    status.requests = runtime.requests;
                    status.started_at = runtime.started_at.clone();
                    status.active_target = runtime.active.clone();
                    status.active_target_id = runtime.active.id.clone();
                    status.listen_port = runtime.bound_port;
                    status.local_address = route.local_address(runtime.bound_port);
                    status.recent = runtime.recent.clone();
                } else if let Some(message) = state.failed.get(&route.id) {
                    status.status = "error".into();
                    status.error = message.clone();
                }
                status
            })
            .collect()
    }

    pub fn route_status(&self, id: &str) -> Option<ProxyStatus> {
        self.list_routes().into_iter().find(|route| route.id == id)
    }

    pub fn start_route(&self, mut route: ProxyRoute) -> Result<ProxyStatus, String> {
        validate_route(&mut route)?;
        let id = new_id();
        route.id = id.clone();
        route.disabled = false;
        {
            let mut state = self.inner.state.lock().unwrap();
            route.position = Self::next_position(&state);
            state.routes.insert(id.clone(), route.clone());
            Self::persist(&state)?;
        }
        if let Err(error) = self.start_route_inner(route) {
            self.inner
                .state
                .lock()
                .unwrap()
                .failed
                .insert(id.clone(), error.clone());
            return Err(error);
        }
        self.route_status(&id)
            .ok_or_else(|| "转发规则不存在".to_string())
    }

    pub fn update_route(&self, mut route: ProxyRoute) -> Result<ProxyStatus, String> {
        validate_route(&mut route)?;
        let id = route.id.clone();
        let was_active;
        {
            let mut state = self.inner.state.lock().unwrap();
            let Some(old) = state.routes.get(&id).cloned() else {
                return Err("转发规则不存在".into());
            };
            was_active = !old.disabled;
            route.disabled = old.disabled;
            route.position = old.position;
            state.routes.insert(id.clone(), route.clone());
            Self::persist(&state)?;
        }
        self.stop_runtime(&id);
        if !was_active {
            return self
                .route_status(&id)
                .ok_or_else(|| "转发规则不存在".into());
        }
        if let Err(error) = self.start_route_inner(route) {
            self.inner
                .state
                .lock()
                .unwrap()
                .failed
                .insert(id.clone(), error.clone());
            return Err(error);
        }
        self.route_status(&id)
            .ok_or_else(|| "转发规则不存在".to_string())
    }

    pub fn delete_route(&self, id: &str) -> Result<(), String> {
        self.stop_runtime(id);
        let mut state = self.inner.state.lock().unwrap();
        if state.routes.remove(id).is_none() {
            return Err("转发规则不存在".into());
        }
        state.failed.remove(id);
        Self::persist(&state)
    }

    pub fn toggle_route(&self, id: &str, enabled: bool) -> Result<String, String> {
        let route = {
            let mut state = self.inner.state.lock().unwrap();
            let Some(route) = state.routes.get_mut(id) else {
                return Err("转发规则不存在".into());
            };
            route.disabled = !enabled;
            let route = route.clone();
            Self::persist(&state)?;
            route
        };
        if !enabled {
            self.stop_runtime(id);
            self.inner.state.lock().unwrap().failed.remove(id);
            return Ok("接口转发已关闭".into());
        }
        if let Err(error) = self.start_route_inner(route) {
            self.inner
                .state
                .lock()
                .unwrap()
                .failed
                .insert(id.to_string(), error.clone());
            return Err(error);
        }
        Ok("接口转发已开启".into())
    }

    pub fn switch_target(&self, id: &str, target_ref: &str) -> Result<ProxyStatus, String> {
        let target_ref = target_ref.trim();
        if target_ref.is_empty() {
            return Err("请选择要切换的环境".into());
        }
        {
            let mut state = self.inner.state.lock().unwrap();
            let Some(route) = state.routes.get_mut(id) else {
                return Err("转发规则不存在".into());
            };
            let chosen = route
                .targets
                .iter()
                .find(|target| {
                    target.id == target_ref || target.name.eq_ignore_ascii_case(target_ref)
                })
                .cloned()
                .ok_or_else(|| "环境不存在".to_string())?;
            route.active_target_id = chosen.id.clone();
            if let Some(running) = state.running.get(id) {
                let mut runtime = running.runtime.lock().unwrap();
                runtime.active = chosen;
            }
            Self::persist(&state)?;
        }
        self.route_status(id)
            .ok_or_else(|| "转发规则不存在".into())
    }

    pub fn close(&self) {
        let ids: Vec<String> = self
            .inner
            .state
            .lock()
            .unwrap()
            .running
            .keys()
            .cloned()
            .collect();
        for id in ids {
            self.stop_runtime(&id);
        }
    }

    fn stop_runtime(&self, id: &str) {
        let running = {
            let mut state = self.inner.state.lock().unwrap();
            state.running.remove(id)
        };
        if let Some(mut running) = running {
            if let Some(shutdown) = running.shutdown.take() {
                let _ = shutdown.send(());
            }
        }
    }

    fn start_route_inner(&self, route: ProxyRoute) -> Result<(), String> {
        // A duplicate start (startup restore racing a user toggle) must not
        // leak the previous listener.
        self.stop_runtime(&route.id);

        let bind_host = if route.listen_host.trim().is_empty() {
            "127.0.0.1".to_string()
        } else {
            route.listen_host.trim().to_string()
        };
        let requested = if (0..=65535).contains(&route.listen_port) {
            route.listen_port as u16
        } else {
            return Err("监听端口无效".into());
        };
        let std_listener = bind_listener(&bind_host, requested)?;
        std_listener
            .set_nonblocking(true)
            .map_err(|error| format!("配置监听失败: {error}"))?;
        let bound_port = std_listener
            .local_addr()
            .map_err(|error| format!("读取监听端口失败: {error}"))?
            .port();
        // Startup restore happens on the UI thread, outside any entered Tokio
        // context; `from_std` requires one.
        let listener = {
            let _guard = self.inner.handle.enter();
            TcpListener::from_std(std_listener)
                .map_err(|error| format!("启动监听失败: {error}"))?
        };

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let runtime = Arc::new(Mutex::new(RuntimeStatus {
            status: "running".into(),
            started_at: now_rfc3339(),
            active: route.active_target(),
            bound_port,
            ..Default::default()
        }));

        {
            let mut state = self.inner.state.lock().unwrap();
            if let Some(stored) = state.routes.get_mut(&route.id) {
                if stored.disabled {
                    return Err("转发已关闭".into());
                }
                if stored.listen_port == 0 {
                    stored.listen_port = bound_port as i64;
                    let _ = Self::persist(&state);
                }
            }
            state.failed.remove(&route.id);
            state.running.insert(
                route.id.clone(),
                RunningRoute {
                    shutdown: Some(shutdown_tx),
                    runtime: runtime.clone(),
                },
            );
        }

        let service = self.clone();
        let route_id = route.id.clone();
        let rule = route.clone();
        self.inner.handle.spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => {
                        let Ok((stream, peer)) = accepted else { break };
                        let service = service.clone();
                        let route_id = route_id.clone();
                        let rule = rule.clone();
                        let runtime = runtime.clone();
                        tokio::spawn(async move {
                            let io = TokioIo::new(stream);
                            let handler = service_fn(move |request| {
                                serve_request(
                                    request,
                                    peer,
                                    service.clone(),
                                    route_id.clone(),
                                    rule.clone(),
                                    runtime.clone(),
                                )
                            });
                            let _ = hyper::server::conn::http1::Builder::new()
                                .serve_connection(io, handler)
                                .with_upgrades()
                                .await;
                        });
                    }
                }
            }
        });
        Ok(())
    }
}

/// Binds a listener, retrying briefly so a just-stopped runtime has time to
/// release its socket before the same route is started again.
fn bind_listener(host: &str, port: u16) -> Result<std::net::TcpListener, String> {
    let mut last_error = None;
    for _ in 0..50 {
        match std::net::TcpListener::bind((host, port)) {
            Ok(listener) => return Ok(listener),
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    Err(format!(
        "监听 {} 失败: {}",
        display_address(host, port),
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "未知错误".into())
    ))
}

fn build_client(insecure: bool) -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(insecure)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .expect("build proxy http client")
}

fn validate_route(route: &mut ProxyRoute) -> Result<(), String> {
    route.name = route.name.trim().to_string();
    route.listen_host = route.listen_host.trim().to_string();
    if route.listen_host.is_empty() {
        route.listen_host = "127.0.0.1".into();
    }
    if !(0..=65535).contains(&route.listen_port) {
        return Err("监听端口无效".into());
    }
    route.strip_prefix = route.strip_prefix.trim().to_string();
    if !route.strip_prefix.is_empty() {
        if !route.strip_prefix.starts_with('/') {
            route.strip_prefix.insert(0, '/');
        }
        route.strip_prefix = route.strip_prefix.trim_end_matches('/').to_string();
        if route.strip_prefix.is_empty() {
            return Err("路径前缀不能为 /".into());
        }
    }
    if route.targets.is_empty() {
        return Err("至少需要配置一个后端环境".into());
    }

    let mut seen = std::collections::HashSet::new();
    for target in &mut route.targets {
        target.name = target.name.trim().to_string();
        target.url = target.url.trim().to_string();
        if target.url.is_empty() {
            return Err("后端地址不能为空".into());
        }
        let parsed = url::Url::parse(&target.url)
            .map_err(|_| format!("后端地址无效：{}", target.url))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(format!("后端地址无效：{}", target.url));
        }
        target.url = target.url.trim_end_matches('/').to_string();
        if target.id.is_empty() || !seen.insert(target.id.clone()) {
            target.id = new_id();
            seen.insert(target.id.clone());
        }
        if target.name.is_empty() {
            target.name = parsed.host_str().unwrap_or("后端").to_string();
        }
    }
    if !route.targets.iter().any(|t| t.id == route.active_target_id) {
        route.active_target_id = route.targets[0].id.clone();
    }
    if route.name.is_empty() {
        route.name = format!(
            "接口转发 {}",
            display_address(&route.listen_host, route.listen_port as u16)
                .trim_start_matches("http://")
                .to_string()
        );
    }
    Ok(())
}

fn is_admin_path(path: &str) -> bool {
    path == ADMIN_PREFIX.trim_end_matches('/') || path.starts_with(ADMIN_PREFIX)
}

async fn serve_request(
    mut request: Request<Incoming>,
    peer: SocketAddr,
    service: ProxyService,
    route_id: String,
    rule: ProxyRoute,
    runtime: Arc<Mutex<RuntimeStatus>>,
) -> Result<Response<BoxBody<Bytes, std::io::Error>>, Infallible> {
    if is_admin_path(request.uri().path()) {
        return Ok(handle_admin(request, &service, &route_id).await);
    }

    let active = runtime.lock().unwrap().active.clone();
    if active.url.is_empty() {
        return Ok(text_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "DevBox: 还没有配置后端环境",
        ));
    }

    let started = Instant::now();
    let method = request.method().clone();
    let display_path = truncate_text(&request.uri().to_string(), 180);

    if rule.cors
        && method == Method::OPTIONS
        && request
            .headers()
            .get("access-control-request-method")
            .is_some()
    {
        let mut response = Response::new(empty_body());
        *response.status_mut() = StatusCode::NO_CONTENT;
        set_cors_headers(response.headers_mut());
        record(&runtime, &method, &display_path, &active, 204, started, "");
        return Ok(response);
    }

    let is_upgrade = request
        .headers()
        .get(http::header::UPGRADE)
        .is_some()
        || request
            .headers()
            .get(http::header::CONNECTION)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_ascii_lowercase().contains("upgrade"))
            .unwrap_or(false);

    let on_upgrade = if is_upgrade {
        Some(hyper::upgrade::on(&mut request))
    } else {
        None
    };

    let upstream = match build_upstream(&rule, &active, request.uri()) {
        Ok(url) => url,
        Err(message) => {
            record(
                &runtime,
                &method,
                &display_path,
                &active,
                502,
                started,
                &message,
            );
            return Ok(devbox_error_response(
                &active,
                rule.cors,
                StatusCode::BAD_GATEWAY,
                &message,
            ));
        }
    };

    let mut headers = HeaderMap::new();
    for (name, value) in request.headers() {
        if HOP_BY_HOP.contains(&name.as_str()) || name == http::header::CONTENT_LENGTH {
            continue;
        }
        headers.insert(name.clone(), value.clone());
    }
    headers.insert(
        HeaderName::from_static("x-devbox-env"),
        HeaderValue::from_str(&escape_header(&active.name))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    headers.insert(
        HeaderName::from_static("x-devbox-upstream"),
        HeaderValue::from_str(&escape_header(&active.url))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    headers.insert(
        HeaderName::from_static("x-forwarded-for"),
        HeaderValue::from_str(&peer.ip().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    if let Some(host) = request.headers().get(http::header::HOST) {
        headers.insert(HeaderName::from_static("x-forwarded-host"), host.clone());
    }
    headers.insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static("http"),
    );

    let body = BodyStream::new(request.into_body())
        .try_filter_map(|frame| async move { Ok(frame.into_data().ok()) })
        .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) });
    let body = reqwest::Body::wrap_stream(body);

    let client = if active.insecure {
        &service.inner.insecure
    } else {
        &service.inner.secure
    };
    let mut outgoing = client
        .request(
            reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
            upstream,
        )
        .headers(headers)
        .body(body);
    if is_upgrade {
        outgoing = outgoing.version(reqwest::Version::HTTP_11);
    }

    let upstream_response = match outgoing.send().await {
        Ok(response) => response,
        Err(error) => {
            let message = error.to_string();
            record(
                &runtime,
                &method,
                &display_path,
                &active,
                502,
                started,
                &message,
            );
            return Ok(devbox_error_response(
                &active,
                rule.cors,
                StatusCode::BAD_GATEWAY,
                &message,
            ));
        }
    };

    let status = upstream_response.status();
    let upstream_headers = upstream_response.headers().clone();

    // WebSocket and other HTTP/1.1 upgrades: relay the 101 response, then
    // splice the two upgraded byte streams together.
    if status == StatusCode::SWITCHING_PROTOCOLS {
        if let Some(on_upgrade) = on_upgrade {
            let mut builder = Response::builder().status(StatusCode::SWITCHING_PROTOCOLS);
            for (name, value) in &upstream_headers {
                if name.as_str() == "connection" || name.as_str() == "upgrade" {
                    builder = builder.header(name, value);
                }
            }
            builder = builder
                .header("connection", "upgrade")
                .header("x-devbox-env", escape_header(&active.name))
                .header("x-devbox-upstream", escape_header(&active.url));
            let downstream_response = builder
                .body(empty_body())
                .unwrap_or_else(|_| Response::new(empty_body()));
            record(
                &runtime,
                &method,
                &display_path,
                &active,
                101,
                started,
                "",
            );
            tokio::spawn(async move {
                let Ok(downstream) = on_upgrade.await else {
                    return;
                };
                let Ok(mut upstream) = upstream_response.upgrade().await else {
                    return;
                };
                let mut downstream = TokioIo::new(downstream);
                let _ =
                    tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await;
            });
            return Ok(downstream_response);
        }
    }

    let mut builder = Response::builder().status(status);
    for (name, value) in &upstream_headers {
        if HOP_BY_HOP.contains(&name.as_str()) {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder = builder
        .header("x-devbox-env", escape_header(&active.name))
        .header("x-devbox-upstream", escape_header(&active.url));
    let mut downstream_response = builder.body(empty_body()).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(text_body("DevBox: 构建响应失败"))
            .unwrap()
    });
    if rule.cors {
        add_cors_headers(downstream_response.headers_mut());
    }

    *downstream_response.body_mut() = response_body_stream(upstream_response);

    record(
        &runtime,
        &method,
        &display_path,
        &active,
        status.as_u16(),
        started,
        "",
    );
    Ok(downstream_response)
}

fn response_body_stream(response: reqwest::Response) -> BoxBody<Bytes, std::io::Error> {
    let stream = response
        .bytes_stream()
        .map(|chunk| chunk.map(Frame::data).map_err(std::io::Error::other));
    BodyExt::boxed(StreamBody::new(stream))
}

fn empty_body() -> BoxBody<Bytes, std::io::Error> {
    Full::new(Bytes::new())
        .map_err(|never| -> std::io::Error { match never {} })
        .boxed()
}

fn text_body(text: &str) -> BoxBody<Bytes, std::io::Error> {
    Full::new(Bytes::from(text.to_string()))
        .map_err(|never| -> std::io::Error { match never {} })
        .boxed()
}

fn text_response(status: StatusCode, message: &str) -> Response<BoxBody<Bytes, std::io::Error>> {
    let mut response = Response::new(text_body(message));
    *response.status_mut() = status;
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn devbox_error_response(
    target: &ProxyTarget,
    cors: bool,
    status: StatusCode,
    message: &str,
) -> Response<BoxBody<Bytes, std::io::Error>> {
    let text = format!(
        "DevBox 接口转发失败\n环境: {}\n目标: {}\n错误: {}\n",
        target.name, target.url, message
    );
    let mut response = text_response(status, &text);
    response.headers_mut().insert(
        HeaderName::from_static("x-devbox-env"),
        HeaderValue::from_str(&escape_header(&target.name))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-devbox-upstream"),
        HeaderValue::from_str(&escape_header(&target.url))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    if cors {
        set_cors_headers(response.headers_mut());
    }
    response
}

fn build_upstream(
    rule: &ProxyRoute,
    target: &ProxyTarget,
    uri: &http::Uri,
) -> Result<reqwest::Url, String> {
    let base = reqwest::Url::parse(&target.url).map_err(|error| error.to_string())?;
    let mut path = uri.path().to_string();
    if !rule.strip_prefix.is_empty() {
        if path == rule.strip_prefix {
            path = "/".into();
        } else if let Some(rest) = path.strip_prefix(&format!("{}/", rule.strip_prefix)) {
            path = format!("/{rest}");
        }
    }
    let base_path = base.path().trim_end_matches('/');
    let joined = if base_path.is_empty() {
        if path.is_empty() {
            "/".to_string()
        } else {
            path
        }
    } else if path.is_empty() || path == "/" {
        format!("{base_path}/")
    } else {
        format!("{base_path}/{}", path.trim_start_matches('/'))
    };
    let mut url = base;
    url.set_path(&joined);
    let query = match (url.query(), uri.query()) {
        (Some(base), Some(incoming)) if !incoming.is_empty() => Some(format!("{base}&{incoming}")),
        (Some(base), _) => Some(base.to_string()),
        (None, Some(incoming)) => Some(incoming.to_string()),
        (None, None) => None,
    };
    url.set_query(query.as_deref());
    Ok(url)
}

fn record(
    runtime: &Arc<Mutex<RuntimeStatus>>,
    method: &Method,
    path: &str,
    target: &ProxyTarget,
    status: u16,
    started: Instant,
    error: &str,
) {
    let entry = ProxyRequestLog {
        time: now_clock(),
        method: method.to_string(),
        path: path.to_string(),
        target: target.name.clone(),
        status,
        duration_ms: started.elapsed().as_millis() as i64,
        error: error.to_string(),
    };
    let mut runtime = runtime.lock().unwrap();
    runtime.requests += 1;
    runtime.recent.insert(0, entry);
    runtime.recent.truncate(MAX_LOGS);
}

fn add_cors_headers(headers: &mut HeaderMap) {
    if !headers.contains_key("access-control-allow-origin") {
        headers.insert(
            http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    }
    if !headers.contains_key("access-control-expose-headers") {
        headers.insert(
            http::header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static("*"),
        );
    }
}

fn set_cors_headers(headers: &mut HeaderMap) {
    headers.insert(
        http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        http::header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS, HEAD"),
    );
    headers.insert(
        http::header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        http::header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        http::header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
}

async fn handle_admin(
    request: Request<Incoming>,
    service: &ProxyService,
    route_id: &str,
) -> Response<BoxBody<Bytes, std::io::Error>> {
    let mut path = request.uri().path();
    if path == ADMIN_PREFIX.trim_end_matches('/') {
        path = ADMIN_PREFIX;
    }
    let action = path
        .trim_start_matches(ADMIN_PREFIX)
        .trim_matches('/')
        .to_string();
    let method = request.method().clone();

    let json = |status: StatusCode, value: serde_json::Value| {
        let text = serde_json::to_string(&value).unwrap_or_else(|_| "{}".into());
        let mut response = Response::new(text_body(&text));
        *response.status_mut() = status;
        response.headers_mut().insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        response
            .headers_mut()
            .insert(http::header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    };

    match action.as_str() {
        "" | "status" => {
            if method != Method::GET {
                return json(
                    StatusCode::METHOD_NOT_ALLOWED,
                    serde_json::json!({"ok": false, "error": "仅支持 GET"}),
                );
            }
            match service.route_status(route_id) {
                Some(status) => json(
                    StatusCode::OK,
                    serde_json::json!({
                        "route": status,
                        "status": format!("GET {ADMIN_PREFIX}status"),
                        "switch": format!("POST {ADMIN_PREFIX}switch?target=<环境名称或 ID>"),
                    }),
                ),
                None => json(
                    StatusCode::NOT_FOUND,
                    serde_json::json!({"ok": false, "error": "转发规则不存在"}),
                ),
            }
        }
        "switch" => {
            if method != Method::GET && method != Method::POST {
                return json(
                    StatusCode::METHOD_NOT_ALLOWED,
                    serde_json::json!({"ok": false, "error": "仅支持 GET / POST"}),
                );
            }
            let mut target_ref = request
                .uri()
                .query()
                .and_then(|query| {
                    url::form_urlencoded::parse(query.as_bytes())
                        .find(|(key, _)| key == "target")
                        .map(|(_, value)| value.to_string())
                })
                .unwrap_or_default();
            if target_ref.is_empty() {
                if let Ok(body) = request.into_body().collect().await {
                    let bytes = body.to_bytes();
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        target_ref = value
                            .get("target")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string();
                    }
                }
            }
            match service.switch_target(route_id, &target_ref) {
                Ok(status) => json(
                    StatusCode::OK,
                    serde_json::json!({
                        "ok": true,
                        "route": status.name,
                        "active": status.active_target,
                    }),
                ),
                Err(error) => json(
                    StatusCode::BAD_REQUEST,
                    serde_json::json!({"ok": false, "error": error}),
                ),
            }
        }
        _ => json(
            StatusCode::NOT_FOUND,
            serde_json::json!({"ok": false, "error": "未知操作，可用: status, switch"}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_normalises_routes() {
        let mut route = ProxyRoute {
            name: "  ".into(),
            listen_host: " ".into(),
            listen_port: 8787,
            strip_prefix: "api/".into(),
            targets: vec![
                ProxyTarget {
                    id: String::new(),
                    name: String::new(),
                    url: "http://127.0.0.1:8080/".into(),
                    insecure: false,
                },
                ProxyTarget {
                    id: String::new(),
                    name: "prod".into(),
                    url: "https://api.example.com".into(),
                    insecure: true,
                },
            ],
            ..Default::default()
        };
        validate_route(&mut route).unwrap();
        assert_eq!(route.listen_host, "127.0.0.1");
        assert_eq!(route.strip_prefix, "/api");
        assert_eq!(route.targets[0].url, "http://127.0.0.1:8080");
        assert_eq!(route.targets[0].name, "127.0.0.1");
        assert!(!route.targets[0].id.is_empty());
        assert_eq!(route.active_target_id, route.targets[0].id);
        assert!(route.name.contains("接口转发"));
    }

    #[test]
    fn rejects_bad_input() {
        let mut route = ProxyRoute {
            listen_port: 70000,
            ..Default::default()
        };
        assert!(validate_route(&mut route).is_err());

        let mut route = ProxyRoute {
            listen_port: 8787,
            strip_prefix: "/".into(),
            ..Default::default()
        };
        assert!(validate_route(&mut route).is_err());

        let mut route = ProxyRoute {
            listen_port: 8787,
            targets: vec![ProxyTarget {
                url: "ftp://example.com".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(validate_route(&mut route).is_err());
    }

    #[test]
    fn joins_upstream_paths() {
        let rule = ProxyRoute {
            strip_prefix: "/api".into(),
            ..Default::default()
        };
        let target = ProxyTarget {
            url: "http://example.com/base".into(),
            ..Default::default()
        };
        let uri: http::Uri = "/api/users?x=1".parse().unwrap();
        let url = build_upstream(&rule, &target, &uri).unwrap();
        assert_eq!(url.as_str(), "http://example.com/base/users?x=1");
    }

    #[test]
    fn strips_prefix_exactly() {
        let rule = ProxyRoute {
            strip_prefix: "/api".into(),
            ..Default::default()
        };
        let target = ProxyTarget {
            url: "http://example.com".into(),
            ..Default::default()
        };
        let uri: http::Uri = "/api".parse().unwrap();
        let url = build_upstream(&rule, &target, &uri).unwrap();
        assert_eq!(url.as_str(), "http://example.com/");
    }

    async fn mock_upstream(name: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);
                    let service = service_fn(move |request: Request<Incoming>| async move {
                        let path = request.uri().path().to_string();
                        Ok::<_, Infallible>(Response::new(Full::new(Bytes::from(format!(
                            "{name}:{path}"
                        )))))
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(io, service)
                        .await;
                });
            }
        });
        port
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn proxies_and_switches_targets() {
        let port_a = mock_upstream("A").await;
        let port_b = mock_upstream("B").await;
        let dir = tempfile::tempdir().unwrap();
        let service = ProxyService::new_with_path(
            tokio::runtime::Handle::current(),
            dir.path().join("api_proxy.json"),
        );
        let route = ProxyRoute {
            name: "test".into(),
            listen_host: "127.0.0.1".into(),
            listen_port: 0,
            strip_prefix: "/api".into(),
            targets: vec![
                ProxyTarget {
                    name: "dev".into(),
                    url: format!("http://127.0.0.1:{port_a}"),
                    ..Default::default()
                },
                ProxyTarget {
                    name: "prod".into(),
                    url: format!("http://127.0.0.1:{port_b}"),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let status = service.start_route(route).unwrap();
        assert_eq!(status.status, "running");
        let base = format!("http://127.0.0.1:{}", status.listen_port);

        let client = reqwest::Client::new();
        let body = client
            .get(format!("{base}/api/hello?x=1"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "A:/hello");

        let switched = service.switch_target(&status.id, "PROD").unwrap();
        assert_eq!(switched.active_target.name, "prod");
        let body = client
            .get(format!("{base}/api/hello"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "B:/hello");

        let admin: serde_json::Value = client
            .get(format!("{base}/__devbox__/status"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(admin["route"]["activeTarget"]["name"], "prod");
        assert!(admin["route"]["requests"].as_u64().unwrap() >= 2);

        let switched: serde_json::Value = client
            .post(format!("{base}/__devbox__/switch?target=dev"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(switched["active"]["name"], "dev");

        service.toggle_route(&status.id, false).unwrap();
        assert_eq!(service.list_routes()[0].status, "stopped");
        service.toggle_route(&status.id, true).unwrap();
        assert_eq!(service.list_routes()[0].status, "running");
        service.close();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn persists_routes_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_proxy.json");
        let port_a = mock_upstream("A").await;
        let service = ProxyService::new_with_path(tokio::runtime::Handle::current(), path.clone());
        let status = service
            .start_route(ProxyRoute {
                name: "persisted".into(),
                listen_host: "127.0.0.1".into(),
                listen_port: 0,
                targets: vec![ProxyTarget {
                    name: "dev".into(),
                    url: format!("http://127.0.0.1:{port_a}"),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let id = status.id.clone();
        service.close();

        let restored = ProxyService::new_with_path(tokio::runtime::Handle::current(), path);
        let routes = restored.list_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].id, id);
        assert_eq!(routes[0].status, "running");
        restored.close();
    }
}
