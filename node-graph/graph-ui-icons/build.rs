// TODO(Timon): Replace with generalized icon crate

use std::fmt::Write;
use std::path::PathBuf;

const FOLDERS: [&str; 2] = ["16x16-nodes", "16x16-ui-general"];

fn main() {
	let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
	let assets = root.join("branding/assets");

	let mut sources = Vec::new();
	if assets.is_dir() {
		println!("cargo:rerun-if-changed={}", assets.display());
		for folder in FOLDERS {
			for entry in std::fs::read_dir(assets.join(folder)).unwrap() {
				let path = entry.unwrap().path();
				if path.extension().is_some_and(|extension| extension == "svg") {
					sources.push((icon_name(&path.file_stem().unwrap().to_string_lossy()), path));
				}
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

fn icon_name(file_stem: &str) -> String {
	let capitalized = |word: &str| word[..1].to_uppercase() + &word[1..];
	file_stem.split('-').filter(|word| !word.is_empty()).map(capitalized).collect()
}
