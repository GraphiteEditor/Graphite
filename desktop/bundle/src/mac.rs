use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::*;

const APP_ID: &str = "rs.graphite.GraphiteEditor";

const PACKAGE: &str = "graphite-desktop-platform-mac";
const HELPER_BIN: &str = "graphite-desktop-platform-mac-helper";

const EXEC_PATH: &str = "Contents/MacOS";
const FRAMEWORKS_PATH: &str = "Contents/Frameworks";
const RESOURCES_PATH: &str = "Contents/Resources";
const FRAMEWORK: &str = "Chromium Embedded Framework.framework";

pub fn main() -> Result<(), Box<dyn Error>> {
	let app_bin = build_bin(PACKAGE, None)?;
	let helper_bin = build_bin(PACKAGE, Some(HELPER_BIN))?;

	let profile_path = profile_path();
	let app_dir = bundle(&profile_path, &app_bin, &helper_bin);

	// TODO: Consider adding more useful cli
	if std::env::args().any(|a| a == "open") {
		let executable_path = app_dir.join(EXEC_PATH).join(APP_NAME);
		run_command(&executable_path.to_string_lossy(), &[]).expect("failed to open app");
	}

	Ok(())
}

fn bundle(out_dir: &Path, app_bin: &Path, helper_bin: &Path) -> PathBuf {
	let app_dir = out_dir.join(APP_NAME).with_extension("app");

	clean_dir(&app_dir);

	create_app(&app_dir, APP_ID, APP_NAME, app_bin, false);

	for helper_type in [None, Some("GPU"), Some("Renderer"), Some("Plugin"), Some("Alerts")] {
		let helper_id_suffix = helper_type.map(|t| format!(".{t}")).unwrap_or_default();
		let helper_id = format!("{APP_ID}.helper{helper_id_suffix}");
		let helper_name_suffix = helper_type.map(|t| format!(" ({t})")).unwrap_or_default();
		let helper_name = format!("{APP_NAME} Helper{helper_name_suffix}");
		let helper_app_dir = app_dir.join(FRAMEWORKS_PATH).join(&helper_name).with_extension("app");
		create_app(&helper_app_dir, &helper_id, &helper_name, helper_bin, true);
	}

	copy_dir(&cef_path().join(FRAMEWORK), &app_dir.join(FRAMEWORKS_PATH).join(FRAMEWORK));

	app_dir
}

fn create_app(app_dir: &Path, id: &str, name: &str, bin: &Path, is_helper: bool) {
	fs::create_dir_all(app_dir.join(EXEC_PATH)).unwrap();

	let app_contents_dir: &Path = &app_dir.join("Contents");
	for p in &[EXEC_PATH, RESOURCES_PATH, FRAMEWORKS_PATH] {
		fs::create_dir_all(app_contents_dir.join(p)).unwrap();
	}

	create_info_plist(app_contents_dir, id, name, is_helper).unwrap();
	fs::copy(bin, app_dir.join(EXEC_PATH).join(name)).unwrap();
}

fn create_info_plist(dir: &Path, id: &str, exec_name: &str, is_helper: bool) -> Result<(), Box<dyn std::error::Error>> {
	let info = InfoPlist {
		cf_bundle_development_region: "en".to_string(),
		cf_bundle_display_name: exec_name.to_string(),
		cf_bundle_executable: exec_name.to_string(),
		cf_bundle_identifier: id.to_string(),
		cf_bundle_info_dictionary_version: "6.0".to_string(),
		cf_bundle_name: exec_name.to_string(),
		cf_bundle_package_type: "APPL".to_string(),
		cf_bundle_signature: "????".to_string(),
		cf_bundle_version: "0.0.0".to_string(),
		cf_bundle_short_version_string: "0.0".to_string(),
		ls_environment: [("MallocNanoZone".to_string(), "0".to_string())].iter().cloned().collect(),
		ls_file_quarantine_enabled: true,
		ls_minimum_system_version: "11.0".to_string(),
		ls_ui_element: if is_helper { Some("1".to_string()) } else { None },
		ns_bluetooth_always_usage_description: exec_name.to_string(),
		ns_supports_automatic_graphics_switching: true,
		ns_web_browser_publickey_credential_usage_description: exec_name.to_string(),
		ns_camera_usage_description: exec_name.to_string(),
		ns_microphone_usage_description: exec_name.to_string(),
	};

	let plist_file = dir.join("Info.plist");
	plist::to_file_xml(plist_file, &info)?;
	Ok(())
}

#[derive(serde::Serialize)]
struct InfoPlist {
	#[serde(rename = "CFBundleDevelopmentRegion")]
	cf_bundle_development_region: String,
	#[serde(rename = "CFBundleDisplayName")]
	cf_bundle_display_name: String,
	#[serde(rename = "CFBundleExecutable")]
	cf_bundle_executable: String,
	#[serde(rename = "CFBundleIdentifier")]
	cf_bundle_identifier: String,
	#[serde(rename = "CFBundleInfoDictionaryVersion")]
	cf_bundle_info_dictionary_version: String,
	#[serde(rename = "CFBundleName")]
	cf_bundle_name: String,
	#[serde(rename = "CFBundlePackageType")]
	cf_bundle_package_type: String,
	#[serde(rename = "CFBundleSignature")]
	cf_bundle_signature: String,
	#[serde(rename = "CFBundleVersion")]
	cf_bundle_version: String,
	#[serde(rename = "CFBundleShortVersionString")]
	cf_bundle_short_version_string: String,
	#[serde(rename = "LSEnvironment")]
	ls_environment: HashMap<String, String>,
	#[serde(rename = "LSFileQuarantineEnabled")]
	ls_file_quarantine_enabled: bool,
	#[serde(rename = "LSMinimumSystemVersion")]
	ls_minimum_system_version: String,
	#[serde(rename = "LSUIElement")]
	ls_ui_element: Option<String>,
	#[serde(rename = "NSBluetoothAlwaysUsageDescription")]
	ns_bluetooth_always_usage_description: String,
	#[serde(rename = "NSSupportsAutomaticGraphicsSwitching")]
	ns_supports_automatic_graphics_switching: bool,
	#[serde(rename = "NSWebBrowserPublicKeyCredentialUsageDescription")]
	ns_web_browser_publickey_credential_usage_description: String,
	#[serde(rename = "NSCameraUsageDescription")]
	ns_camera_usage_description: String,
	#[serde(rename = "NSMicrophoneUsageDescription")]
	ns_microphone_usage_description: String,
}
