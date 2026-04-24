use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rusty_common::config::{AgentOsTarget, ProxyList, RustyConfig};
use std::io::Write;

fn write_temp(content: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

const FULL_CONFIG: &str = r#"
server:
  http_port: 8080
redis:
  url: "redis://localhost:6379"
  key_prefix: "rusty:"
ai:
  provider: openai
  api_key: "sk-bench-key"
  model: "gpt-4o"
  resolution:
    quality: 0.85
flux:
  url: "https://flux.example.com"
  token: "tok-bench"
  function_name: "rusty-agent"
  pending_timeout_secs: 10
api_keys:
  - "key1"
  - "key2"
  - "key3"
agent_env:
  LOG_LEVEL: "info"
  REGION: "us-east-1"
"#;

fn bench_config_load(c: &mut Criterion) {
    let f = write_temp(FULL_CONFIG);
    let path = f.path().to_str().unwrap().to_string();

    c.bench_function("config/load", |b| {
        b.iter(|| RustyConfig::load(black_box(&path)).unwrap());
    });
}

fn bench_config_load_missing_file(c: &mut Criterion) {
    c.bench_function("config/load_missing_file", |b| {
        b.iter(|| RustyConfig::load(black_box("/nonexistent/config.yaml")).unwrap_err());
    });
}

fn bench_proxy_list_get_for_geo(c: &mut Criterion) {
    let yaml = r#"
US:
  proxies:
    - "proxy1.us:3128"
    - "proxy2.us:3128"
    - "proxy3.us:3128"
GB:
  proxies:
    - "proxy1.gb:3128"
DE:
  proxies:
    - "proxy1.de:3128"
    - "proxy2.de:3128"
"#;
    let list: ProxyList = yaml_serde::from_str(yaml).unwrap();

    c.bench_function("config/proxy_list/get_for_geo_hit", |b| {
        b.iter(|| list.get_proxies_for_geo(black_box(Some("US"))));
    });
}

fn bench_proxy_list_get_for_geo_miss(c: &mut Criterion) {
    let yaml = r#"
US:
  proxies:
    - "proxy1.us:3128"
"#;
    let list: ProxyList = yaml_serde::from_str(yaml).unwrap();

    c.bench_function("config/proxy_list/get_for_geo_miss", |b| {
        b.iter(|| list.get_proxies_for_geo(black_box(Some("ZZ"))));
    });
}

fn bench_proxy_list_get_all(c: &mut Criterion) {
    let mut entries = String::new();
    for code in ["US", "GB", "DE", "FR", "JP", "AU", "CA", "BR"] {
        entries.push_str(&format!("{code}:\n  proxies:\n"));
        for i in 0..10 {
            entries.push_str(&format!("    - \"proxy{i}.{}:3128\"\n", code.to_lowercase()));
        }
    }
    let list: ProxyList = yaml_serde::from_str(&entries).unwrap();

    c.bench_function("config/proxy_list/get_all/80_proxies", |b| {
        b.iter(|| list.get_all());
    });
}

fn bench_agent_os_target_binary_name(c: &mut Criterion) {
    c.bench_function("config/agent_os_target/binary_name_linux", |b| {
        b.iter(|| AgentOsTarget::Linux.binary_name());
    });
    c.bench_function("config/agent_os_target/binary_name_windows", |b| {
        b.iter(|| AgentOsTarget::Windows.binary_name());
    });
}

criterion_group!(
    benches,
    bench_config_load,
    bench_config_load_missing_file,
    bench_proxy_list_get_for_geo,
    bench_proxy_list_get_for_geo_miss,
    bench_proxy_list_get_all,
    bench_agent_os_target_binary_name,
);
criterion_main!(benches);
