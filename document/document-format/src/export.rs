//! Export: walking the working copy through an archive codec, keeping payloads as-is.
//!
//! [`ExportFormat`] / [`ExportOptions`] are the public knobs; the [`Gdd`] export methods drive a
//! [`ExportSink`] (folder / zip / xz) through the manifest → registry → history → resources sequence.

#[cfg(not(target_family = "wasm"))]
use std::path::Path;

use document_container::archive::Archive;
use graphene_resource::{LoadResource, Resource, ResourceHash};

use crate::error::Error;
use crate::layout::Layout;
use crate::session_state::SessionState;
use crate::{Gdd, MANIFEST_CODEC, io};

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

impl<L: Layout> Gdd<L> {
	/// Stream the working copy to `dest` as a folder/zip/xz archive, keeping payload codecs as-is.
	/// Does not mutate `self` and does not buffer the export. Native-only (writes a filesystem path).
	///
	/// # Errors
	/// [`Error::InvalidExportOptions`] for incoherent options, [`Error::MissingResource`] if an
	/// embedded resource's bytes are absent from `byte_store`.
	#[cfg(not(target_family = "wasm"))]
	pub async fn export(&self, dest: &Path, format: ExportFormat, options: ExportOptions, byte_store: &dyn LoadResource) -> Result<(), Error> {
		options.validate().map_err(Error::InvalidExportOptions)?;

		match format {
			ExportFormat::Folder => {
				let mut folder = document_container::backends::folder::FolderBackend::create(dest)?;
				let mut sink = FolderSink { folder: &mut folder };
				self.stream_entries(options, byte_store, &mut sink).await?;
			}
			ExportFormat::Zip => {
				let file = std::fs::File::create(dest).map_err(document_container::ContainerError::Io)?;
				let mut writer = document_container::archive::Zip::writer(file)?;
				self.stream_entries(options, byte_store, &mut writer).await?;
				use document_container::archive::ArchiveWriter;
				writer.finish()?;
			}
			ExportFormat::Xz => {
				let file = std::fs::File::create(dest).map_err(document_container::ContainerError::Io)?;
				let mut writer = document_container::archive::Xz::writer(file)?;
				self.stream_entries(options, byte_store, &mut writer).await?;
				use document_container::archive::ArchiveWriter;
				writer.finish()?;
			}
		}

		Ok(())
	}

	/// In-memory variant of [`export`](Self::export) returning the archive bytes. Available on every
	/// target (no `std::fs`) but buffers the whole archive. `legacy_document` is embedded verbatim at
	/// [`Layout::legacy_basename`]. `ExportFormat::Folder` has no single-file form and is rejected.
	pub async fn export_to_bytes(&self, format: ExportFormat, options: ExportOptions, byte_store: &dyn LoadResource, legacy_document: Option<&[u8]>) -> Result<Vec<u8>, Error> {
		options.validate().map_err(Error::InvalidExportOptions)?;

		let cursor = std::io::Cursor::new(Vec::new());
		let buffer = match format {
			ExportFormat::Folder => return Err(Error::InvalidExportOptions("folder export has no single-file byte form")),
			ExportFormat::Zip => {
				let mut writer = document_container::archive::Zip::writer(cursor)?;
				self.stream_entries(options, byte_store, &mut writer).await?;
				if let Some(legacy) = legacy_document {
					ExportSink::write_entry(&mut writer, self.layout.legacy_basename(), legacy)?;
				}
				writer.finish_into()?
			}
			ExportFormat::Xz => {
				let mut writer = document_container::archive::Xz::writer(cursor)?;
				self.stream_entries(options, byte_store, &mut writer).await?;
				if let Some(legacy) = legacy_document {
					ExportSink::write_entry(&mut writer, self.layout.legacy_basename(), legacy)?;
				}
				writer.finish_into()?
			}
		};

		Ok(buffer.into_inner())
	}

	/// Drive a sink through manifest → session → registry → history → resources, one entry at a time,
	/// keeping each payload's recorded codec.
	async fn stream_entries(&self, options: ExportOptions, byte_store: &dyn LoadResource, sink: &mut dyn ExportSink) -> Result<(), Error> {
		use document_container::AsyncContainer;

		let codecs = self.manifest.codecs;
		sink.write_entry(&io::path_for(self.layout.manifest_basename(), MANIFEST_CODEC), &MANIFEST_CODEC.write_single(&self.manifest)?)?;

		// Carry the per-peer cursor + view settings so a `.gdd` reopened elsewhere restores the viewport.
		let session_state = SessionState {
			head_rev: self.session.head_rev(),
			redo_stack: self.session.redo_stack().to_vec(),
			next_node_counter: self.session.next_node_counter(),
			view_settings: self.view_settings.clone(),
			network_view_settings: self.network_view_settings.clone(),
		};
		sink.write_entry(&io::path_for(self.layout.session_basename(), codecs.session), &codecs.session.write_single(&session_state)?)?;

		let working_copy_hashes: std::collections::HashSet<ResourceHash> = self.resource_hashes().await?.into_iter().collect();

		// Resources to embed as bytes: every `Embedded` entry, plus link-only ones when
		// `embed_all_resources`. Bytes already in the working copy are written by the copy-through pass
		// below, so only the gap is loaded from the byte store here.
		let mut export_session = self.session.clone();
		let mut hashes_from_store: Vec<ResourceHash> = Vec::new();
		let mut links_to_promote: Vec<graph_storage::ResourceId> = Vec::new();
		for (id, entry) in &export_session.registry().resources {
			let Some(hash) = entry.hash else { continue };
			let embed = entry.has_embedded_source() || options.embed_all_resources;
			if !embed {
				continue;
			}
			if !entry.has_embedded_source() {
				links_to_promote.push(*id);
			}
			if !working_copy_hashes.contains(&hash) {
				hashes_from_store.push(hash);
			}
		}

		hashes_from_store.sort_unstable();
		hashes_from_store.dedup();

		// Fail fast if an embedded resource is missing, then promote link-only sources on the clone so
		// the exported registry and history stay consistent. The live `Gdd` is untouched.
		let mut embedded_bytes: Vec<(ResourceHash, Resource)> = Vec::new();
		for hash in hashes_from_store {
			let Some(resource) = byte_store.load(hash).await else {
				return Err(Error::MissingResource(hash));
			};
			embedded_bytes.push((hash, resource));
		}
		export_session.embed_resource_sources(links_to_promote)?;

		if options.include_registry {
			sink.write_entry(
				&io::path_for(self.layout.registry_basename(), codecs.registry),
				&codecs.registry.write_single(export_session.registry())?,
			)?;
		}

		if options.include_history {
			let mut buffer = Vec::new();
			for delta in export_session.history() {
				codecs.history.append(&mut buffer, delta)?;
			}
			if !buffer.is_empty() {
				sink.write_entry(&io::path_for(self.layout.history_basename(), codecs.history), &buffer)?;
			}
		}

		// Copy bytes the working copy already holds, tracking covered hashes so the embed pass below
		// doesn't re-emit them.
		let mut emitted = std::collections::HashSet::new();
		let resources_dir = self.layout.resources_dir();
		if self.working.list_dirs("").await?.iter().any(|d| d == resources_dir) {
			let prefix = format!("{resources_dir}/");
			for path in self.working.list(resources_dir).await? {
				if let Some(hash) = path.strip_prefix(&prefix).and_then(|name| name.parse::<ResourceHash>().ok()) {
					emitted.insert(hash);
				}
				let holder = self.working.read(&path).await?;

				// Native `External` (mmap'd) holders copy CoW via a source path; others write bytes.
				#[cfg(not(target_family = "wasm"))]
				match holder.source_path() {
					Some(src_path) => sink.write_entry_from_path(&path, src_path)?,
					None => sink.write_entry(&path, holder.as_slice())?,
				}
				#[cfg(target_family = "wasm")]
				sink.write_entry(&path, holder.as_slice())?;
			}
		}

		for (hash, resource) in &embedded_bytes {
			if emitted.insert(*hash) {
				sink.write_entry(&self.layout.resource_path(hash), resource.as_ref())?;
			}
		}

		Ok(())
	}
}

/// Sink an export streams entries into, so one async loop drives folder/zip/xz writes. Archive sinks
/// work on every target; the folder sink and `write_entry_from_path` are native-only. `Send` because
/// `stream_entries` holds `&mut dyn ExportSink` across `.await`s.
pub(crate) trait ExportSink: Send {
	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error>;

	/// Copy a file from disk into the sink. Default reads it into memory; the folder sink overrides to
	/// `fs::copy` (CoW). Native-only: only reachable for an `External` (mmap'd) holder.
	#[cfg(not(target_family = "wasm"))]
	fn write_entry_from_path(&mut self, path: &str, src: &std::path::Path) -> Result<(), Error> {
		let bytes = std::fs::read(src).map_err(document_container::ContainerError::Io)?;
		self.write_entry(path, &bytes)
	}
}

#[cfg(not(target_family = "wasm"))]
struct FolderSink<'a> {
	folder: &'a mut document_container::backends::folder::FolderBackend,
}

#[cfg(not(target_family = "wasm"))]
impl ExportSink for FolderSink<'_> {
	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error> {
		document_container::Container::write(self.folder, path, bytes)?;
		Ok(())
	}

	fn write_entry_from_path(&mut self, path: &str, src: &std::path::Path) -> Result<(), Error> {
		document_container::validate_path(path)?;
		let dest = self.folder.root().join(path);
		if let Some(parent) = dest.parent() {
			std::fs::create_dir_all(parent).map_err(document_container::ContainerError::Io)?;
		}
		std::fs::copy(src, &dest).map_err(document_container::ContainerError::Io)?;
		Ok(())
	}
}

impl<W: std::io::Write + std::io::Seek + Send> ExportSink for document_container::archive::ZipWriter<W> {
	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error> {
		use document_container::archive::ArchiveWriter;
		ArchiveWriter::write_entry(self, path, bytes)?;
		Ok(())
	}
}

impl<W: std::io::Write + std::io::Seek + Send> ExportSink for document_container::archive::XzWriter<W> {
	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error> {
		use document_container::archive::ArchiveWriter;
		ArchiveWriter::write_entry(self, path, bytes)?;
		Ok(())
	}
}
