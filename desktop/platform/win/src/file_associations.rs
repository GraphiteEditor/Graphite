use std::env;
use std::path::Path;

use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows_registry::CURRENT_USER;

const PROG_ID: &str = "Graphite.Document";
const EXECUTABLE_NAME: &str = "Graphite.exe";
const APP_FRIENDLY_NAME: &str = "Graphite";
const DOCUMENT_FRIENDLY_NAME: &str = "Graphite Document";
const MIME_TYPE: &str = "application/graphite+json";
const FILE_EXTENSION: &str = ".graphite";
const SUPPORTED_EXTENSIONS: &[&str] = &[FILE_EXTENSION, ".svg", ".png", ".jpg", ".jpeg"];

pub fn write() {
	if let Err(e) = FileAssociationWriter::new(&env::current_exe().unwrap())
		.document_type(PROG_ID, DOCUMENT_FRIENDLY_NAME)
		.application(EXECUTABLE_NAME, APP_FRIENDLY_NAME, SUPPORTED_EXTENSIONS)
		.extension(FILE_EXTENSION, PROG_ID, MIME_TYPE)
		.write()
	{
		eprintln!("Failed to register file associations: {e}");
	}
}

struct FileAssociationWriter {
	open_command: String,
	icon_value: String,
	entries: Vec<RegistryEntry>,
}

struct RegistryEntry {
	path: String,
	name: String,
	value: String,
}

impl FileAssociationWriter {
	fn new(executable: &Path) -> Self {
		let exe_string = executable.to_string_lossy();
		Self {
			open_command: format!("\"{exe_string}\" \"%1\""),
			icon_value: format!("{exe_string},0"),
			entries: Vec::new(),
		}
	}

	fn document_type(mut self, prog_id: &str, friendly_name: &str) -> Self {
		let base = format!("Software\\Classes\\{prog_id}");
		self.push(&base, "", friendly_name);
		self.push(&format!("{base}\\DefaultIcon"), "", &self.icon_value.clone());
		self.push(&format!("{base}\\shell\\open\\command"), "", &self.open_command.clone());
		self
	}

	fn application(mut self, executable_name: &str, friendly_name: &str, supported_extensions: &[&str]) -> Self {
		let base = format!("Software\\Classes\\Applications\\{executable_name}");
		self.push(&base, "FriendlyAppName", friendly_name);
		self.push(&format!("{base}\\shell\\open\\command"), "", &self.open_command.clone());

		let supported_path = format!("{base}\\SupportedTypes");
		for extension in supported_extensions {
			self.push(&supported_path, extension, "");
		}
		self
	}

	fn extension(mut self, extension: &str, prog_id: &str, mime_type: &str) -> Self {
		let path = format!("Software\\Classes\\{extension}");
		self.push(&path, "", prog_id);
		self.push(&path, "Content Type", mime_type);
		self
	}

	fn push(&mut self, path: &str, name: &str, value: &str) {
		self.entries.push(RegistryEntry {
			path: path.to_owned(),
			name: name.to_owned(),
			value: value.to_owned(),
		});
	}

	fn write(self) -> windows_registry::Result<()> {
		let mut changed = false;

		for entry in &self.entries {
			let key = CURRENT_USER.create(&entry.path)?;
			if key.get_string(&entry.name).ok().as_deref() == Some(entry.value.as_str()) {
				continue;
			}
			key.set_string(&entry.name, &entry.value)?;
			changed = true;
		}

		if changed {
			unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
		}
		Ok(())
	}
}
