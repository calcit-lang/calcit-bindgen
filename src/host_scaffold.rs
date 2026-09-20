use crate::{
    WASMTIME_HTTP_HOST_CARGO_FILE, WASMTIME_HTTP_HOST_CONFIG_EXAMPLE_FILE,
    WASMTIME_HTTP_HOST_MAIN_FILE, WASMTIME_HTTP_HOST_README_FILE,
};

const MAIN: &str = r#"#[tokio::main]
async fn main() {
    if let Err(error) = calcit_bindgen::wasmtime_host::run_from_args().await {
        eprintln!("calcit Wasmtime host failed: {error}");
        std::process::exit(1);
    }
}
"#;

const README: &str = r#"# Calcit buffered HTTP Wasmtime host

此目录由 `calcit-bindgen generate` 管理，不要直接修改。复制
`capabilities.example.cirru` 到生成目录之外，明确填写 Component、零参数 Unit 入口、
HTTP origin、宿主响应上限与必要 preopen，然后运行：

```bash
cargo run --manifest-path rust/wasmtime-http-host/Cargo.toml -- /path/to/capabilities.cirru
```

`allowed-origins` 与 `preopens` 缺省时均为空；没有显式授权就不能访问网络或宿主文件。
preopen 的 `:access` 只能是 `:read` 或 `:read-write`。`max-response-bytes` 是宿主上限，
会与 Calcit 请求里的上限取较小值。当前 host 只调用零参数、返回 Unit 的顶层 export，
避免引入动态参数 codec；更复杂调用应由业务专用宿主封装。
"#;

const CONFIG_EXAMPLE: &str = r#"{}
  :component |../../component/component.wasm
  :entry |run
  :max-response-bytes 1048576
  :allowed-origins $ []
  :preopens $ []
"#;

pub(crate) fn render() -> Vec<(&'static str, String)> {
    let cargo = format!(
        r#"[package]
name = "calcit-wasmtime-http-host"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
calcit-bindgen = {{ version = "={}", features = ["wasmtime-http"] }}
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
"#,
        env!("CARGO_PKG_VERSION")
    );
    vec![
        (WASMTIME_HTTP_HOST_CARGO_FILE, cargo),
        (WASMTIME_HTTP_HOST_MAIN_FILE, MAIN.to_owned()),
        (WASMTIME_HTTP_HOST_README_FILE, README.to_owned()),
        (
            WASMTIME_HTTP_HOST_CONFIG_EXAMPLE_FILE,
            CONFIG_EXAMPLE.to_owned(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_the_generator_version_and_keeps_capabilities_default_deny() {
        let files = render();
        let cargo = &files
            .iter()
            .find(|(path, _)| *path == WASMTIME_HTTP_HOST_CARGO_FILE)
            .expect("generated host Cargo.toml")
            .1;
        assert!(cargo.contains(&format!("version = \"={}\"", env!("CARGO_PKG_VERSION"))));
        let config = &files
            .iter()
            .find(|(path, _)| *path == WASMTIME_HTTP_HOST_CONFIG_EXAMPLE_FILE)
            .expect("generated host capability example")
            .1;
        assert!(config.contains(":allowed-origins $ []"));
        assert!(config.contains(":preopens $ []"));
        assert!(config.contains(":component |../../component/component.wasm"));
    }
}
