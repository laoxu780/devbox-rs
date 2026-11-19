//! Local port inspection: list listening sockets and test whether ports are free.
//!
//! macOS / Linux use `lsof`, Windows uses `netstat`. Both parsers are kept
//! separate from the UI so they can be unit tested without touching the
//! machine's network state.

use std::collections::HashSet;
use std::net::TcpListener;
use std::process::Command;

/// A socket that is currently accepting (TCP) or bound (UDP) locally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListenEntry {
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub process: String,
    pub pid: u32,
}

/// Result of testing whether a local port can be bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortStatus {
    pub port: u16,
    pub available: bool,
    pub detail: String,
}

/// Maximum number of ports one check may cover, so a pasted range cannot
/// freeze the UI.
const MAX_CHECK_PORTS: usize = 200;

pub fn list_listeners() -> Result<Vec<ListenEntry>, String> {
    #[cfg(target_os = "windows")]
    {
        list_listeners_netstat()
    }
    #[cfg(not(target_os = "windows"))]
    {
        list_listeners_lsof()
    }
}

#[cfg(not(target_os = "windows"))]
fn list_listeners_lsof() -> Result<Vec<ListenEntry>, String> {
    let mut entries = Vec::new();
    for (args, protocol) in [
        (&["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"][..], "TCP"),
        (&["-nP", "-iUDP", "-F", "pcn"][..], "UDP"),
    ] {
        let output = Command::new("lsof")
            .args(args)
            .output()
            .map_err(|error| format!("无法运行 lsof：{error}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "lsof 执行失败".into()
            } else {
                stderr
            });
        }
        entries.extend(parse_lsof_f(&String::from_utf8_lossy(&output.stdout), protocol));
    }
    Ok(entries)
}

/// Parse `lsof -F pcn` output. Each field is one line prefixed with its code;
/// a new `p` record starts a new process.
pub fn parse_lsof_f(output: &str, protocol: &str) -> Vec<ListenEntry> {
    fn flush(
        pid: &mut Option<u32>,
        command: &mut String,
        names: &mut Vec<String>,
        out: &mut Vec<ListenEntry>,
        protocol: &str,
    ) {
        let Some(pid_value) = pid.take() else { return };
        for name in names.drain(..) {
            if let Some((address, port)) = split_address_port(&name) {
                out.push(ListenEntry {
                    protocol: protocol.to_string(),
                    address,
                    port,
                    process: if command.is_empty() {
                        "unknown".into()
                    } else {
                        command.clone()
                    },
                    pid: pid_value,
                });
            }
        }
        command.clear();
    }

    let mut result = Vec::new();
    let mut pid = None;
    let mut command = String::new();
    let mut names = Vec::new();
    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let (key, value) = line.split_at(1);
        match key {
            "p" => {
                flush(&mut pid, &mut command, &mut names, &mut result, protocol);
                pid = value.trim().parse().ok();
            }
            "c" => command = value.trim().to_string(),
            "n" => names.push(value.trim().to_string()),
            _ => {}
        }
    }
    flush(&mut pid, &mut command, &mut names, &mut result, protocol);
    result
}

fn split_address_port(name: &str) -> Option<(String, u16)> {
    let (address, port) = if let Some(rest) = name.strip_prefix('[') {
        let (host, rest) = rest.split_once(']')?;
        (format!("[{host}]"), rest.strip_prefix(':')?)
    } else {
        let (host, port) = name.rsplit_once(':')?;
        (host.to_string(), port)
    };
    Some((address, port.parse().ok()?))
}

#[cfg(target_os = "windows")]
fn list_listeners_netstat() -> Result<Vec<ListenEntry>, String> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|error| format!("无法运行 netstat：{error}"))?;
    if !output.status.success() {
        return Err("netstat 执行失败".into());
    }
    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 || !parts[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let Some((address, port)) = split_address_port(parts[1]) else {
            continue;
        };
        let Ok(pid) = parts[4].parse() else { continue };
        entries.push(ListenEntry {
            protocol: "TCP".into(),
            address,
            port,
            process: "—".into(),
            pid,
        });
    }
    Ok(entries)
}

/// Parse a user-supplied port list: `8080, 3000-3010`, whitespace and Chinese
/// commas are accepted. Results are de-duplicated and capped.
pub fn parse_port_input(input: &str) -> Vec<u16> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for token in input.split(|c: char| {
        c.is_whitespace() || matches!(c, ',' | ';' | '，' | '；' | '、')
    }) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some((start, end)) = token.split_once('-') {
            let (Ok(start), Ok(end)) = (start.trim().parse::<u16>(), end.trim().parse::<u16>())
            else {
                continue;
            };
            if start > end {
                continue;
            }
            for port in start..=end {
                if result.len() >= MAX_CHECK_PORTS {
                    return result;
                }
                if seen.insert(port) {
                    result.push(port);
                }
            }
        } else if let Ok(port) = token.parse::<u16>() {
            if result.len() >= MAX_CHECK_PORTS {
                break;
            }
            if seen.insert(port) {
                result.push(port);
            }
        }
    }
    result
}

/// Test each port by attempting to bind it. An occupied port is annotated with
/// the listener that owns it when available.
pub fn check_ports(ports: &[u16], listeners: &[ListenEntry]) -> Vec<PortStatus> {
    ports
        .iter()
        .map(|&port| {
            if port == 0 {
                return PortStatus {
                    port,
                    available: false,
                    detail: "端口 0 由系统自动分配".into(),
                };
            }
            // macOS allows binding 0.0.0.0 even when 127.0.0.1:<port> is
            // occupied, so probe both addresses before declaring a port free.
            let mut occupied = false;
            let mut permission_denied = false;
            let mut failure = None;
            for host in ["0.0.0.0", "127.0.0.1"] {
                match TcpListener::bind((host, port)) {
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                        occupied = true
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                        permission_denied = true
                    }
                    Err(error) => failure = Some(error),
                }
            }
            if let Some(error) = failure {
                return PortStatus {
                    port,
                    available: false,
                    detail: format!("检测失败：{error}"),
                };
            }
            let owner = listeners
                .iter()
                .find(|entry| entry.port == port && entry.protocol == "TCP");
            if occupied {
                PortStatus {
                    port,
                    available: false,
                    detail: match owner {
                        Some(entry) => format!("被 {} (PID {}) 占用", entry.process, entry.pid),
                        None => "端口已被占用".into(),
                    },
                }
            } else {
                PortStatus {
                    port,
                    available: true,
                    detail: if permission_denied {
                        "可用（绑定需要管理员权限）".into()
                    } else {
                        "可用".into()
                    },
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lsof_f_records() {
        let output = "p123\ncGoogle Chrome\nn*:8080\nn127.0.0.1:3000\np456\ncpython3\nn[::1]:5432\n";
        let entries = parse_lsof_f(output, "TCP");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].pid, 123);
        assert_eq!(entries[0].process, "Google Chrome");
        assert_eq!(entries[0].address, "*");
        assert_eq!(entries[0].port, 8080);
        assert_eq!(entries[1].address, "127.0.0.1");
        assert_eq!(entries[2].address, "[::1]");
        assert_eq!(entries[2].pid, 456);
        assert_eq!(entries[2].port, 5432);
    }

    #[test]
    fn parses_port_ranges_and_deduplicates() {
        assert_eq!(parse_port_input("8080, 3000-3002,8080"), vec![8080, 3000, 3001, 3002]);
        assert_eq!(parse_port_input("80 443；5432"), vec![80, 443, 5432]);
        assert_eq!(parse_port_input("3002-3000"), Vec::<u16>::new());
        assert!(parse_port_input("abc").is_empty());
    }

    #[test]
    fn detects_an_occupied_port() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let entries = vec![ListenEntry {
            protocol: "TCP".into(),
            address: "127.0.0.1".into(),
            port,
            process: "test".into(),
            pid: 42,
        }];
        let status = &check_ports(&[port], &entries)[0];
        assert!(!status.available);
        assert!(status.detail.contains("test"));
    }
}
