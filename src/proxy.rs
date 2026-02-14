pub const BUILD_RS_TEMPLATE: &str = r##"
use std::{env, path::PathBuf};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let def_path = PathBuf::from(manifest_dir).join("proxy.def");
        println!("cargo:rustc-link-arg=/DEF:{}", def_path.display());
        println!("cargo:rerun-if-changed=proxy.def");
    }
}
"##;

pub const LIB_RS_TEMPLATE: &str = r##"
#![allow(non_snake_case)]
extern crate lazy_static;

use std::sync::{Arc, Mutex};
use std::{ptr, thread};
use lazy_static::lazy_static;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxA, MB_OK};
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows_sys::core::BOOL;
use dyncvoke::dyncvoke_core::{nt_create_thread_ex,get_function_address, load_library_a};

mod forward;

const NATIVE: bool = true;
const DLL_NAME: &str = r#"{ORIGINAL_DLL_PATH}"#;

static mut CALLBACK_TABLE: [usize; {NUM_EXPORTS}] = [0; {NUM_EXPORTS}];
lazy_static! {
    static ref sync_lock: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllMain(
    _hinst: *mut std::ffi::c_void,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => 1,
        DLL_PROCESS_DETACH => 1,
        _ => 1,
    }
}

fn initialize_component() {
    unsafe {
        MessageBoxA(ptr::null_mut(), windows_sys::core::s!("Module initialized. ProxyMode Success"), windows_sys::core::s!("Status"), MB_OK);
    }
}

fn dispatch_call(
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
    a7: u64,
    a8: u64,
    a9: u64,
    a10: u64,
    a11: u64,
    a12: u64,
    a13: u64,
    a14: u64,
    a15: u64,
    a16: u64,
    a17: u64,
    a18: u64,
    a19: u64,
    a20: u64,
    export_id: u32,
) -> u64 {
    let lock = Arc::clone(&sync_lock);
    let mut guard = lock.lock().unwrap();

    if *guard == 0 {
        *guard += 1;
        if NATIVE {
            #[allow(unused_unsafe)]
            unsafe {
                let thread_handle: *mut HANDLE = ptr::null_mut();
                let func_ptr = initialize_component as *const();
                let start_addr = func_ptr as *mut std::ffi::c_void;

                let result = nt_create_thread_ex(
                    thread_handle,
                    0x1FFFFF,
                    ptr::null_mut(),
                    -1isize as *mut std::ffi::c_void,
                    start_addr,
                    ptr::null_mut(),
                    0, 0, 0, 0,
                    ptr::null_mut()
                );

                if result != 0 {
                    thread::spawn(|| {
                        initialize_component();
                    });
                }
            }
        } else {
            thread::spawn(|| {
                initialize_component();
            });
        }
    }
    drop(guard);

    unsafe {
        if CALLBACK_TABLE[export_id as usize] != 0 {
            let target: extern "system" fn(
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
                u64,
            ) -> u64 = std::mem::transmute(CALLBACK_TABLE[export_id as usize]);
            return target(
                a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, a17, a18,
                a19, a20,
            );
        }
    }

    let proc_name = match export_id {
        {MATCH_STATEMENT}
        _ => "".to_string(),
    };

    if proc_name.is_empty() {
        return 0;
    }

    let module_handle = load_library_a(DLL_NAME);
    if module_handle == 0 {
        return 0;
    }

    let proc_addr = get_function_address(module_handle, &proc_name);
    if proc_addr == 0 {
        return 0;
    }

    unsafe {
        CALLBACK_TABLE[export_id as usize] = proc_addr;
        let target: extern "system" fn(
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
            u64,
        ) -> u64 = std::mem::transmute(proc_addr);
        return target(
            a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, a17, a18, a19,
            a20,
        );
    }
}

{HIJACK_FUNCTION}
"##;
