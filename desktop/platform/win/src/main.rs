#![windows_subsystem = "windows"]

fn main() {
	#[cfg(target_os = "windows")]
	{
		use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
		let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
	}

	graphite_desktop::start();
}
