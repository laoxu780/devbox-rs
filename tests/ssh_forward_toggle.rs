use std::io::Write;

use devbox::ssh::{SshForwardRule, SshService};

#[test]
fn disabling_a_forward_closes_its_local_port() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let profiles_path = dir.path().join("ssh_profiles.json");
    let forwards_path = dir.path().join("ssh_forwards.json");
    let profiles = serde_json::json!({
        "profiles": [{
            "id": "p",
            "name": "test",
            "host": "127.0.0.1",
            "port": 1,
            "username": "nobody",
            "authType": "password",
            "privateKeyPath": "",
            "strictHostKeyChecking": false
        }]
    });
    let mut file = std::fs::File::create(&profiles_path).unwrap();
    file.write_all(profiles.to_string().as_bytes()).unwrap();

    let ssh = SshService::new_with_paths(runtime.handle().clone(), profiles_path, forwards_path);
    let status = ssh
        .start_forward(SshForwardRule {
            profile_id: "p".into(),
            name: "test forward".into(),
            local_host: "127.0.0.1".into(),
            local_port: 0,
            remote_host: "127.0.0.1".into(),
            remote_port: 80,
            ..Default::default()
        })
        .unwrap();
    let addr = status.local_address.clone();
    let mut client = std::net::TcpStream::connect(&addr).unwrap();
    client
        .set_read_timeout(Some(std::time::Duration::from_millis(500)))
        .unwrap();

    ssh.toggle_forward(&status.id, false).unwrap();
    runtime.block_on(async { tokio::time::sleep(std::time::Duration::from_millis(200)).await });

    let after = ssh
        .list_forwards()
        .into_iter()
        .find(|item| item.id == status.id)
        .unwrap();
    assert_eq!(after.status, "stopped", "status after disable: {after:?}");
    assert!(
        std::net::TcpStream::connect(&addr).is_err(),
        "local port {addr} is still listening after disable"
    );
    use std::io::Read;
    let mut buf = [0u8; 1];
    match client.read(&mut buf) {
        Ok(0) => {}
        Ok(n) => panic!("client still received {n} bytes after disable"),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            panic!("existing connection stayed open after disable")
        }
        Err(_) => {}
    }

    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("ssh_forwards.json")).unwrap())
            .unwrap();
    assert_eq!(
        stored["forwards"][0]["disabled"], true,
        "disabled flag was not persisted"
    );
}
