use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use cab::{CabinetBuilder, CompressionType};
use msi::{Column, Insert, Package, PackageType, Value};

use crate::common::*;

const EXECUTABLE: &str = "Graphite.exe";
const PRODUCT_VERSION: &str = "0.0.0";
const MANUFACTURER: &str = "Graphite Labs, LLC";
const UPGRADE_CODE: &str = "{8D42B6D4-B0E8-41BC-9F41-74F1E319BC1D}";

const PROG_ID: &str = "Graphite.Document";
const DOCUMENT_FRIENDLY_NAME: &str = "Graphite Document";
const FILE_EXTENSION: &str = ".graphite";
const MIME_TYPE: &str = "application/graphite+json";
const EXTRA_EXTENSIONS: &[&str] = &[".svg", ".png", ".jpg", ".jpeg"];

const CABINET_STREAM: &str = "Data1.cab";

pub fn main() -> Result<(), Box<dyn Error>> {
	let app_bin = build_bin("graphite-desktop-platform-win", None)?;
	let profile_path = profile_path();
	let bundle_dir = bundle(&profile_path, &app_bin);
	let msi_path = create_msi(&bundle_dir, &profile_path)?;

	// TODO: Consider adding more useful cli
	let args: Vec<String> = std::env::args().collect();
	if let Some(pos) = args.iter().position(|a| a == "open") {
		let executable = bundle_dir.join(EXECUTABLE);
		let extra_args: Vec<&str> = args[pos + 1..].iter().map(|s| s.as_str()).collect();
		run_command(&executable.to_string_lossy(), &extra_args)?;
	} else {
		println!("MSI created at: {}", msi_path.display());
	}
	Ok(())
}

fn bundle(out_dir: &Path, app_bin: &Path) -> PathBuf {
	let app_dir = out_dir.join(APP_NAME);
	clean_dir(&app_dir);
	copy_dir(&cef_path(), &app_dir);
	if let Err(e) = remove_unnecessary_cef_files(&app_dir) {
		eprintln!("Failed to remove unnecessary CEF files: {}", e);
	}
	fs::copy(app_bin, app_dir.join(EXECUTABLE)).unwrap();
	app_dir
}

fn remove_unnecessary_cef_files(app_dir: &Path) -> Result<(), Box<dyn Error>> {
	fs::remove_dir_all(app_dir.join("cmake"))?;
	fs::remove_dir_all(app_dir.join("include"))?;
	fs::remove_dir_all(app_dir.join("libcef_dll"))?;
	for entry in fs::read_dir(app_dir.join("locales"))? {
		let path = entry?.path();
		if path.is_file() && path.file_name() != Some("en-US.pak".as_ref()) {
			fs::remove_file(path)?;
		}
	}
	fs::remove_file(app_dir.join("archive.json"))?;
	fs::remove_file(app_dir.join("CMakeLists.txt"))?;
	fs::remove_file(app_dir.join("bootstrapc.exe"))?;
	fs::remove_file(app_dir.join("bootstrap.exe"))?;
	fs::remove_file(app_dir.join("libcef.lib"))?;
	fs::remove_file(app_dir.join("CREDITS.html"))?;
	Ok(())
}

struct AppFile {
	abs_path: PathBuf,
	file_id: String,
	msi_filename: String,
	component_id: String,
	sequence: u32,
	size: u64,
}

struct AppDirectory {
	dir_id: String,
	parent_id: Option<String>,
	default_dir: String,
}

struct AppComponent {
	component_id: String,
	component_guid: String,
	dir_id: String,
	key_file_id: String,
}

struct Entries {
	files: Vec<AppFile>,
	directories: Vec<AppDirectory>,
	components: Vec<AppComponent>,
}

fn product_code() -> String {
	let commit = std::process::Command::new("git")
		.args(["rev-parse", "HEAD"])
		.output()
		.ok()
		.and_then(|o| String::from_utf8(o.stdout).ok())
		.map(|s| s.trim().to_ascii_uppercase())
		.filter(|s| s.len() >= 32)
		.unwrap_or_else(|| "0".repeat(32));
	format!("{{{}-{}-{}-{}-{}}}", &commit[..8], &commit[8..12], &commit[12..16], &commit[16..20], &commit[20..32],)
}

fn fnv1a(s: &str) -> u64 {
	s.bytes().fold(14695981039346656037u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211))
}

fn component_guid(seed: &str) -> String {
	let h1 = fnv1a(seed);
	let h2 = fnv1a(&format!("{seed}_2"));
	format!(
		"{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}",
		(h1 >> 32) as u32,
		((h1 >> 16) & 0xFFFF) as u16,
		(h1 & 0xFFFF) as u16,
		((h2 >> 48) & 0xFFFF) as u16,
		h2 & 0x0000_FFFF_FFFF_FFFF,
	)
}

fn msi_filename(name: &str) -> String {
	let dot = name.rfind('.');
	let (base, ext) = dot.map(|i| (&name[..i], &name[i + 1..])).unwrap_or((name, ""));
	if base.len() <= 8 && ext.len() <= 3 {
		return name.to_string();
	}
	let short_base: String = base.chars().filter(|c| c.is_ascii_alphanumeric()).take(6).map(|c| c.to_ascii_uppercase()).collect();
	let short_ext: String = ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(3).map(|c| c.to_ascii_uppercase()).collect();
	if short_ext.is_empty() {
		format!("{short_base}~1|{name}")
	} else {
		format!("{short_base}~1.{short_ext}|{name}")
	}
}

fn collect_entries(bundle_dir: &Path) -> Result<Entries, Box<dyn Error>> {
	let mut raw_files: Vec<(PathBuf, PathBuf)> = Vec::new();
	gather_files(bundle_dir, bundle_dir, &mut raw_files)?;

	let mut all_dir_strs: BTreeSet<String> = BTreeSet::new();
	all_dir_strs.insert(String::new());
	for (_, rel) in &raw_files {
		let mut path = rel.parent().unwrap_or(Path::new("")).to_owned();
		loop {
			let s = path.to_string_lossy().replace('\\', "/");
			if s.is_empty() {
				break;
			}
			all_dir_strs.insert(s);
			match path.parent().filter(|p| !p.as_os_str().is_empty()) {
				Some(p) => path = p.to_owned(),
				None => break,
			}
		}
	}

	let mut dir_to_ids: BTreeMap<String, (String, String)> = BTreeMap::new();
	for dir_str in &all_dir_strs {
		let h = fnv1a(dir_str);
		let dir_id = if dir_str.is_empty() { "INSTALLDIR".to_string() } else { format!("DIR_{h:016X}") };
		let comp_id = format!("COMP_{h:016X}");
		dir_to_ids.insert(dir_str.clone(), (dir_id, comp_id));
	}

	let mut directories = vec![
		AppDirectory {
			dir_id: "TARGETDIR".into(),
			parent_id: None,
			default_dir: "SourceDir".into(),
		},
		AppDirectory {
			dir_id: "ProgramFiles64Folder".into(),
			parent_id: Some("TARGETDIR".into()),
			default_dir: ".".into(),
		},
		AppDirectory {
			dir_id: "INSTALLDIR".into(),
			parent_id: Some("ProgramFiles64Folder".into()),
			default_dir: APP_NAME.into(),
		},
	];
	let mut subdir_strs: Vec<&str> = all_dir_strs.iter().filter(|s| !s.is_empty()).map(String::as_str).collect();
	subdir_strs.sort_by_key(|s| s.len());
	for dir_str in subdir_strs {
		let (dir_id, _) = &dir_to_ids[dir_str];
		let path = Path::new(dir_str);
		let parent_str = path.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
		let parent_id = dir_to_ids.get(parent_str.as_str()).map(|(id, _)| id.clone()).unwrap_or_else(|| "INSTALLDIR".into());
		let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
		directories.push(AppDirectory {
			dir_id: dir_id.clone(),
			parent_id: Some(parent_id),
			default_dir: name,
		});
	}

	let mut comp_key_files: BTreeMap<String, String> = BTreeMap::new();
	let mut file_dir_strs: BTreeSet<String> = BTreeSet::new();
	let mut files = Vec::new();
	let mut seq = 1u32;

	for (abs_path, rel_path) in &raw_files {
		let dir_str = rel_path.parent().unwrap_or(Path::new("")).to_string_lossy().replace('\\', "/");
		let (dir_id, comp_id) = dir_to_ids[dir_str.as_str()].clone();
		file_dir_strs.insert(dir_str.clone());

		let h = fnv1a(&rel_path.to_string_lossy().replace('\\', "/"));
		let file_id = format!("FILE_{h:016X}");
		let filename = msi_filename(rel_path.file_name().unwrap_or_default().to_str().unwrap_or_default());
		let size = fs::metadata(abs_path)?.len();

		comp_key_files.entry(comp_id.clone()).or_insert_with(|| file_id.clone());
		files.push(AppFile {
			abs_path: abs_path.clone(),
			file_id,
			msi_filename: filename,
			component_id: comp_id,
			sequence: seq,
			size,
		});
		seq += 1;
	}

	let components: Vec<AppComponent> = file_dir_strs
		.iter()
		.filter_map(|dir_str| {
			let (dir_id, comp_id) = dir_to_ids.get(dir_str.as_str())?;
			let key_file_id = comp_key_files.get(comp_id)?.clone();
			Some(AppComponent {
				component_id: comp_id.clone(),
				component_guid: component_guid(dir_str),
				dir_id: dir_id.clone(),
				key_file_id,
			})
		})
		.collect();

	Ok(Entries { files, directories, components })
}

fn gather_files(base: &Path, dir: &Path, out: &mut Vec<(PathBuf, PathBuf)>) -> std::io::Result<()> {
	let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
	entries.sort_by_key(|e| e.file_name());
	for entry in entries {
		let path = entry.path();
		if path.is_dir() {
			gather_files(base, &path, out)?;
		} else {
			let rel = path.strip_prefix(base).unwrap().to_path_buf();
			out.push((path, rel));
		}
	}
	Ok(())
}

fn build_cabinet(files: &[AppFile]) -> Result<Vec<u8>, Box<dyn Error>> {
	let mut builder = CabinetBuilder::new();
	{
		let folder = builder.add_folder(CompressionType::MSZIP);
		for file in files {
			folder.add_file(file.file_id.as_str());
		}
	}
	let mut writer = builder.build(Cursor::new(Vec::new()))?;
	let mut iter = files.iter();
	while let Some(mut fw) = writer.next_file()? {
		let file = iter.next().expect("cabinet/file count mismatch");
		fw.write_all(&fs::read(&file.abs_path)?)?;
	}
	Ok(writer.finish()?.into_inner())
}

fn create_msi(bundle_dir: &Path, out_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
	let entries = collect_entries(bundle_dir)?;
	let cabinet_bytes = build_cabinet(&entries.files)?;

	let msi_path = out_dir.join("Graphite.msi");
	let mut pkg = Package::create(PackageType::Installer, fs::File::create(&msi_path)?)?;

	{
		let info = pkg.summary_info_mut();
		info.set_title("Graphite Installation Database");
		info.set_subject(APP_NAME);
		info.set_author(MANUFACTURER);
		info.set_creating_application("graphite-desktop-bundle");
		info.set_arch("x64");
	}

	define_tables(&mut pkg)?;
	populate_properties(&mut pkg)?;
	populate_directories(&mut pkg, &entries.directories)?;
	populate_components(&mut pkg, &entries.components)?;
	populate_features(&mut pkg)?;
	populate_feature_components(&mut pkg, &entries.components)?;
	populate_files(&mut pkg, &entries.files)?;
	populate_media(&mut pkg, entries.files.len() as u32)?;
	populate_registry(&mut pkg, &entries.components)?;
	populate_sequences(&mut pkg)?;

	{
		let mut stream = pkg.write_stream(CABINET_STREAM)?;
		stream.write_all(&cabinet_bytes)?;
	}

	pkg.flush()?;
	Ok(msi_path)
}

fn define_tables(pkg: &mut Package<fs::File>) -> Result<(), Box<dyn Error>> {
	pkg.create_table("Property", vec![Column::build("Property").primary_key().id_string(72), Column::build("Value").text_string(0)])?;
	pkg.create_table(
		"Directory",
		vec![
			Column::build("Directory").primary_key().id_string(72),
			Column::build("Directory_Parent").nullable().id_string(72),
			Column::build("DefaultDir").string(255),
		],
	)?;
	pkg.create_table(
		"Component",
		vec![
			Column::build("Component").primary_key().id_string(72),
			Column::build("ComponentId").nullable().string(38),
			Column::build("Directory_").id_string(72),
			Column::build("Attributes").int16(),
			Column::build("Condition").nullable().text_string(255),
			Column::build("KeyPath").nullable().id_string(72),
		],
	)?;
	pkg.create_table(
		"Feature",
		vec![
			Column::build("Feature").primary_key().id_string(38),
			Column::build("Feature_Parent").nullable().id_string(38),
			Column::build("Title").nullable().text_string(64),
			Column::build("Description").nullable().text_string(255),
			Column::build("Display").nullable().int16(),
			Column::build("Level").int16(),
			Column::build("Directory_").nullable().id_string(72),
			Column::build("Attributes").int16(),
		],
	)?;
	pkg.create_table(
		"FeatureComponents",
		vec![Column::build("Feature_").primary_key().id_string(38), Column::build("Component_").primary_key().id_string(72)],
	)?;
	pkg.create_table(
		"File",
		vec![
			Column::build("File").primary_key().id_string(72),
			Column::build("Component_").id_string(72),
			Column::build("FileName").string(255),
			Column::build("FileSize").int32(),
			Column::build("Version").nullable().string(72),
			Column::build("Language").nullable().string(20),
			Column::build("Attributes").nullable().int16(),
			Column::build("Sequence").int32(),
		],
	)?;
	pkg.create_table(
		"Media",
		vec![
			Column::build("DiskId").primary_key().int16(),
			Column::build("LastSequence").int32(),
			Column::build("DiskPrompt").nullable().text_string(64),
			Column::build("Cabinet").nullable().string(255),
			Column::build("VolumeLabel").nullable().string(32),
			Column::build("Source").nullable().string(72),
		],
	)?;
	pkg.create_table(
		"Registry",
		vec![
			Column::build("Registry").primary_key().id_string(72),
			Column::build("Root").int16(),
			Column::build("Key").formatted_string(255),
			Column::build("Name").nullable().formatted_string(255),
			Column::build("Value").nullable().formatted_string(0),
			Column::build("Component_").id_string(72),
		],
	)?;
	pkg.create_table(
		"InstallExecuteSequence",
		vec![
			Column::build("Action").primary_key().id_string(72),
			Column::build("Condition").nullable().text_string(255),
			Column::build("Sequence").int16(),
		],
	)?;
	pkg.create_table(
		"InstallUISequence",
		vec![
			Column::build("Action").primary_key().id_string(72),
			Column::build("Condition").nullable().text_string(255),
			Column::build("Sequence").int16(),
		],
	)?;
	Ok(())
}

fn populate_properties(pkg: &mut Package<fs::File>) -> Result<(), Box<dyn Error>> {
	pkg.insert_rows(Insert::into("Property").rows(vec![
		vec![Value::from("ProductName"), Value::from(APP_NAME)],
		vec![Value::from("ProductCode"), Value::from(product_code())],
		vec![Value::from("ProductVersion"), Value::from(PRODUCT_VERSION)],
		vec![Value::from("Manufacturer"), Value::from(MANUFACTURER)],
		vec![Value::from("UpgradeCode"), Value::from(UPGRADE_CODE)],
		vec![Value::from("INSTALLLEVEL"), Value::from("1")],
	]))?;
	Ok(())
}

fn populate_directories(pkg: &mut Package<fs::File>, dirs: &[AppDirectory]) -> Result<(), Box<dyn Error>> {
	let rows: Vec<Vec<Value>> = dirs
		.iter()
		.map(|d| {
			vec![
				Value::from(d.dir_id.as_str()),
				d.parent_id.as_deref().map(Value::from).unwrap_or(Value::Null),
				Value::from(d.default_dir.as_str()),
			]
		})
		.collect();
	pkg.insert_rows(Insert::into("Directory").rows(rows))?;
	Ok(())
}

fn populate_components(pkg: &mut Package<fs::File>, components: &[AppComponent]) -> Result<(), Box<dyn Error>> {
	let rows: Vec<Vec<Value>> = components
		.iter()
		.map(|c| {
			vec![
				Value::from(c.component_id.as_str()),
				Value::from(c.component_guid.as_str()),
				Value::from(c.dir_id.as_str()),
				Value::from(0i16),
				Value::Null,
				Value::from(c.key_file_id.as_str()),
			]
		})
		.collect();
	pkg.insert_rows(Insert::into("Component").rows(rows))?;
	Ok(())
}

fn populate_features(pkg: &mut Package<fs::File>) -> Result<(), Box<dyn Error>> {
	pkg.insert_rows(Insert::into("Feature").row(vec![
		Value::from("ProductFeature"),
		Value::Null,
		Value::from(APP_NAME),
		Value::from("Graphite Vector Editor"),
		Value::from(1i16),
		Value::from(1i16),
		Value::from("INSTALLDIR"),
		Value::from(0i16),
	]))?;
	Ok(())
}

fn populate_feature_components(pkg: &mut Package<fs::File>, components: &[AppComponent]) -> Result<(), Box<dyn Error>> {
	let rows: Vec<Vec<Value>> = components.iter().map(|c| vec![Value::from("ProductFeature"), Value::from(c.component_id.as_str())]).collect();
	pkg.insert_rows(Insert::into("FeatureComponents").rows(rows))?;
	Ok(())
}

fn populate_files(pkg: &mut Package<fs::File>, files: &[AppFile]) -> Result<(), Box<dyn Error>> {
	let rows: Vec<Vec<Value>> = files
		.iter()
		.map(|f| {
			vec![
				Value::from(f.file_id.as_str()),
				Value::from(f.component_id.as_str()),
				Value::from(f.msi_filename.as_str()),
				Value::Int(f.size as i32),
				Value::Null,
				Value::Null,
				Value::Null,
				Value::Int(f.sequence as i32),
			]
		})
		.collect();
	pkg.insert_rows(Insert::into("File").rows(rows))?;
	Ok(())
}

fn populate_media(pkg: &mut Package<fs::File>, last_sequence: u32) -> Result<(), Box<dyn Error>> {
	pkg.insert_rows(Insert::into("Media").row(vec![
		Value::from(1i16),
		Value::Int(last_sequence as i32),
		Value::Null,
		Value::from(format!("#{CABINET_STREAM}")),
		Value::Null,
		Value::Null,
	]))?;
	Ok(())
}

fn populate_registry(pkg: &mut Package<fs::File>, components: &[AppComponent]) -> Result<(), Box<dyn Error>> {
	let root_comp = components
		.iter()
		.find(|c| c.dir_id == "INSTALLDIR")
		.map(|c| c.component_id.as_str())
		.unwrap_or_else(|| components.first().map(|c| c.component_id.as_str()).unwrap_or(""));

	let exe = format!("[INSTALLDIR]{EXECUTABLE}");
	let open_cmd = format!("\"[INSTALLDIR]{EXECUTABLE}\" \"%1\"");

	let mut rows: Vec<Vec<Value>> = vec![
		reg("reg_progid", PROG_ID, None, Some(DOCUMENT_FRIENDLY_NAME), root_comp),
		reg("reg_progid_icon", &format!("{PROG_ID}\\DefaultIcon"), None, Some(&format!("{exe},0")), root_comp),
		reg("reg_progid_cmd", &format!("{PROG_ID}\\shell\\open\\command"), None, Some(&open_cmd), root_comp),
		reg("reg_ext", FILE_EXTENSION, None, Some(PROG_ID), root_comp),
		reg("reg_ext_mime", FILE_EXTENSION, Some("Content Type"), Some(MIME_TYPE), root_comp),
		reg("reg_app_name", &format!("Applications\\{EXECUTABLE}"), Some("FriendlyAppName"), Some(APP_NAME), root_comp),
		reg("reg_app_cmd", &format!("Applications\\{EXECUTABLE}\\shell\\open\\command"), None, Some(&open_cmd), root_comp),
		reg("reg_app_ext", &format!("Applications\\{EXECUTABLE}\\SupportedTypes"), Some(FILE_EXTENSION), Some(""), root_comp),
	];

	for (i, ext) in EXTRA_EXTENSIONS.iter().enumerate() {
		rows.push(reg(&format!("reg_app_ext_{i}"), &format!("Applications\\{EXECUTABLE}\\SupportedTypes"), Some(ext), Some(""), root_comp));
	}

	pkg.insert_rows(Insert::into("Registry").rows(rows))?;
	Ok(())
}

fn reg(id: &str, key: &str, name: Option<&str>, value: Option<&str>, comp: &str) -> Vec<Value> {
	vec![
		Value::from(id),
		Value::from(0i16),
		Value::from(key),
		name.map(Value::from).unwrap_or(Value::Null),
		value.map(Value::from).unwrap_or(Value::Null),
		Value::from(comp),
	]
}

fn populate_sequences(pkg: &mut Package<fs::File>) -> Result<(), Box<dyn Error>> {
	pkg.insert_rows(Insert::into("InstallExecuteSequence").rows(vec![
		seq("CostInitialize", 800),
		seq("FileCost", 900),
		seq("CostFinalize", 1000),
		seq("InstallValidate", 1400),
		seq("InstallInitialize", 1500),
		seq("ProcessComponents", 1600),
		seq("UnpublishComponents", 1700),
		seq("UnpublishFeatures", 1800),
		seq("RemoveRegistryValues", 2600),
		seq("RemoveFiles", 3500),
		seq("InstallFiles", 4000),
		seq("WriteRegistryValues", 5000),
		seq("RegisterUser", 6000),
		seq("RegisterProduct", 6100),
		seq("PublishComponents", 6200),
		seq("PublishFeatures", 6300),
		seq("PublishProduct", 6400),
		seq("InstallFinalize", 6600),
	]))?;
	pkg.insert_rows(Insert::into("InstallUISequence").rows(vec![seq("CostInitialize", 800), seq("FileCost", 900), seq("CostFinalize", 1000), seq("ExecuteAction", 1300)]))?;
	Ok(())
}

fn seq(action: &str, sequence: i16) -> Vec<Value> {
	vec![Value::from(action), Value::Null, Value::from(sequence)]
}
