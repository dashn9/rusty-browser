/// Queries a public IP echo service to determine this machine's outbound public IP.
pub async fn detect_public_ip() -> Option<String> {
    let ip = reqwest::get("https://api.ipify.org")
        .await.ok()?
        .text()
        .await.ok()?;
    let ip = ip.trim().to_string();
    if ip.is_empty() { None } else { Some(ip) }
}

/// Binds to port 0 and lets the OS assign a free port, then returns it.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("0.0.0.0:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("failed to find a free port")
}
