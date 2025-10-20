mod common;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "windows")]
mod win;

fn main() {
	#[cfg(target_os = "macos")]
	mac::main().unwrap();
	#[cfg(target_os = "windows")]
	win::main().unwrap();
	#[cfg(target_os = "linux")]
	todo!("Linux bundling not implemented yet");
}
