use std::env;
use std::error::Error;

use windows_registry::CURRENT_USER;

const PROG_ID: &str = "Graphite.Document";
const EXECUTABLE_NAME: &str = "Graphite.exe";
const APP_FRIENDLY_NAME: &str = "Graphite";
const DOCUMENT_FRIENDLY_NAME: &str = "Graphite Document";
const MIME_TYPE: &str = "application/graphite+json";
const FILE_EXTENSION: &str = ".graphite";
const SUPPORTED_EXTENSIONS: &[&str] = &[FILE_EXTENSION, ".svg", ".png", ".jpg", ".jpeg"];

pub fn register() {
	if let Err(e) = register_inner() {
		eprintln!("Failed to register file associations: {e}");
	}
}

fn register_inner() -> Result<(), Box<dyn Error>> {
	let exe = env::current_exe()?;
	let exe_string = exe.to_string_lossy();
	let open_command = format!("\"{exe_string}\" \"%1\"");
	let icon_value = format!("{exe_string},0");

	let prog_id = CURRENT_USER.create(format!("Software\\Classes\\{PROG_ID}"))?;
	prog_id.set_string("", DOCUMENT_FRIENDLY_NAME)?;
	let prog_id_icon = CURRENT_USER.create(format!("Software\\Classes\\{PROG_ID}\\DefaultIcon"))?;
	prog_id_icon.set_string("", &icon_value)?;
	let prog_id_command = CURRENT_USER.create(format!("Software\\Classes\\{PROG_ID}\\shell\\open\\command"))?;
	prog_id_command.set_string("", &open_command)?;

	let app_base = format!("Software\\Classes\\Applications\\{EXECUTABLE_NAME}");
	let app = CURRENT_USER.create(&app_base)?;
	app.set_string("FriendlyAppName", APP_FRIENDLY_NAME)?;
	let app_command = CURRENT_USER.create(format!("{app_base}\\shell\\open\\command"))?;
	app_command.set_string("", &open_command)?;
	let supported = CURRENT_USER.create(format!("{app_base}\\SupportedTypes"))?;
	for extension in SUPPORTED_EXTENSIONS {
		supported.set_string(extension, "")?;
	}

	let extension = CURRENT_USER.create(format!("Software\\Classes\\{FILE_EXTENSION}"))?;
	extension.set_string("", PROG_ID)?;
	extension.set_string("Content Type", MIME_TYPE)?;

	Ok(())
}
