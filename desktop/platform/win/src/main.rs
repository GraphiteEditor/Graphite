#![windows_subsystem = "windows"]

#[cfg(target_os = "windows")]
mod file_associations;

fn main() {
	#[cfg(target_os = "windows")]
	file_associations::write();

	graphite_desktop::start();
}
