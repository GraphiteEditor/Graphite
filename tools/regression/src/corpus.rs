//! The regression corpus: a directory, usually a git repository of its own, holding `.gdd` documents and the images they must render.
//!
//! ```text
//! corpus.json                       Which documents to render, how, and how closely they must match
//! documents/<name>.gdd
//! references/<name>/<time>s.png     One reference per animation time
//! ingest/                           Documents waiting for `regression ingest` to add them
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "corpus.json";
pub const INGEST: &str = "ingest";

const README: &str = "# Graphite regression corpus

Documents and the images they must render, for `cargo run regression check` in the Graphite repository.

To add documents, put `.gdd` files in `ingest/` and run this from a Graphite checkout:

```sh
cargo run regression --corpus <this directory> ingest
```

Each one moves to `documents/` with new references under `references/`, and gets an entry in `corpus.json`. Commit the result. A document renders once at time zero unless a `<name>.json` next to it says otherwise, for example `{ \"times\": [0, 0.5, 1], \"scale\": 2 }`.

Old documents are the point of this repository, because they prove that migrations still reach them. Don't upgrade or replace a document here. When a change to how documents look is intended, run `bless` and commit the new references.
";

const GIT_ATTRIBUTES: &str = "*.png filter=lfs diff=lfs merge=lfs -text\n";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
	pub documents: Vec<Entry>,
}

/// One document and how it renders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
	pub name: String,
	/// Animation times in seconds, each rendered to its own reference.
	#[serde(default = "Entry::default_times")]
	pub times: Vec<f64>,
	#[serde(default = "Entry::default_scale")]
	pub scale: f64,
	/// Output size in pixels of a region at the document origin, instead of the artwork's bounds at `scale`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub size: Option<(u32, u32)>,
	#[serde(default)]
	pub tolerance: Tolerance,
}

/// How to render a document in the ingest folder, from an optional `<name>.json` beside it.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct IngestSettings {
	pub times: Vec<f64>,
	pub scale: f64,
	pub size: Option<(u32, u32)>,
}

impl Default for IngestSettings {
	fn default() -> Self {
		Self {
			times: Entry::default_times(),
			scale: Entry::default_scale(),
			size: None,
		}
	}
}

impl Entry {
	pub fn new(name: String, times: Vec<f64>, scale: f64, size: Option<(u32, u32)>) -> Self {
		Self {
			name,
			times,
			scale,
			size,
			tolerance: Tolerance::default(),
		}
	}

	fn default_times() -> Vec<f64> {
		vec![0.]
	}

	fn default_scale() -> f64 {
		1.
	}
}

/// How far a render may drift from its reference, since GPUs and drivers round differently.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Tolerance {
	/// The largest difference of one channel, out of 255, that still counts as the same pixel.
	pub channel: u8,
	/// The share of pixels that may differ by more than `channel`.
	pub pixels: f64,
}

impl Default for Tolerance {
	fn default() -> Self {
		Self { channel: 2, pixels: 0.001 }
	}
}

pub struct Corpus {
	pub root: PathBuf,
	pub manifest: Manifest,
}

impl Corpus {
	/// Create the corpus layout at `root`, keeping whatever is already there.
	pub fn init(root: &Path) -> Result<Self, String> {
		for directory in ["documents", "references", INGEST] {
			let path = root.join(directory);
			std::fs::create_dir_all(&path).map_err(|error| format!("{}: {error}", path.display()))?;
		}

		let keep_ingest = root.join(INGEST).join(".gitkeep");
		for (path, contents) in [(root.join("README.md"), README), (root.join(".gitattributes"), GIT_ATTRIBUTES), (keep_ingest, "")] {
			if !path.exists() {
				std::fs::write(&path, contents).map_err(|error| format!("{}: {error}", path.display()))?;
			}
		}

		let corpus = Self::open(root)?;
		corpus.save()?;
		Ok(corpus)
	}

	/// Open the corpus at `root`, starting an empty one if it has no manifest yet.
	pub fn open(root: &Path) -> Result<Self, String> {
		let manifest_path = root.join(MANIFEST);
		let manifest = match std::fs::read_to_string(&manifest_path) {
			Ok(text) => serde_json::from_str(&text).map_err(|error| format!("{}: {error}", manifest_path.display()))?,
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => Manifest::default(),
			Err(error) => return Err(format!("{}: {error}", manifest_path.display())),
		};
		Ok(Self { root: root.to_path_buf(), manifest })
	}

	pub fn save(&self) -> Result<(), String> {
		let manifest_path = self.root.join(MANIFEST);
		let text = serde_json::to_string_pretty(&self.manifest).map_err(|error| error.to_string())?;
		std::fs::write(&manifest_path, text + "\n").map_err(|error| format!("{}: {error}", manifest_path.display()))
	}

	pub fn document(&self, name: &str) -> PathBuf {
		self.root.join("documents").join(format!("{name}.gdd"))
	}

	pub fn reference(&self, name: &str, time: f64) -> PathBuf {
		self.root.join("references").join(name).join(format!("{time:.3}s.png"))
	}

	/// The entries `names` selects, or every entry when it's empty.
	pub fn select(&self, names: &[String]) -> Result<Vec<Entry>, String> {
		if names.is_empty() {
			return Ok(self.manifest.documents.clone());
		}

		names
			.iter()
			.map(|name| {
				let entry = self.manifest.documents.iter().find(|entry| &entry.name == name);
				entry.cloned().ok_or_else(|| format!("the corpus has no document `{name}`"))
			})
			.collect()
	}

	/// Add `entry`, replacing an entry of the same name.
	pub fn insert(&mut self, entry: Entry) {
		self.manifest.documents.retain(|existing| existing.name != entry.name);
		self.manifest.documents.push(entry);
		self.manifest.documents.sort_by(|a, b| a.name.cmp(&b.name));
	}
}
