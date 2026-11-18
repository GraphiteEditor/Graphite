use std::fmt::Write;
use std::path::PathBuf;

const FOLDERS: [&str; 2] = ["16x16-nodes/", "16x16-ui-general/"];

fn main() {
	let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
	let icons_ts = root.join("frontend/src/icons.ts");
	let assets = root.join("branding/assets");
	println!("cargo:rerun-if-changed={}", icons_ts.display());

	let mut sources = Vec::new();
	if assets.is_dir() {
		println!("cargo:rerun-if-changed={}", assets.display());
		let icons_ts = std::fs::read_to_string(&icons_ts).unwrap();
		for line in icons_ts.lines() {
			let Some((name, path)) = line.strip_prefix("import ").and_then(|rest| rest.split_once(" from \"/../branding/assets/")) else {
				continue;
			};
			let path = path.trim_end_matches("\";");
			if FOLDERS.iter().any(|folder| path.starts_with(folder)) {
				sources.push((name.to_string(), assets.join(path)));
			}
		}
	} else {
		println!("cargo:rerun-if-changed={}", root.join(".branding").display());
		println!("cargo:warning=No branding/ folder: the graph UI draws no icons until it is there (`cargo run` downloads it)");
	}
	sources.sort();

	let mut table = String::from("static SOURCES: &[(&str, &str)] = &[\n");
	for (name, path) in &sources {
		writeln!(table, "\t({name:?}, include_str!({:?})),", path.display().to_string()).unwrap();
	}
	table.push_str("];\n");
	std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("icons.rs"), table).unwrap();
}
