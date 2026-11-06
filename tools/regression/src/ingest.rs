//! Adds the documents waiting in the corpus's ingest folder.

use crate::add_document;
use crate::corpus::{Corpus, Entry, INGEST, IngestSettings};
use std::error::Error;
use std::path::{Path, PathBuf};

/// Whether every waiting document was added. One that fails stays in the folder, and the others still go through.
pub(crate) async fn ingest(corpus: &mut Corpus) -> Result<bool, Box<dyn Error>> {
	let folder = corpus.root.join(INGEST);
	let mut waiting: Vec<PathBuf> = std::fs::read_dir(&folder)
		.map_err(|error| format!("{}: {error}", folder.display()))?
		.filter_map(Result::ok)
		.map(|entry| entry.path())
		.collect();
	waiting.sort();

	let mut all_added = true;
	let mut added = 0;
	for path in waiting {
		match path.extension().and_then(|extension| extension.to_str()) {
			Some("gdd") => {}
			// Settings go with their document, and the keep file holds the empty folder in git
			Some("json") | None => continue,
			Some(_) => {
				println!("Skipped {}: only .gdd documents can be added. Open it in the editor and save it as a .gdd", path.display());
				all_added = false;
				continue;
			}
		}

		match add(corpus, &path).await {
			Ok(name) => {
				println!("Added {name}");
				added += 1;
			}
			Err(error) => {
				println!("FAIL {}: {error}", path.display());
				all_added = false;
			}
		}
	}

	println!("Added {added} documents. Commit the result");
	Ok(all_added)
}

async fn add(corpus: &mut Corpus, path: &Path) -> Result<String, Box<dyn Error>> {
	let name = path.file_stem().and_then(|stem| stem.to_str()).ok_or("the document has no file name")?.to_string();
	let settings_path = path.with_extension("json");
	let settings: IngestSettings = match std::fs::read_to_string(&settings_path) {
		Ok(text) => serde_json::from_str(&text).map_err(|error| format!("{}: {error}", settings_path.display()))?,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => IngestSettings::default(),
		Err(error) => return Err(format!("{}: {error}", settings_path.display()).into()),
	};
	add_document(corpus, path, Entry::new(name.clone(), settings.times, settings.scale, settings.size)).await?;

	std::fs::remove_file(path)?;
	if settings_path.exists() {
		std::fs::remove_file(settings_path)?;
	}
	Ok(name)
}
