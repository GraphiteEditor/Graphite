//! Typed handle for `.gdd` documents.
//!
//! [`Gdd`] owns a [`graph_storage::Session`] plus a working-copy [`document_container::AnyContainer`].
//! Mutations flow through `Gdd` to keep the session and the on-disk working copy mirrored.
//! Export is a separate, explicit operation — see [`export::ExportFormat`].
//!
//! See the "On-disk container" section of `node-graph/rfcs/document-format.md` for the format spec.

use std::sync::Arc;
// `Path`, `Archive`, and `FolderBackend` are only used by the native-only path-based open/create
// and filesystem export, so they're gated off wasm to avoid unused-import warnings.
#[cfg(not(target_family = "wasm"))]
use std::path::Path;

#[cfg(not(target_family = "wasm"))]
use document_container::archive::Archive;
#[cfg(not(target_family = "wasm"))]
use document_container::backends::folder::FolderBackend;
use document_container::{AnyContainer, AsyncContainer, ByteHolder, ContainerError};
use graph_storage::{CommitError, Delta, HotOp, NodeMetadataSource, PeerId, Registry, Rev, Session, TimeStamp};
use graphene_resource::ResourceFuture;
use graphene_resource::{LoadResource, Resource, ResourceHash, ResourceStorage};

pub mod codec;
pub mod error;
pub mod export;
pub mod io;
pub mod layout;
pub mod manifest;
pub mod session_state;

pub use codec::{Codec, CodecError};
pub use error::Error;
pub use export::{ExportFormat, ExportOptions};
pub use io::ReadError;
pub use layout::{GddV1, Layout};
pub use manifest::{Manifest, PayloadCodecs};
pub use session_state::SessionState;

/// The manifest is always JSON: it is the bootstrap file, read before any other payload's codec is
/// known, so its own codec cannot itself be configurable.
pub const MANIFEST_CODEC: Codec = Codec::Json;

/// Working-copy codecs. The working copy lives in appdata, not under VCS — these defaults
/// optimize for size and write cost. MessagePack is self-describing, so it round-trips the
/// type-erased `serde_json::Value` bodies that resource and attribute deltas carry (a non-self-
/// describing format like postcard cannot). JSON/JSONL is opt-in via `ExportFormat::Folder` for
/// users who want a diffable on-disk representation. Recorded in the manifest at create time and
/// read back on open (see [`manifest::PayloadCodecs`]), so the persist path never probes the filesystem.
pub const DEFAULT_SESSION_CODEC: Codec = Codec::Json;
pub const DEFAULT_REGISTRY_CODEC: Codec = Codec::MessagePack;
pub const DEFAULT_HISTORY_CODEC: Codec = Codec::MessagePackFrames;
pub const DEFAULT_HOT_LOG_CODEC: Codec = Codec::MessagePackFrames;

/// Editor-facing handle. Owns the `Session` and the working-copy container; mutations are mirrored
/// to disk continuously (every retirement appends to the history file and re-snapshots the registry).
///
/// The per-edit persist path (`commit_from_runtime`, `apply_hot_op`, `retire`) is synchronous and
/// read-free: the manifest is cached in memory (so payload codecs and `last_retired_at` need no
/// disk read), and writes go through the container's sync write surface. Only `open` / `create` /
/// `export` are async, since they read.
/// `Clone` shares the working-copy container (`Arc<AnyContainer>`) so a cloned handle reads and writes
/// the *same* on-disk/OPFS working copy — including any writes still queued on the OPFS backend. The
/// `Session` is cloned (a snapshot copy); the container is shared.
#[derive(Clone)]
pub struct Gdd<L: Layout = GddV1> {
	session: Session,
	working: Arc<AnyContainer>,
	layout: L,
	/// In-memory copy of the manifest, kept authoritative since `Gdd` is its sole writer. Holds the
	/// per-payload codecs (so the persist path never probes the filesystem) and `last_retired_at`
	/// (so retirement writes the manifest without first reading it). Lets the persist path stay
	/// fully read-free and synchronous.
	manifest: Manifest,
	/// Per-peer view settings (PTZ, rulers, etc.), persisted in `session.json` not the registry, so
	/// they stay out of the CRDT/history. Opaque to the storage layer; the editor owns the keys/values.
	view_settings: std::collections::HashMap<String, serde_json::Value>,
	/// Per-network view settings (node-graph nav + previewing), keyed by stable [`NetworkId`]. Same per-peer
	/// `session.json` treatment as [`view_settings`](Self::view_settings), but scoped per network.
	network_view_settings: std::collections::HashMap<graph_storage::NetworkId, std::collections::HashMap<String, serde_json::Value>>,
}

/// Native folder-backed convenience constructors. On wasm the editor builds an OPFS-backed
/// `AnyContainer` itself and uses [`Gdd::open_in`] / [`Gdd::create_in`] directly.
#[cfg(not(target_family = "wasm"))]
impl<L: Layout + Default> Gdd<L> {
	/// Open an existing working copy at `path`. Validates the manifest, materializes the session
	/// from `registry.bin` (fast path) or by replaying `history.jsonl` (slow path), then applies
	/// the persisted hot log on top.
	pub async fn open(path: &Path) -> Result<Self, Error> {
		let working = AnyContainer::Folder(FolderBackend::open(path)?);
		let layout = L::default();
		Self::open_in(working, layout).await
	}

	/// Create a fresh, empty working copy at `path` bound to `peer`. Writes a default manifest
	/// and session state; the caller fills in editor metadata via [`Gdd::update_manifest`].
	pub async fn create(path: &Path, peer: PeerId, document_uuid: u64, editor_version: String, stdlib_version: String) -> Result<Self, Error> {
		let working = AnyContainer::Folder(FolderBackend::create(path)?);
		let layout = L::default();
		Self::create_in(working, layout, peer, document_uuid, editor_version, stdlib_version).await
	}
}

impl<L: Layout> Gdd<L> {
	/// Open a `.gdd` from archive bytes (xz or zip, auto-detected) by materializing it into `working`,
	/// then opening it as a working copy. The archive is deserialized into an in-memory staging backend
	/// (the archive reader is synchronous), then each entry is written into `working` via the sync
	/// `write_non_blocking` surface — durable on folder/memory, eagerly enqueued on OPFS. `working` is
	/// expected to be a fresh per-document container; entries with colliding paths are overwritten.
	pub async fn open_from_archive(bytes: &[u8], working: AnyContainer, layout: L) -> Result<Self, Error> {
		use document_container::AsyncContainer;
		use document_container::Container;
		use document_container::backends::memory::MemoryBackend;

		let mut staging = MemoryBackend::new();
		document_container::archive::open_auto(bytes, &mut staging)?;

		// Copy every entry, recursing into subdirectories: `list` is single-level, so a flat top-level
		// copy would skip the `resources/<hash>` subtree (the document's resource + declaration bytes).
		let mut directories = vec![String::new()];
		while let Some(dir) = directories.pop() {
			for path in Container::list(&staging, &dir)? {
				let holder = Container::read(&staging, &path)?;
				working.write_non_blocking(&path, holder.as_slice())?;
			}
			directories.extend(Container::list_dirs(&staging, &dir)?);
		}

		Self::open_in(working, layout).await
	}

	/// Backend-agnostic open. Splits out so tests can supply a [`document_container::backends::memory::MemoryBackend`].
	///
	/// # Errors
	/// [`Error::WrongFormat`] / [`Error::UnsupportedVersion`] if the manifest fails validation, plus
	/// the usual [`Error::Read`] / [`Error::Codec`] / [`Error::Crdt`] if a payload is malformed.
	pub async fn open_in(working: AnyContainer, layout: L) -> Result<Self, Error> {
		let manifest: Manifest = io::read_single(&working, layout.manifest_basename(), MANIFEST_CODEC).await?;
		validate_manifest(&manifest)?;
		let codecs = manifest.codecs;

		let session_state: SessionState = match io::exists(&working, layout.session_basename(), codecs.session).await {
			true => io::read_single(&working, layout.session_basename(), codecs.session).await?,
			false => SessionState::default(),
		};

		let has_registry = io::exists(&working, layout.registry_basename(), codecs.registry).await;
		let has_history = io::exists(&working, layout.history_basename(), codecs.history).await;

		let mut session = match (has_registry, has_history) {
			(true, true) => {
				let registry: Registry = io::read_single(&working, layout.registry_basename(), codecs.registry).await?;
				let history = load_history(&working, &layout, codecs.history).await?;
				Session::load(manifest.peer_id, registry, history, session_state.head_rev, session_state.redo_stack, session_state.next_node_counter)
			}
			(true, false) => {
				// Registry-only export: synthesize a history that reproduces this state.
				let registry: Registry = io::read_single(&working, layout.registry_basename(), codecs.registry).await?;
				Session::bootstrap_from_registry(manifest.peer_id, registry)?
			}
			(false, _) => Session::replay_from_history(manifest.peer_id, load_history(&working, &layout, codecs.history).await?, session_state.next_node_counter)?,
		};

		replay_hot_log(&working, &layout, codecs.hot_log, &mut session).await?;

		Ok(Self {
			session,
			working: Arc::new(working),
			layout,
			manifest,
			view_settings: session_state.view_settings,
			network_view_settings: session_state.network_view_settings,
		})
	}

	/// Backend-agnostic create. Records the working-copy default codecs (see `DEFAULT_*_CODEC`) in
	/// the manifest and writes each payload with its recorded codec.
	pub async fn create_in(working: AnyContainer, layout: L, peer: PeerId, document_uuid: u64, editor_version: String, stdlib_version: String) -> Result<Self, Error> {
		let manifest = Manifest::new(document_uuid, peer, editor_version, stdlib_version);
		let codecs = manifest.codecs;
		io::write_single(&working, layout.manifest_basename(), MANIFEST_CODEC, &manifest)?;
		io::write_single(&working, layout.session_basename(), codecs.session, &SessionState::default())?;

		let session = Session::with_peer(peer);
		io::write_single(&working, layout.registry_basename(), codecs.registry, session.registry())?;

		Ok(Self {
			session,
			working: Arc::new(working),
			layout,
			manifest,
			view_settings: std::collections::HashMap::new(),
			network_view_settings: std::collections::HashMap::new(),
		})
	}
}

fn validate_manifest(manifest: &Manifest) -> Result<(), Error> {
	if manifest.format != manifest::FORMAT_MAGIC {
		return Err(Error::WrongFormat {
			found: manifest.format.clone(),
			expected: manifest::FORMAT_MAGIC,
		});
	}
	if manifest.format_version > manifest::SUPPORTED_FORMAT_VERSION {
		return Err(Error::UnsupportedVersion {
			found: manifest.format_version,
			max_supported: manifest::SUPPORTED_FORMAT_VERSION,
		});
	}
	Ok(())
}

async fn load_history<L: Layout>(working: &AnyContainer, layout: &L, codec: Codec) -> Result<Vec<Delta>, Error> {
	if !io::exists(working, layout.history_basename(), codec).await {
		return Ok(Vec::new());
	}
	Ok(io::iter::<Delta>(working, layout.history_basename(), codec).await?)
}

async fn replay_hot_log<L: Layout>(working: &AnyContainer, layout: &L, codec: Codec, session: &mut Session) -> Result<(), Error> {
	if !io::exists(working, layout.hot_log_basename(), codec).await {
		return Ok(());
	}
	for hot_op in io::iter::<HotOp>(working, layout.hot_log_basename(), codec).await? {
		session.replay_hot_op(hot_op)?;
	}
	Ok(())
}

impl<L: Layout> Gdd<L> {
	pub fn session(&self) -> &Session {
		&self.session
	}

	pub fn can_undo(&self) -> bool {
		self.session.can_undo()
	}

	pub fn can_redo(&self) -> bool {
		self.session.can_redo()
	}

	/// Move the undo cursor back one commit (silent-zone reflog undo) and persist the new cursor. Returns
	/// the undone `Rev`. The working registry is rewound in place by the reverse delta, so re-snapshot it
	/// (alongside `head`) or a reopen would read a `registry.bin` inconsistent with the persisted cursor.
	pub fn undo(&mut self) -> Result<Rev, Error> {
		let rev = self.session.undo()?;
		self.persist_registry_snapshot()?;
		self.persist_session_state()?;
		Ok(rev)
	}

	/// Re-apply the most-recently-undone commit and persist the new cursor and re-snapshotted registry.
	pub fn redo(&mut self) -> Result<Rev, Error> {
		let rev = self.session.redo()?;
		self.persist_registry_snapshot()?;
		self.persist_session_state()?;
		Ok(rev)
	}

	pub fn registry(&self) -> &Registry {
		self.session.registry()
	}

	/// Resolve each runtime `network_path` to its stable [`NetworkId`](graph_storage::NetworkId), so the
	/// editor can key per-network, per-peer view state by a stable id. See [`Session::network_ids`].
	pub fn network_ids<M: NodeMetadataSource>(
		&self,
		network: &graph_craft::document::NodeNetwork,
		metadata: &M,
	) -> Result<std::collections::HashMap<Vec<core_types::uuid::NodeId>, graph_storage::NetworkId>, CommitError> {
		self.session.network_ids(network, metadata)
	}

	/// Every resource hash referenced by the current registry or anywhere in history, so resource GC keeps
	/// redoable/re-undoable gestures' resources (notably proto-node declaration bytes) alive even when an
	/// undo has dropped them from the current registry.
	pub fn all_referenced_resource_hashes(&self) -> std::collections::HashSet<ResourceHash> {
		self.session.all_referenced_resource_hashes()
	}

	pub fn layout(&self) -> &L {
		&self.layout
	}

	/// Drop the session and return the working-copy container + layout.
	/// Intended for test code that needs to reopen against the same container; panics if the container
	/// is still shared by a `Gdd` clone (tests don't clone before calling this).
	pub fn into_storage(self) -> (AnyContainer, L) {
		let working = Arc::try_unwrap(self.working).unwrap_or_else(|_| panic!("into_storage called while the working-copy container is still shared by a Gdd clone"));
		(working, self.layout)
	}

	/// The in-memory manifest. `Gdd` is its sole writer, so this is authoritative without re-reading
	/// disk.
	pub fn manifest(&self) -> &Manifest {
		&self.manifest
	}

	/// Edit the cached manifest and persist it. Always JSON, synchronous.
	pub fn update_manifest(&mut self, edit: impl FnOnce(&mut Manifest)) -> Result<(), Error> {
		edit(&mut self.manifest);
		io::write_single(&self.working, self.layout.manifest_basename(), MANIFEST_CODEC, &self.manifest)?;
		Ok(())
	}

	/// Stage a runtime snapshot as hot ops without retiring: diff the runtime against the working
	/// registry, append the hot frames (so a crash recovers the work), and persist proto-node
	/// declaration bytes. The working registry reflects the edit immediately, but nothing enters durable
	/// retired history until [`retire_pending_gesture`](Self::retire_pending_gesture). Staging on every
	/// edit while retiring only at gesture boundaries lets several edits coalesce into one retired gesture.
	///
	/// # Errors
	/// [`Error::Commit`] if the runtime diff is rejected by the session. On an [`Error::Container`] /
	/// [`Error::Codec`] from persisting the hot frames, the session has already advanced past what the
	/// working copy reflects, so the caller should treat the document as needing re-persist.
	pub fn stage_runtime_snapshot<M: NodeMetadataSource>(
		&mut self,
		network: &graph_craft::document::NodeNetwork,
		metadata: &M,
		resources: &graphene_resource::ResourceRegistry,
		byte_store: &dyn ResourceStorage,
	) -> Result<(), Error> {
		let (hot_ops, declaration_bytes) = self.session.stage_from_runtime(network, metadata, resources)?;

		for hot_op in &hot_ops {
			self.append_hot_frame(hot_op)?;
		}

		// Persist proto-node declaration content to the byte store (the global cache in the editor,
		// the working-copy container for standalone export). Content-addressed, so re-storing
		// identical bytes on every commit is an idempotent no-op.
		for bytes in declaration_bytes.values() {
			byte_store.store(bytes);
		}
		Ok(())
	}

	/// Retire every pending hot op into durable history as a single gesture (marking the batch's last
	/// delta as the gesture boundary), then re-snapshot the registry. One gesture is one undo unit, so
	/// the caller invokes this at each undo-step boundary and before any undo/redo. A no-op when there
	/// are no pending hot ops.
	pub fn retire_pending_gesture(&mut self) -> Result<Vec<Rev>, Error> {
		let Some(up_to) = self.session.hot_log().iter().map(|hot_op| hot_op.timestamp).max() else {
			return Ok(Vec::new());
		};
		self.retire_inner(up_to, true)
	}

	/// Commit a runtime snapshot as one complete gesture: stage it, then immediately retire it into
	/// durable history. Convenience for callers that produce a whole gesture atomically (tests, and any
	/// one-shot commit). Equivalent to [`stage_runtime_snapshot`](Self::stage_runtime_snapshot) followed
	/// by [`retire_pending_gesture`](Self::retire_pending_gesture).
	pub fn commit_from_runtime<M: NodeMetadataSource>(
		&mut self,
		network: &graph_craft::document::NodeNetwork,
		metadata: &M,
		resources: &graphene_resource::ResourceRegistry,
		byte_store: &dyn ResourceStorage,
	) -> Result<Vec<Rev>, Error> {
		self.stage_runtime_snapshot(network, metadata, resources, byte_store)?;
		self.retire_pending_gesture()
	}

	/// Resolve the proto-node declarations referenced by the registry into a [`graph_storage::Declarations`]
	/// map, loading each `ProtoNode`'s bytes from `byte_store` (the global cache in the editor, the
	/// working-copy container for standalone). Only resources referenced by `Implementation::ProtoNode`
	/// are visited, so image/font resources are skipped. Cold-path (open / `to_runtime`); async
	/// because resource loads are.
	pub async fn declarations(&self, byte_store: &dyn LoadResource) -> graph_storage::Declarations {
		use graph_storage::Implementation;

		let registry = self.session.registry();
		let mut declarations = graph_storage::Declarations::new();

		for node in registry.node_instances.values() {
			let Implementation::ProtoNode(id) = node.implementation() else { continue };
			if declarations.contains_key(id) {
				continue;
			}
			let Some(hash) = registry.resources.get(id).and_then(|entry| entry.hash) else {
				log::error!("Declaration resource {id} has no resolved hash; cannot load ProtoNode");
				continue;
			};
			let Some(resource) = byte_store.load(hash).await else {
				log::error!("Declaration bytes for {id} (hash {hash}) missing from byte store");
				continue;
			};
			match graph_storage::decode_declaration(resource.as_ref()) {
				Ok(proto) => {
					declarations.insert(*id, proto);
				}
				Err(error) => log::error!("Failed to deserialize ProtoNode for {id}: {error}"),
			}
		}

		declarations
	}

	/// Apply a hot op from the broadcast stream, appending one frame to the hot log.
	///
	/// # Errors
	/// Returns [`Error::Crdt`] if the op is rejected by the session. A failure to persist the hot
	/// frame is logged, not returned.
	pub fn apply_hot_op(&mut self, op: HotOp) -> Result<(), Error> {
		self.session.apply_hot_op(op.clone())?;
		if let Err(error) = self.append_hot_frame(&op) {
			log::error!("Failed to append hot op frame: {error}");
		}
		Ok(())
	}

	/// Persist freshly-staged hot ops and immediately retire them into durable history. Appends each
	/// hot frame (so a crash before retirement still recovers the work), then retires up to the last
	/// staged timestamp, which drains exactly these ops and re-snapshots the registry. Returns the
	/// retired `Rev`s. A no-op when nothing was staged.
	fn append_and_retire(&mut self, hot_ops: &[HotOp], gesture: bool) -> Result<Vec<Rev>, Error> {
		let Some(last) = hot_ops.last() else { return Ok(Vec::new()) };

		for hot_op in hot_ops {
			self.append_hot_frame(hot_op)?;
		}

		self.retire_inner(last.timestamp, gesture)
	}

	/// Encode the history deltas identified by `revs` and append them to the history file. Iterates
	/// `session.history()` (topological/append order) filtered by `revs` membership, so the appended
	/// frames preserve replay order regardless of the order `revs` lists them in.
	fn append_history_deltas(&mut self, revs: &[Rev]) -> Result<(), Error> {
		let wanted: std::collections::HashSet<Rev> = revs.iter().copied().collect();
		let mut buffer = Vec::new();
		for delta in self.session.history().filter(|delta| wanted.contains(&delta.id)) {
			self.manifest.codecs.history.append(&mut buffer, delta)?;
		}
		self.working.append_non_blocking(&io::path_for(self.layout.history_basename(), self.manifest.codecs.history), &buffer)?;
		Ok(())
	}

	/// Set a local annotation (e.g. a commit message) on an existing retired delta and re-persist it.
	/// Unlike the per-gesture marker written inline at retire, this targets an already-written delta, so
	/// the whole history file is rewritten in topological order. O(history) — fine for occasional user
	/// labeling, not for per-gesture marking (which uses the inline path). No-op if `rev` is unknown.
	pub fn annotate_delta(&mut self, rev: Rev, key: &str, value: serde_json::Value) -> Result<(), Error> {
		if self.session.annotate_delta(rev, key, value) {
			self.rewrite_history()?;
		}
		Ok(())
	}

	/// Rewrite the entire history file from the in-memory session. `history()` yields deltas in
	/// topological (append) order, which is a valid replay order, so no separate sort is needed.
	fn rewrite_history(&mut self) -> Result<(), Error> {
		let mut buffer = Vec::new();
		for delta in self.session.history() {
			self.manifest.codecs.history.append(&mut buffer, delta)?;
		}
		self.working.write_non_blocking(&io::path_for(self.layout.history_basename(), self.manifest.codecs.history), &buffer)?;
		Ok(())
	}

	fn persist_session_state(&mut self) -> Result<(), Error> {
		let state = SessionState {
			head_rev: self.session.head_rev(),
			redo_stack: self.session.redo_stack().to_vec(),
			next_node_counter: self.session.next_node_counter(),
			view_settings: self.view_settings.clone(),
			network_view_settings: self.network_view_settings.clone(),
		};
		io::write_single(&self.working, self.layout.session_basename(), self.manifest.codecs.session, &state)?;
		Ok(())
	}

	/// Re-snapshot the materialized working registry to `registry.bin`. `Session::load` trusts the stored
	/// registry to match the persisted `head`, so any cursor move (undo/redo) that rewinds the working
	/// registry without retiring must re-persist it or a reopen would read a registry inconsistent with
	/// `head`. Synchronous and hot-path-safe (`write_non_blocking`).
	fn persist_registry_snapshot(&mut self) -> Result<(), Error> {
		io::write_single(&self.working, self.layout.registry_basename(), self.manifest.codecs.registry, self.session.registry())?;
		Ok(())
	}

	/// The per-peer view settings read from `session.json` (PTZ, rulers, overlays, snapping, collapse).
	/// Opaque `ui::doc::*` blobs; the editor decodes them. Empty for a fresh document.
	pub fn view_settings(&self) -> &std::collections::HashMap<String, serde_json::Value> {
		&self.view_settings
	}

	/// Replace the per-peer view settings and persist them to `session.json`. Called by the editor when
	/// the viewport or a document-level toggle changes; never enters the registry, history, or CRDT.
	pub fn set_view_settings(&mut self, view_settings: std::collections::HashMap<String, serde_json::Value>) -> Result<(), Error> {
		self.view_settings = view_settings;
		self.persist_session_state()
	}

	/// The per-network view settings read from `session.json` (node-graph nav + previewing), keyed by
	/// [`NetworkId`](graph_storage::NetworkId). Opaque `ui::nav::*` / `ui::previewing` blobs the editor decodes.
	pub fn network_view_settings(&self) -> &std::collections::HashMap<graph_storage::NetworkId, std::collections::HashMap<String, serde_json::Value>> {
		&self.network_view_settings
	}

	/// Replace the per-network view settings and persist them to `session.json`. Per-peer, per-network; never
	/// enters the registry, history, or CRDT.
	pub fn set_network_view_settings(&mut self, network_view_settings: std::collections::HashMap<graph_storage::NetworkId, std::collections::HashMap<String, serde_json::Value>>) -> Result<(), Error> {
		self.network_view_settings = network_view_settings;
		self.persist_session_state()
	}

	fn append_hot_frame(&mut self, op: &HotOp) -> Result<(), Error> {
		let mut buffer = Vec::new();
		self.manifest.codecs.hot_log.append(&mut buffer, op)?;
		self.working.append_non_blocking(&io::path_for(self.layout.hot_log_basename(), self.manifest.codecs.hot_log), &buffer)?;
		Ok(())
	}

	/// Working-copy checkpoint: promote hot ops with timestamp `≤ up_to` into retired deltas,
	/// append them to the history file, rewrite the hot log with remaining (unretired) ops,
	/// re-snapshot the registry, and bump `last_retired_at` on the manifest. Synchronous.
	pub fn retire(&mut self, up_to: TimeStamp) -> Result<Vec<Rev>, Error> {
		self.retire_inner(up_to, false)
	}

	/// `gesture`: mark the batch's last delta as a gesture boundary (one undo unit) before its history
	/// frame is written, so the marker persists on reopen without a later frame rewrite.
	fn retire_inner(&mut self, up_to: TimeStamp, gesture: bool) -> Result<Vec<Rev>, Error> {
		let new_revs = self.session.retire(up_to)?;

		// Mark before `append_history_deltas` so the on-disk frame carries the boundary.
		if gesture && let Some(&last) = new_revs.last() {
			self.session.mark_interaction_end(last);
		}

		if !new_revs.is_empty() {
			self.append_history_deltas(&new_revs)?;
		}

		// Rewrite hot log with whatever survived retirement.
		let mut hot_buffer = Vec::new();
		for hot_op in self.session.hot_log() {
			self.manifest.codecs.hot_log.append(&mut hot_buffer, hot_op)?;
		}
		self.working
			.write_non_blocking(&io::path_for(self.layout.hot_log_basename(), self.manifest.codecs.hot_log), &hot_buffer)?;

		// Re-snapshot registry.
		io::write_single(&self.working, self.layout.registry_basename(), self.manifest.codecs.registry, self.session.registry())?;

		self.persist_session_state()?;

		// Bump cached manifest timestamp and persist it.
		self.update_manifest(|m| m.last_retired_at = Some(chrono::Utc::now().to_rfc3339()))?;

		Ok(new_revs)
	}

	pub async fn read_resource(&self, hash: &ResourceHash) -> Result<ByteHolder, ContainerError> {
		self.working.read(&self.layout.resource_path(hash)).await
	}

	/// Store the legacy `.graphite` document bytes verbatim inside the working copy (dual-write soak).
	/// Synchronous (hot-path safe via `write_non_blocking`): called at the autosave boundary alongside
	/// the registry snapshot. The bytes are opaque to `Gdd` — it never deserializes them.
	pub fn store_legacy_document(&self, bytes: &[u8]) -> Result<(), ContainerError> {
		self.working.write_non_blocking(self.layout.legacy_basename(), bytes)
	}

	/// Read back the embedded legacy `.graphite` document, if present. The compare-on-open oracle and
	/// the recovery fallback both go through here. `None` when no legacy blob was ever written.
	pub async fn read_legacy_document(&self) -> Option<ByteHolder> {
		self.working.read(self.layout.legacy_basename()).await.ok()
	}

	/// Register a resource under `id` and store its bytes. Commits an `AddResource` delta (a single
	/// `DataSource::Embedded` source resolved to the content hash) through the session so the registry
	/// records the resource and the entry replicates, then writes the bytes into the working copy's
	/// content-addressed store. The caller owns `id` allocation.
	pub fn add_resource(&mut self, id: graph_storage::ResourceId, bytes: &[u8]) -> Result<(), Error> {
		let hash = ResourceHash::from(bytes);

		self.working.write_non_blocking(&self.layout.resource_path(&hash), bytes)?;

		let hot_ops = self.session.stage_embedded_resource(id, hash)?;
		self.append_and_retire(&hot_ops, false)?;
		Ok(())
	}

	/// Like [`add_resource`](Self::add_resource) but copies the bytes from a filesystem `src` rather
	/// than buffering them. Folder backends use `fs::copy` (CoW on supported filesystems); other
	/// backends fall back to read-then-write. Native-only: there is no filesystem source path on wasm.
	#[cfg(not(target_family = "wasm"))]
	pub fn add_resource_from_path(&mut self, id: graph_storage::ResourceId, hash: ResourceHash, src: &Path) -> Result<(), Error> {
		let dest_path = self.layout.resource_path(&hash);
		if let AnyContainer::Folder(folder) = self.working.as_ref() {
			let full = folder.root().join(&dest_path);
			if let Some(parent) = full.parent() {
				std::fs::create_dir_all(parent).map_err(ContainerError::Io)?;
			}
			std::fs::copy(src, &full).map_err(ContainerError::Io)?;
		} else {
			let bytes = std::fs::read(src).map_err(ContainerError::Io)?;
			self.working.write_non_blocking(&dest_path, &bytes)?;
		}

		let hot_ops = self.session.stage_embedded_resource(id, hash)?;
		self.append_and_retire(&hot_ops, false)?;
		Ok(())
	}

	pub async fn has_resource(&self, hash: &ResourceHash) -> bool {
		self.working.exists(&self.layout.resource_path(hash)).await
	}

	pub fn remove_resource(&self, hash: &ResourceHash) -> Result<(), ContainerError> {
		self.working.remove_non_blocking(&self.layout.resource_path(hash))
	}

	pub fn resource_proxy(&self) -> ResourceProxy<L>
	where
		L: Clone,
	{
		ResourceProxy(self.working.clone(), self.layout.clone())
	}

	/// Enumerate every resource currently in the working copy. Paths that don't parse as a
	/// `ResourceHash` (foreign files dropped into the resources directory) are silently skipped.
	pub async fn resource_hashes(&self) -> Result<Vec<ResourceHash>, ContainerError> {
		let dir = self.layout.resources_dir();
		if !self.working.list_dirs("").await?.iter().any(|d| d == dir) {
			return Ok(Vec::new());
		}
		let entries = self.working.list(dir).await?;
		let prefix = format!("{dir}/");
		let mut hashes = Vec::with_capacity(entries.len());
		for entry in entries {
			let Some(name) = entry.strip_prefix(&prefix) else { continue };
			if let Ok(hash) = name.parse::<ResourceHash>() {
				hashes.push(hash);
			}
		}
		Ok(hashes)
	}

	/// Build a self-contained export of the working copy: keeps typed payloads in their recorded
	/// codecs (no re-encode), omits session/hot-log (peer-local + ephemeral), copies resources
	/// straight through, then materializes as a folder, zip, or xz archive at `dest`. Does not mutate
	/// `self` and does not buffer the full export — resources stream end-to-end. Native-only:
	/// export writes to a filesystem path.
	///
	/// `byte_store` is the source for `embed_all_resources`: in the editor the working copy holds no
	/// resource bytes (they live in the app-global cache), so embedding resolves each registry hash
	/// through the store. It is unused when `embed_all_resources` is false.
	///
	/// # Errors
	/// [`Error::InvalidExportOptions`] if the options are incoherent, or [`Error::MissingResource`] if
	/// an embedded resource's bytes are absent from `byte_store`.
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

	/// Build a self-contained archive of the working copy in memory and return its bytes, instead of
	/// writing to a filesystem path. Available on every target (no `std::fs`), so the editor can hand
	/// the bytes to the frontend to download / save. Buffers the whole archive in memory; fine for
	/// document-sized saves, not for huge exports (the streaming `export` covers that, native-only).
	///
	/// `legacy_document`, when present, is embedded verbatim at `Layout::legacy_basename()`, so the
	/// produced `.gdd` carries the legacy `.graphite` fallback the dual-write soak relies on.
	/// `ExportFormat::Folder` has no single-file byte form and is rejected.
	pub async fn export_to_bytes(&self, format: ExportFormat, options: ExportOptions, byte_store: &dyn LoadResource, legacy_document: Option<&[u8]>) -> Result<Vec<u8>, Error> {
		use document_container::archive::Archive;

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

	/// Drive a sink through manifest → registry → history → resources. Payloads keep the working
	/// copy's recorded per-payload codecs (no re-encode), so registry stays single-value and history
	/// stays multi-value without the caller having to keep them coherent. Each entry is written one
	/// at a time so the sink only ever sees one payload's bytes; the manifest itself is always JSON.
	async fn stream_entries(&self, options: ExportOptions, byte_store: &dyn LoadResource, sink: &mut dyn ExportSink) -> Result<(), Error> {
		use document_container::AsyncContainer;

		let codecs = self.manifest.codecs;
		sink.write_entry(&io::path_for(self.layout.manifest_basename(), MANIFEST_CODEC), &MANIFEST_CODEC.write_single(&self.manifest)?)?;

		// Include the per-peer session state (cursor + `view_settings` like PTZ/rulers) so a `.gdd` opened on
		// another machine restores the saved viewport and undo position. It's working-copy-only otherwise, so
		// without this the archive's `view_settings` would be empty on open and the viewport would reset.
		let session_state = SessionState {
			head_rev: self.session.head_rev(),
			redo_stack: self.session.redo_stack().to_vec(),
			next_node_counter: self.session.next_node_counter(),
			view_settings: self.view_settings.clone(),
			network_view_settings: self.network_view_settings.clone(),
		};
		sink.write_entry(&io::path_for(self.layout.session_basename(), codecs.session), &codecs.session.write_single(&session_state)?)?;

		// Hashes the working copy already holds on disk; their bytes are copied through verbatim below
		// and don't need the byte store.
		let working_copy_hashes: std::collections::HashSet<ResourceHash> = self.resource_hashes().await?.into_iter().collect();

		// Decide which resources travel as bytes. A resource already marked `Embedded` always has its
		// bytes materialized (in the editor they live in the byte store, not the working copy, so a
		// plain export must still pull them). `embed_all_resources` additionally promotes link-only
		// resources (`Url`/`FilePath`/`Font`) by prepending an `Embedded` source for a self-contained
		// export. Bytes already in the working copy are skipped here; the copy-through pass writes them.
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

		// Multiple resource entries can resolve to the same content hash; dedup so each is loaded once.
		hashes_from_store.sort_unstable();
		hashes_from_store.dedup();

		// Load the gap from the byte store (fail fast if an embedded resource is missing), then commit
		// the link promotions as real `AddSource` deltas on the clone so the exported registry and
		// history stay consistent. The live `Gdd` is untouched.
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

		// Copy whatever resource bytes the working copy already holds, tracking which hashes are
		// covered so the embed pass doesn't re-emit them.
		let mut emitted = std::collections::HashSet::new();
		let resources_dir = self.layout.resources_dir();
		if self.working.list_dirs("").await?.iter().any(|d| d == resources_dir) {
			let prefix = format!("{resources_dir}/");
			for path in self.working.list(resources_dir).await? {
				if let Some(hash) = path.strip_prefix(&prefix).and_then(|name| name.parse::<ResourceHash>().ok()) {
					emitted.insert(hash);
				}
				let holder = self.working.read(&path).await?;

				// On native, an `External` (mmap'd) holder exposes a source path the sink can copy
				// directly (CoW / kernel-side); other holders, and every holder on wasm (OPFS has no
				// filesystem path), fall back to writing the in-memory bytes.
				#[cfg(not(target_family = "wasm"))]
				match holder.source_path() {
					Some(src_path) => sink.write_entry_from_path(&path, src_path)?,
					None => sink.write_entry(&path, holder.as_slice())?,
				}
				#[cfg(target_family = "wasm")]
				sink.write_entry(&path, holder.as_slice())?;
			}
		}

		// Write the embedded bytes the working copy didn't already hold.
		for (hash, resource) in &embedded_bytes {
			if emitted.insert(*hash) {
				sink.write_entry(&self.layout.resource_path(hash), resource.as_ref())?;
			}
		}

		Ok(())
	}
}

impl<L: Layout + Send + Sync> LoadResource for Gdd<L> {
	fn load(&self, hash: ResourceHash) -> ResourceFuture<'_> {
		Box::pin(async move {
			let bytes = self.working.read(&self.layout.resource_path(&hash)).await.ok()?;
			Some(Resource::new(bytes))
		})
	}
}
pub struct ResourceProxy<T: Layout>(Arc<AnyContainer>, T);

impl<L: Layout + Send + Sync> LoadResource for ResourceProxy<L> {
	fn load(&self, hash: ResourceHash) -> ResourceFuture<'_> {
		Box::pin(async move {
			let bytes = self.0.read(&self.1.resource_path(&hash)).await.ok()?;
			Some(Resource::new(bytes))
		})
	}
}

impl<L: Layout + Send + Sync> ResourceStorage for Gdd<L> {
	fn store(&self, data: &[u8]) -> ResourceHash {
		let hash = ResourceHash::from(data);
		if let Err(error) = self.working.write_non_blocking(&self.layout.resource_path(&hash), data) {
			log::error!("ResourceStorage::store failed for {hash}: {error}");
		}
		hash
	}

	fn contains(&self, hash: &ResourceHash) -> bool {
		self.working.exists_non_blocking(&self.layout.resource_path(hash))
	}

	fn garbage_collect(&self, used: &[ResourceHash]) {
		// `garbage_collect` is synchronous but listing resources is async, so the native path blocks on
		// it. That's unavailable on wasm (single-threaded; `block_on` would deadlock). The editor never
		// uses `Gdd` as the runtime `ResourceStorage` on wasm (it GCs the app-global cache instead), so
		// this is an unreachable configuration there rather than a missing feature.
		#[cfg(target_family = "wasm")]
		{
			let _ = used;
			log::error!("ResourceStorage::garbage_collect is not supported for Gdd on wasm");
		}
		#[cfg(not(target_family = "wasm"))]
		{
			let kept: std::collections::HashSet<&ResourceHash> = used.iter().collect();
			let hashes = match futures::executor::block_on(self.resource_hashes()) {
				Ok(hashes) => hashes,
				Err(error) => {
					log::error!("Failed to list resources during garbage_collect: {error}");
					return;
				}
			};
			for hash in hashes {
				if kept.contains(&hash) {
					continue;
				}
				if let Err(error) = self.working.remove_non_blocking(&self.layout.resource_path(&hash)) {
					log::error!("ResourceStorage::garbage_collect failed to remove {hash}: {error}");
				}
			}
		}
	}
}

/// Abstraction over the sink an export streams entries into. Lets a single async loop drive
/// folder writes, zip writes, and xz writes without duplicating the entry sequence. The archive
/// sinks (zip/xz) work on every target since the codecs are pure-Rust; only the folder sink and
/// the filesystem copy-through (`write_entry_from_path`) are native-only.
///
/// `Send` because `stream_entries` holds `&mut dyn ExportSink` across `.await`s, so the enclosing
/// future (e.g. the editor's save future) must be `Send` on native. The concrete sinks are all `Send`.
trait ExportSink: Send {
	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error>;

	/// Copy a file from disk into the sink. Default impl reads the source into memory and
	/// forwards to `write_entry`; sinks like the folder writer override to use `fs::copy`
	/// (CoW on supported filesystems, kernel-side copy otherwise). Native-only: only reachable
	/// for an `External` (mmap'd) holder, which doesn't exist on wasm.
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
