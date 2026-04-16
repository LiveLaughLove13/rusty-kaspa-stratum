//! Prometheus HTTP: `/metrics`, `/api/*`, static dashboard files.
//!
//! Split across `static_files`, `stats_json/` (types + parse + aggregate), `config_api`, and `serve`.

mod config_api;
mod ops_access;
mod serve;
mod static_files;
mod stats_json;

pub use config_api::{set_web_config_path, set_web_status_config};
pub use serve::{start_prom_server, start_web_server_all};

#[cfg(test)]
mod tests {
    use super::config_api::{set_web_config_path, set_web_status_config};
    use super::serve::{HttpMode, handle_http_request};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::io::AsyncReadExt;

    async fn send_request(mode: HttpMode, request: &str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let request = request.to_string();
        let server = tokio::spawn(async move {
            let (stream, peer) = listener.accept().await.unwrap();
            handle_http_request(stream, &request, &mode, peer)
                .await
                .unwrap();
        });

        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
        server.await.unwrap();
        String::from_utf8_lossy(&buf).to_string()
    }

    fn temp_config_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rkstratum_config_test_{}_{}.yaml",
            std::process::id(),
            nanos
        ))
    }

    #[tokio::test]
    async fn test_http_routing_and_config_write() {
        let config_path = temp_config_path();
        set_web_config_path(config_path.clone());
        std::fs::write(
            &config_path,
            r#"
kaspad_address: "127.0.0.1:16110"
stratum_port: ":5555"
min_share_diff: 8192
"#,
        )
        .unwrap();

        set_web_status_config("127.0.0.1:16110".to_string(), 2);

        let mode = HttpMode::Instance {
            instance_id: "0".to_string(),
            web_bind: "127.0.0.1:0".to_string(),
        };

        let status_resp = send_request(mode.clone(), "GET /api/status HTTP/1.1\r\n\r\n").await;
        assert!(status_resp.contains("200 OK"));
        assert!(status_resp.contains("\"kaspad_address\""));
        assert!(status_resp.contains("\"instances\":2"));
        assert!(status_resp.contains("\"host_metrics_enabled\""));
        assert!(status_resp.contains("\"geoip_enabled\""));

        let stats_resp = send_request(mode.clone(), "GET /api/stats HTTP/1.1\r\n\r\n").await;
        assert!(stats_resp.contains("200 OK"));
        assert!(stats_resp.contains("application/json"));

        let config_resp = send_request(mode.clone(), "GET /api/config HTTP/1.1\r\n\r\n").await;
        assert!(config_resp.contains("200 OK"));
        assert!(config_resp.contains("\"kaspad_address\""));

        // SAFETY: test-only env change scoped to this process; no concurrent mutation expected.
        unsafe {
            std::env::set_var("RKSTRATUM_ALLOW_CONFIG_WRITE", "1");
        }
        let json_body =
            r#"{"kaspad_address":"127.0.0.2:16110","stratum_port":":5556","min_share_diff":4096}"#;
        let post_req = format!(
            "POST /api/config HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            json_body.len(),
            json_body
        );
        let post_resp = send_request(mode, &post_req).await;
        assert!(post_resp.contains("\"success\": true"));

        let saved = std::fs::read_to_string(&config_path).unwrap();
        assert!(!saved.contains("global:"));
        assert!(saved.contains("instances:"));
    }
}
