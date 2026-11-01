//! Rewrites `.gdd` files through the migrations this build links.

use document_format::{ExportFormat, ExportOptions};
use std::error::Error;
use std::path::{Path, PathBuf};

/// `paths`, with each directory replaced by the `.gdd` files directly in it.
pub(crate) fn expand(paths: &[PathBuf]) -> Result<Vec<PathBuf>, Box<dyn Error>> {
	let mut documents = Vec::new();
	for path in paths {
		if !path.is_dir() {
			documents.push(path.clone());
			continue;
		}

		let mut found: Vec<PathBuf> = std::fs::read_dir(path)?
			.filter_map(Result::ok)
			.map(|entry| entry.path())
			.filter(|path| path.extension().is_some_and(|extension| extension == "gdd"))
			.collect();
		found.sort();
		documents.extend(found);
	}
	Ok(documents)
}

/// Open `path`, which migrates it, and save it back as a state-only xz archive if it needed migrating. Returns whether it did.
///
/// An archive's bytes aren't stable across a reopen, so the file is only rewritten when it changed. The embedded legacy `.graphite` is dropped, since it doesn't migrate and the editor would open it in place of the migrated state.
pub(crate) async fn upgrade(path: &Path) -> Result<bool, Box<dyn Error>> {
	let archive = std::fs::read(path)?;
	let gdd = graphene_cli::open_gdd(&archive).await?;
	if !gdd.migrated() {
		return Ok(false);
	}

	let options = ExportOptions {
		include_history: false,
		..Default::default()
	};
	let upgraded = gdd.export_to_bytes(ExportFormat::Xz, options, &gdd, None).await?;
	std::fs::write(path, upgraded)?;
	Ok(true)
}
