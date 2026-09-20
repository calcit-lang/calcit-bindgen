use crate::{
    WASMTIME_HTTP_HOST_CARGO_FILE, WASMTIME_HTTP_HOST_CONFIG_EXAMPLE_FILE,
    WASMTIME_HTTP_HOST_MAIN_FILE, WASMTIME_HTTP_HOST_README_FILE,
};

const MAIN: &str = r#"#[tokio::main]
async fn main() {
    let code = calcit_bindgen::wasmtime_host::run_from_args_with_exit_code().await;
    if code != 0 {
        std::process::exit(code);
    }
}
"#;

const README: &str = r#"# Calcit buffered HTTP Wasmtime host

此目录由 `calcit-bindgen generate` 管理，不要直接修改。复制
`capabilities.example.cirru` 到生成目录之外，明确填写 Component、入口、严格类型的
Cirru EDN 参数、HTTP origin、宿主响应上限与必要 preopen，然后运行：

```bash
cargo run --manifest-path rust/wasmtime-http-host/Cargo.toml -- /path/to/capabilities.cirru
```

`allowed-origins` 与 `preopens` 缺省时均为空；没有显式授权就不能访问网络或宿主文件。
preopen 的 `:access` 只能是 `:read` 或 `:read-write`。`max-response-bytes` 是宿主上限，
会与 Calcit 请求里的上限取较小值。`arguments` 会在调用前按 Component function type
严格解码；结果以 Cirru EDN 写到 stdout，诊断只写 stderr。零参数 Unit 入口继续使用空列表。

需要文件化调用时，`:arguments-file` 从已授权 guest path 读取完整 Cirru EDN 参数列表，
`:result-file` 把 stdout 的规范结果同步写入 `:read-write` preopen。两者都不接受 host path、
越界或 symlink 逃逸。退出码固定为：成功 0、输入或 invalid-request 2、capability denied 3、
transport 4、response-too-large 5、unsupported 6、其他宿主失败 1。

Cargo 运行产生的 `Cargo.lock` 与 `target/` 不属于生成 manifest；`check` 会忽略它们，下一次
`generate` 会随整个受管目录一起重建。capability 配置与其他用户文件必须放在生成目录之外。
"#;

const CONFIG_EXAMPLE: &str = r#"{}
  :component |../../component/component.wasm
  :entry |run
  :arguments $ []
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
        let main = &files
            .iter()
            .find(|(path, _)| *path == WASMTIME_HTTP_HOST_MAIN_FILE)
            .expect("generated host main")
            .1;
        assert!(main.contains("run_from_args_with_exit_code"));
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
        assert!(config.contains(":arguments $ []"));
        assert!(config.contains(":component |../../component/component.wasm"));
    }
}
