use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PACKAGE_NAME: &str = "graphite-desktop-platform-mac";
const APP_BIN_FEATURE: &str = "app";
const HELPER_BIN_FEATURE: &str = "helper";

const APP_ID: &str = "rs.graphite.GraphiteEditor";
const APP_NAME: &str = "Graphite Editor";
const HELPER_BASE_NAME: &str = "Graphite Editor Helper";
const HELPER_TYPES: &[Option<&str>] = &[None, Some("GPU"), Some("Renderer"), Some("Plugin"), Some("Alerts")];

const EXEC_PATH: &str = "Contents/MacOS";
const FRAMEWORKS_PATH: &str = "Contents/Frameworks";
const RESOURCES_PATH: &str = "Contents/Resources";
const FRAMEWORK: &str = "Chromium Embedded Framework.framework";

pub fn main() -> Result<(), Box<dyn Error>> {
	let mut profile = env!("CARGO_PROFILE");
	let profile_path = PathBuf::from(env!("CARGO_WORKSPACE_DIR")).join(format!("target/{profile}"));
	if profile == "debug" {
		profile = "dev";
	}

	let app_bin = build_bin(APP_BIN_FEATURE, profile, &profile_path)?;
	let helper_bin = build_bin(HELPER_BIN_FEATURE, profile, &profile_path)?;

	let app_dir = bundle(&profile_path, &app_bin, &helper_bin);

	// TODO: Consider adding more useful cli
	if std::env::args().any(|a| a == "open") {
		let app_path = app_dir.to_string_lossy();
		run_command("open", &[&app_path]).expect("failed to open app")
	}

	Ok(())
}

fn build_bin(feature: &str, profile: &str, profile_path: &PathBuf) -> Result<PathBuf, Box<dyn Error>> {
	run_command("cargo", &["build", "--package", PACKAGE_NAME, "--profile", profile, "--no-default-features", "--features", feature])?;
	let bin_path = profile_path.join(format!("{PACKAGE_NAME}-{feature}"));
	fs::copy(profile_path.join(PACKAGE_NAME), &bin_path)?;
	Ok(bin_path)
}

fn bundle(out_dir: &Path, app_bin: &Path, helper_bin: &Path) -> PathBuf {
	let app_dir = create_app(out_dir, APP_ID, APP_NAME, app_bin, false);

	copy_cef(&app_dir);

	for &helper_type in HELPER_TYPES {
		let helper_id_suffix = helper_type.map(|t| format!(".{t}")).unwrap_or_default();
		let helper_id = format!("{APP_ID}.helper{helper_id_suffix}");
		let helper_name_suffix = helper_type.map(|t| format!(" ({t})")).unwrap_or_default();
		let helper_name = format!("{HELPER_BASE_NAME}{helper_name_suffix}");
		create_app(&app_dir.join(FRAMEWORKS_PATH), &helper_id, &helper_name, helper_bin, true);
	}

	app_dir
}

fn create_app(out_dir: &Path, id: &str, name: &str, bin: &Path, is_helper: bool) -> PathBuf {
	let bundle = out_dir.join(name).with_extension("app");
	fs::create_dir_all(&bundle.join(EXEC_PATH)).unwrap();

	let app_contents_dir: &Path = &bundle.join("Contents");
	for p in &[EXEC_PATH, RESOURCES_PATH, FRAMEWORKS_PATH] {
		fs::create_dir_all(app_contents_dir.join(p)).unwrap();
	}

	create_info_plist(&app_contents_dir, id, name, is_helper).unwrap();
	fs::copy(bin, bundle.join(EXEC_PATH).join(name)).unwrap();
	bundle
}

fn copy_cef(app_dir: &PathBuf) {
	let cef_src = PathBuf::from(std::env::var("CEF_PATH").expect("CEF_PATH needs to be set"));
	let dest: PathBuf = app_dir.join(FRAMEWORKS_PATH).join(FRAMEWORK);
	if dest.exists() {
		fs::remove_dir_all(&dest).unwrap();
	}
	copy_directory(&cef_src.join(FRAMEWORK), &dest);
}

fn copy_directory(src: &Path, dst: &Path) {
	fs::create_dir_all(dst).unwrap();
	for entry in fs::read_dir(src).unwrap() {
		let entry = entry.unwrap();
		let dst_path = dst.join(entry.file_name());
		if entry.file_type().unwrap().is_dir() {
			copy_directory(&entry.path(), &dst_path);
		} else {
			fs::copy(entry.path(), &dst_path).unwrap();
		}
	}
}

fn run_command(program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
	let status = Command::new(program).args(args).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
	if !status.success() {
		std::process::exit(1);
	}
	Ok(())
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
