//! Windows file-type registration. Writes per-user (`HKCU`) entries so no admin elevation is required.
//! The list of registry values we want to have set is described within [`registration_entries`].
//! Each launch re-reads them all and only re-writes when at least one differs from the desired state.

use crate::consts::{APP_DESCRIPTION, APP_NAME, DOCUMENT_FRIENDLY_NAME, DOCUMENT_MIME_TYPE, DOCUMENT_TYPE_IDENTIFIER};
use crate::wrapper::FILE_EXTENSION;
use std::io;
use std::path::{Path, PathBuf};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

/// Defensive fallback if `current_exe()` somehow returns a path with no filename component.
const DEFAULT_EXE_FILENAME: &str = "Graphite.exe";

/// Extensions Graphite claims as the primary handler. Double-clicking these in Explorer launches
/// Graphite (subject to the user's prior `UserChoice` association, which always wins).
/// Stored without the leading dot; [`registration_entries`] prepends it where Windows expects one.
const OWNED_EXTENSIONS: &[&str] = &[FILE_EXTENSION];

/// Extensions where Graphite appears in "Open with..." but does not displace any existing default handler.
/// Stored without the leading dot; [`registration_entries`] prepends it where Windows expects one.
const OPEN_WITH_EXTENSIONS: &[&str] = &["svg", "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp"];

pub(super) fn register() {
	let exe_path = match std::env::current_exe() {
		Ok(path) => {
			// `current_exe()` may return a path with a `\\?\` verbatim prefix.
			// That prefix is technically valid in registry command strings but Explorer occasionally mishandles it, so strip it.
			let path: &Path = &path;
			let lossy = path.to_string_lossy();
			if let Some(stripped) = lossy.strip_prefix(r"\\?\") {
				PathBuf::from(stripped)
			} else {
				path.to_path_buf()
			}
		}
		Err(error) => {
			tracing::error!("Failed to determine current executable path for OS file registration: {error}");
			return;
		}
	};

	let exe_string = exe_path.to_string_lossy().into_owned();
	let exe_filename = exe_path.file_name().and_then(|name| name.to_str()).unwrap_or(DEFAULT_EXE_FILENAME).to_owned();

	let command = format!("\"{exe_string}\" \"%1\"");
	let icon = format!("\"{exe_string}\",0");

	let entries = registration_entries(&exe_filename, &command, &icon);

	if registration_is_current(&entries) {
		return;
	}

	if let Err(error) = write_associations(&entries) {
		tracing::error!("Failed to register Windows file associations: {error}");
		return;
	}

	// Tell Explorer to refresh its association/icon caches so the new mapping is visible immediately without requiring a sign-out.
	// This is what causes desktop icons to briefly flicker, which is why we gate it behind the `registration_is_current` check above.
	unsafe {
		SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
	}

	tracing::info!("Registered Windows file associations for {exe_string}");
}

struct RegistryEntry {
	subkey: String,
	/// An empty string refers to the key's default (unnamed) value.
	value_name: String,
	value_data: String,
}

fn registration_entries(exe_filename: &str, command: &str, icon: &str) -> Vec<RegistryEntry> {
	let app_path = format!(r"Software\Classes\Applications\{exe_filename}");
	let app_command_path = format!(r"{app_path}\shell\open\command");
	let app_supported_path = format!(r"{app_path}\SupportedTypes");
	let progid_path = format!(r"Software\Classes\{DOCUMENT_TYPE_IDENTIFIER}");
	let progid_icon_path = format!(r"{progid_path}\DefaultIcon");
	let progid_command_path = format!(r"{progid_path}\shell\open\command");
	let capabilities_path = format!(r"Software\{APP_NAME}\Capabilities");
	let file_associations_path = format!(r"{capabilities_path}\FileAssociations");
	let mime_associations_path = format!(r"{capabilities_path}\MimeAssociations");

	let mut entries: Vec<RegistryEntry> = Vec::new();
	let mut push = |subkey: &str, value_name: &str, value_data: &str| {
		entries.push(RegistryEntry {
			subkey: subkey.to_owned(),
			value_name: value_name.to_owned(),
			value_data: value_data.to_owned(),
		})
	};

	// 1. The Application entry:
	// What Windows uses to populate "Open with..." menus and `Applications\<exe>` lookups.
	// Keyed by exe filename, so re-launching from a new directory writes a new command line under that key.
	push(&app_path, "FriendlyAppName", APP_NAME);
	push(&app_command_path, "", command);
	for extension in OWNED_EXTENSIONS.iter().chain(OPEN_WITH_EXTENSIONS) {
		push(&app_supported_path, &format!(".{extension}"), "");
	}

	// 2. The ProgID describing a Graphite document:
	// Referenced by every extension below.
	push(&progid_path, "", DOCUMENT_FRIENDLY_NAME);
	push(&progid_path, "FriendlyTypeName", DOCUMENT_FRIENDLY_NAME);
	push(&progid_icon_path, "", icon);
	push(&progid_command_path, "", command);

	// 3. Owned extensions:
	// Bind to the ProgID as a default fallback, attach MIME and perceived type metadata, and surface in "Open with..." via OpenWithProgids.
	// Setting the default does not override an existing `UserChoice` that the user may have made via the OS UI.
	for extension in OWNED_EXTENSIONS {
		let extension_path = format!(r"Software\Classes\.{extension}");
		let extension_open_with_path = format!(r"{extension_path}\OpenWithProgids");
		push(&extension_path, "", DOCUMENT_TYPE_IDENTIFIER);
		push(&extension_path, "Content Type", DOCUMENT_MIME_TYPE);
		push(&extension_path, "PerceivedType", "document");
		push(&extension_open_with_path, DOCUMENT_TYPE_IDENTIFIER, "");
	}

	// 4. Alternate extensions:
	// Only add the ProgID to OpenWithProgids so Graphite shows up under "Open with..." without claiming to be the default handler.
	for extension in OPEN_WITH_EXTENSIONS {
		let extension_open_with_path = format!(r"Software\Classes\.{extension}\OpenWithProgids");
		push(&extension_open_with_path, DOCUMENT_TYPE_IDENTIFIER, "");
	}

	// 5. Capabilities + RegisteredApplications:
	// The modern (Win8+) scheme. Required for Graphite to appear in "Settings" > "Default apps" > "Choose defaults by app",
	// and to be treated as a known app rather than a fresh suggestion in the "Open With" dialog.
	push(&capabilities_path, "ApplicationName", APP_NAME);
	push(&capabilities_path, "ApplicationDescription", APP_DESCRIPTION);
	push(&capabilities_path, "ApplicationIcon", icon);
	for extension in OWNED_EXTENSIONS.iter().chain(OPEN_WITH_EXTENSIONS) {
		push(&file_associations_path, &format!(".{extension}"), DOCUMENT_TYPE_IDENTIFIER);
	}
	push(&mime_associations_path, DOCUMENT_MIME_TYPE, DOCUMENT_TYPE_IDENTIFIER);
	push(r"Software\RegisteredApplications", APP_NAME, &capabilities_path);

	entries
}

/// Returns `true` when every registered entry already matches what we'd write.
fn registration_is_current(entries: &[RegistryEntry]) -> bool {
	let hkcu = RegKey::predef(HKEY_CURRENT_USER);
	entries.iter().all(|entry| {
		hkcu.open_subkey(&entry.subkey)
			.ok()
			.and_then(|key| key.get_value::<String, _>(&entry.value_name).ok())
			.is_some_and(|existing| existing == entry.value_data)
	})
}

fn write_associations(entries: &[RegistryEntry]) -> io::Result<()> {
	let hkcu = RegKey::predef(HKEY_CURRENT_USER);
	for entry in entries {
		let (key, _) = hkcu.create_subkey(&entry.subkey)?;
		key.set_value(&entry.value_name, &entry.value_data)?;
	}
	Ok(())
}
