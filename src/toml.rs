pub const CARGO_TOML_TEMPLATE: &str = r#"
[package]
name = "{PROJECT_NAME}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
lazy_static = "1.4.0"
{PROXY_DEPS}

[dependencies.windows-sys]
version = "0.61.2"
features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_Threading",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Memory",
    "Win32_System_Diagnostics_Debug",
    "Win32_System_SystemServices",
    "Win32_System_LibraryLoader",
    "Win32_UI_Shell",
]

[build-dependencies]
windres = "0.2.2"
"#;