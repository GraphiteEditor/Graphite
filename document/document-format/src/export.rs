//! Export options. Walking the working copy through an archive codec, keeping payloads as-is.
//! Implementation lives on [`crate::Gdd::export`].

/// Export wrapping. Payloads keep the working copy's recorded per-payload codecs (see
/// [`crate::manifest::PayloadCodecs`]); export does not re-encode.
#[derive(Copy, Clone, Debug)]
pub enum ExportFormat {
	/// Copy the working copy to a destination folder.
	Folder,
	/// Wrap as a `.gdd.zip` archive (deflate, pure-Rust `zip` crate).
	Zip,
	/// Wrap as a `.gdd.xz` archive (whole-archive xz via `lzma-rust2`).
	Xz,
}

#[derive(Copy, Clone, Debug)]
pub struct ExportOptions {
	/// Whether to include the registry snapshot. `false` produces a history-only export, useful
	/// for VCS workflows where the diffable `history.jsonl` is the interesting payload and the
	/// registry would rewrite whole-file on every retirement. Consumers replay history from an
	/// empty registry.
	pub include_registry: bool,
	/// Whether to include `history.jsonl`. `false` produces a flat snapshot (registry only),
	/// useful for sharing without revealing edit history and for cutting file size.
	pub include_history: bool,
	/// Materialize every `DataSource::FilePath` resource into `resources/<hash>` for portability.
	/// Does not mutate the in-memory `Gdd`.
	pub embed_all_resources: bool,
}

impl ExportOptions {
	/// Returns an error description if the combination is incoherent.
	pub fn validate(&self) -> Result<(), &'static str> {
		if !self.include_registry && !self.include_history {
			return Err("export must include at least one of: registry, history");
		}
		Ok(())
	}
}

impl Default for ExportOptions {
	fn default() -> Self {
		Self {
			include_registry: true,
			include_history: true,
			embed_all_resources: false,
		}
	}
}
