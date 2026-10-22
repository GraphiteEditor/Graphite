mod common;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "windows")]
mod win;

// Avoid incorrect warnings about an unused dependency (this crate sets `DEP_CEF_DLL_WRAPPER_CEF_DIR` which is used in `build.rs`)
use cef_dll_sys as _;

fn main() {
	#[cfg(target_os = "linux")]
	linux::main().unwrap();
	#[cfg(target_os = "macos")]
	mac::main().unwrap();
	#[cfg(target_os = "windows")]
	win::main().unwrap();
}
