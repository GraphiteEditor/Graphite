//! The regression corpus: a directory, usually a git repository of its own, holding `.gdd` documents and the images they must render.
//!
//! ```text
//! corpus.json                       Which documents to render, how, and how closely they must match
//! documents/<name>.gdd
//! references/<name>/<time>s.png     One reference per animation time
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "corpus.json";

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
