use criterion::{Criterion, black_box, criterion_group, criterion_main};

// Import the type from the binary. Because rusty-agent is a [[bin]], we
// replicate only the serde-testable struct here via JSON round-trips.
// All benchmarks exercise serde_json parsing, which is the hot path in
// from_env() and the main cost during agent startup.

const FULL_JSON: &str = r#"{
    "driver_executable_path": "/usr/bin/chromedriver",
    "host": "localhost",
    "port": 9515,
    "driver_flags": ["--verbose", "--log-path=/tmp/driver.log"],
    "sandbox": true,
    "chrome_executable_path": "/usr/bin/google-chrome",
    "user_data_dir": "/tmp/chrome-profile",
    "browser_flags": ["--headless", "--no-sandbox", "--disable-gpu", "--window-size=1920,1080"]
}"#;

const MINIMAL_JSON: &str = r#"{"driver_flags": [], "browser_flags": []}"#;

#[derive(serde::Deserialize)]
struct ChromeBrowserLaunchConfig {
    #[allow(dead_code)]
    driver_executable_path: Option<String>,
    #[allow(dead_code)]
    host: Option<String>,
    #[allow(dead_code)]
    port: Option<u16>,
    driver_flags: Vec<String>,
    #[allow(dead_code)]
    sandbox: bool,
    #[allow(dead_code)]
    chrome_executable_path: Option<String>,
    #[allow(dead_code)]
    user_data_dir: Option<String>,
    browser_flags: Vec<String>,
}

fn bench_deserialize_full(c: &mut Criterion) {
    c.bench_function("browser_config/deserialize_full", |b| {
        b.iter(|| {
            let _: ChromeBrowserLaunchConfig =
                serde_json::from_str(black_box(FULL_JSON)).unwrap();
        });
    });
}

fn bench_deserialize_minimal(c: &mut Criterion) {
    c.bench_function("browser_config/deserialize_minimal", |b| {
        b.iter(|| {
            let _: ChromeBrowserLaunchConfig =
                serde_json::from_str(black_box(MINIMAL_JSON)).unwrap();
        });
    });
}

fn bench_deserialize_invalid(c: &mut Criterion) {
    let bad = r#"not valid json at all ]["#;
    c.bench_function("browser_config/deserialize_invalid", |b| {
        b.iter(|| {
            let result: Result<ChromeBrowserLaunchConfig, _> =
                serde_json::from_str(black_box(bad));
            let _ = result;
        });
    });
}

fn bench_serialize_round_trip(c: &mut Criterion) {
    let cfg: ChromeBrowserLaunchConfig = serde_json::from_str(FULL_JSON).unwrap();
    // Re-serialize the parsed flags (the only Vec fields that can be serialized via serde_json::to_string)
    c.bench_function("browser_config/vec_serialize", |b| {
        b.iter(|| {
            let _ = serde_json::to_string(black_box(&cfg.driver_flags)).unwrap();
            let _ = serde_json::to_string(black_box(&cfg.browser_flags)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_deserialize_full,
    bench_deserialize_minimal,
    bench_deserialize_invalid,
    bench_serialize_round_trip,
);
criterion_main!(benches);
