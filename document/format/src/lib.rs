//! Typed handle for `.gdd` documents.
//!
//! [`Gdd`] owns a [`document_graph_storage::Session`] plus a working-copy [`document_container::AnyContainer`].
//! Mutations flow through `Gdd` to keep the session and the on-disk working copy mirrored.
//! Export is a separate, explicit operation — see [`export::ExportFormat`].
//!
//! See the "On-disk container" section of `node-graph/rfcs/document-format.md` for the format spec.

// `Path` and `FolderBackend` are only used by the native-only path-based open/create, so they're
// gated off wasm to avoid unused-import warnings.
#[cfg(not(target_family = "wasm"))]
use std::path::Path;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use document_container::backends::folder::FolderBackend;
use document_container::{AnyContainer, AsyncContainer, ByteHolder};
#[cfg(feature = "migrations")]
use document_graph_storage::Rev;
#[cfg(feature = "conversion")]
use document_graph_storage::{CommitError, NodeMetadataSource};
use document_graph_storage::{HistoryMetadata, MetadataFact, PeerId, Registry, Session, Sorted, UserId};
#[cfg(feature = "conversion")]
use graphene_resource::LoadResource;
use graphene_resource::{ResourceHash, ResourceStorage};

pub mod codec;
pub mod error;
pub mod export;
pub mod io;
pub mod layout;
mod load;
pub mod manifest;
#[cfg(feature = "migrations")]
mod migrate;
#[cfg(feature = "network")]
pub mod network;
pub mod persist;
pub mod resource;

pub use codec::{Codec, CodecError};
pub use document_graph_storage::SessionState;
pub use error::Error;
pub use export::{ExportFormat, ExportOptions};
pub use io::ReadError;
pub use layout::{GddV1Layout, Layout};
pub use manifest::{Manifest, PayloadCodecs};
pub use resource::ResourceProxy;

/// The default [`Layout`], so callers write `GddV1` for the common `Gdd<GddV1Layout>` handle.
pub type GddV1 = Gdd<GddV1Layout>;

/// The manifest is always JSON: it is the bootstrap file, read before any other payload's codec is
/// known, so its own codec cannot itself be configurable.
pub const MANIFEST_CODEC: Codec = Codec::Json;

/// Working-copy codecs. The working copy lives in appdata, not under VCS — these defaults
/// optimize for size and write cost. JSON/JSONL is opt-in via `ExportFormat::Folder` for users who
/// want a diffable on-disk representation. Recorded in the manifest at create time and read back on
/// open (see [`manifest::PayloadCodecs`]), so the persist path never probes the filesystem.
pub const DEFAULT_SESSION_CODEC: Codec = Codec::Json;
/// One fact per line, so recording one only appends.
pub const METADATA_CODEC: Codec = Codec::JsonLines;

/// The facts as lines of the metadata file.
pub(crate) fn encode_facts<'a>(facts: impl IntoIterator<Item = &'a MetadataFact>) -> Result<Vec<u8>, Error> {
	let mut buffer = Vec::new();
	for fact in facts {
		METADATA_CODEC.append(&mut buffer, fact)?;
	}
	Ok(buffer)
}
pub const DEFAULT_REGISTRY_CODEC: Codec = Codec::Postcard;
pub const DEFAULT_HISTORY_CODEC: Codec = Codec::PostcardFrames;
pub const DEFAULT_HOT_LOG_CODEC: Codec = Codec::PostcardFrames;

/// Editor-facing handle. Owns the `Session` and the working-copy container; mutations are mirrored
/// to disk continuously (every retirement appends to the history file and re-snapshots the registry).
///
/// The per-edit persist path (`commit_from_runtime`, `apply_hot_op`, `retire`) is synchronous and
/// read-free: the manifest is cached in memory (so payload codecs need no disk read), and writes go
/// through the container's sync write surface. Only `open` / `export` are async, since they
/// read.
/// `Clone` shares the working-copy container (`AnyContainer` is a handle) so a cloned handle reads and writes
/// the *same* on-disk/OPFS working copy — including any writes still queued on the OPFS backend. The
/// `Session` is cloned as a snapshot copy. A clone is not connected to the room.
pub struct Gdd<L: Layout = GddV1Layout> {
	pub(crate) session: Session,
	pub(crate) working: AnyContainer,
	pub(crate) layout: L,
	/// In-memory copy of the manifest, kept authoritative since `Gdd` is its sole writer. Holds the
	/// per-payload codecs so the persist path never probes the filesystem, keeping it fully read-free
	/// and synchronous.
	pub(crate) manifest: Manifest,
	/// See [`HistoryMetadata`].
	pub(crate) metadata: HistoryMetadata,
	/// Per-peer view settings (PTZ, rulers, etc.), persisted in `session.json` not the registry, so
	/// they stay out of the CRDT/history. Opaque to the storage layer; the editor owns the keys/values.
	pub(crate) view_settings: std::collections::BTreeMap<String, document_graph_storage::Value>,
	/// Per-network view settings (node-graph nav + previewing), keyed by stable [`NetworkId`]. Same per-peer
	/// `session.json` treatment as [`view_settings`](Self::view_settings), but scoped per network.
	pub(crate) network_view_settings: std::collections::BTreeMap<document_graph_storage::NetworkId, std::collections::BTreeMap<String, document_graph_storage::Value>>,
	pub(crate) migrated: bool,
	/// Where resource bytes live. `None` means the working copy's own content-addressed store.
	pub(crate) byte_store: Option<Arc<dyn ResourceStorage>>,
	#[cfg(feature = "network")]
	pub(crate) network: Option<peer_transport::Replica>,
	#[cfg(feature = "network")]
	pub(crate) pending_persist: PendingPersist,
	/// What peers changed since [`take_remote_changes`](Self::take_remote_changes) last took it.
	#[cfg(feature = "network")]
	pub(crate) remote_changes: network::RemoteChanges,
	/// Whether the document belongs in its room, persisted so a reopen reconnects.
	pub(crate) shared: bool,
}

/// Whole-file rewrites deferred to the end of a poll, so a batch of remote packets costs one rewrite per file.
#[cfg(feature = "network")]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PendingPersist {
	pub(crate) history: bool,
	pub(crate) hot_log: bool,
	pub(crate) snapshot: bool,
}

impl<L: Layout + Clone> Clone for Gdd<L> {
	fn clone(&self) -> Self {
		Self {
			view_settings: self.view_settings.clone(),
			network_view_settings: self.network_view_settings.clone(),
			byte_store: self.byte_store.clone(),
			migrated: self.migrated,
			shared: self.shared,
			..Self::assemble(self.session.clone(), self.working.clone(), self.layout.clone(), self.manifest.clone(), self.metadata.clone())
		}
	}
}

impl<L: Layout> Gdd<L> {
	/// A handle with no view settings, not shared, and no live state.
	fn assemble(session: Session, working: AnyContainer, layout: L, manifest: Manifest, metadata: HistoryMetadata) -> Self {
		Self {
			session,
			working,
			layout,
			manifest,
			metadata,
			view_settings: Default::default(),
			network_view_settings: Default::default(),
			byte_store: None,
			migrated: false,
			#[cfg(feature = "network")]
			network: None,
			#[cfg(feature = "network")]
			pending_persist: PendingPersist::default(),
			#[cfg(feature = "network")]
			remote_changes: network::RemoteChanges::default(),
			shared: false,
		}
	}
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
	pub fn create(path: &Path, peer: PeerId, document_uuid: u64, editor_version: String, stdlib_version: String) -> Result<Self, Error> {
		let working = AnyContainer::Folder(FolderBackend::create(path)?);
		let layout = L::default();
		Self::create_in(working, layout, peer, document_uuid, editor_version, stdlib_version)
	}
}

impl<L: Layout> Gdd<L> {
	/// Open a `.gdd` from archive bytes (xz or zip, auto-detected) by materializing it into `working`,
	/// then opening it as a working copy. The archive is deserialized into an in-memory staging backend
	/// (the archive reader is synchronous), then each entry is written into `working` via the sync
	/// `write_non_blocking` surface — durable on folder/memory, eagerly enqueued on OPFS. `working` is
	/// expected to be a fresh per-document container; entries with colliding paths are overwritten.
	#[cfg(any(feature = "zip", feature = "xz"))]
	pub async fn open_from_archive(bytes: &[u8], mut working: AnyContainer, layout: L) -> Result<Self, Error> {
		document_container::archive::open_auto(bytes, &mut working)?;

		Self::open_in(working, layout).await
	}

	/// Backend-agnostic open. Splits out so tests can supply a [`document_container::backends::memory::MemoryBackend`].
	///
	/// # Errors
	/// [`Error::MissingManifest`] if the container holds no manifest, [`Error::WrongFormat`] / [`Error::UnsupportedVersion`]
	/// if it fails validation, plus the usual [`Error::Read`] / [`Error::Codec`] / [`Error::Crdt`] if a payload is malformed.
	pub async fn open_in(working: AnyContainer, layout: L) -> Result<Self, Error> {
		persist::finish_staged_rewrite(&working, &layout).await?;

		let manifest: Manifest = match io::read_single(&working, layout.manifest_basename(), MANIFEST_CODEC).await {
			Ok(manifest) => manifest,
			Err(io::ReadError::NotFound { .. }) => return Err(Error::MissingManifest),
			Err(error) => return Err(error.into()),
		};
		validate_manifest(&manifest)?;
		let codecs = manifest.codecs;

		#[cfg_attr(not(feature = "migrations"), expect(unused_mut, reason = "only format migrations change the stored records"))]
		let mut records = load::StoredRecords::read(&working, &layout, codecs).await?;
		#[cfg(feature = "migrations")]
		let format_outcome = migrate::migrate_format(&mut records, codecs, manifest.format_version)?;
		#[cfg_attr(not(feature = "migrations"), expect(unused_mut, reason = "only format migrations change the decoded payloads"))]
		let mut payloads = records.decode(codecs)?;
		#[cfg(feature = "migrations")]
		let mut rehashed = std::collections::HashMap::new();
		#[cfg(feature = "migrations")]
		match format_outcome {
			Some(migration_runner::HistoryOutcome::Rewritten) => {
				// Records changed, so their content-addressed revs did too
				let revs = migration_runner::rehash_deltas(payloads.history.as_deref_mut().unwrap_or_default());
				rehashed = revs.clone();
				let remap = |rev: Rev| revs.get(&rev).copied().unwrap_or(rev);
				let state = &mut payloads.session;
				state.head_rev = state.head_rev.map(remap);
				state.last_broadcast_rev = state.last_broadcast_rev.map(remap);
				state.redo_stack.iter_mut().for_each(|rev| *rev = remap(*rev));
			}
			Some(migration_runner::HistoryOutcome::Truncated) => {
				let state = &mut payloads.session;
				state.head_rev = None;
				state.last_broadcast_rev = None;
				state.redo_stack.clear();
			}
			Some(migration_runner::HistoryOutcome::Untouched) | None => {}
		}
		let load::DecodedPayloads {
			registry,
			history,
			hot_log,
			session: mut session_state,
		} = payloads;
		let metadata = match io::exists(&working, layout.metadata_basename(), METADATA_CODEC).await {
			true => HistoryMetadata::fold(io::iter_lenient(&working, layout.metadata_basename(), METADATA_CODEC).await?),
			false => HistoryMetadata::default(),
		};
		#[cfg(feature = "migrations")]
		let metadata = match format_outcome {
			Some(migration_runner::HistoryOutcome::Rewritten) => migrate::remap_metadata(&metadata, |rev| Some(rehashed.get(&rev).copied().unwrap_or(rev))),
			Some(migration_runner::HistoryOutcome::Truncated) => migrate::remap_metadata(&metadata, |_| None),
			_ => metadata,
		};

		// The history a registry-only document gets here has to be stored before the first edit, which would otherwise append to nothing
		let bootstrapped = registry.is_some() && history.is_none();

		let peer = session_state.peer_id;
		let user = session_state.user_id;
		let mut session = match (registry, history) {
			(Some(registry), Some(history)) => Session::load(
				peer,
				user,
				registry,
				history,
				session_state.head_rev,
				std::mem::take(&mut session_state.redo_stack),
				session_state.next_node_counter,
			),
			// Registry-only export: synthesize a history that reproduces this state
			(Some(registry), None) => Session::bootstrap_from_registry(peer, user, registry)?,
			(None, history) => Session::replay_from_history(peer, user, history.unwrap_or_default(), session_state.next_node_counter)?,
		};

		// Before the hot log replays, so a retired op in it is dropped rather than staged again.
		restore_cursor(&mut session, &session_state);
		for hot_op in hot_log {
			session.replay_hot_op(hot_op)?;
		}

		let mut gdd = Self {
			view_settings: session_state.view_settings,
			network_view_settings: session_state.network_view_settings,
			shared: session_state.shared,
			..Self::assemble(session, working, layout, manifest, metadata)
		};

		#[cfg(feature = "migrations")]
		{
			let migrated_nodes = gdd.migrate_nodes().await?;
			gdd.migrated = format_outcome.is_some() || migrated_nodes;
			if gdd.migrated {
				gdd.manifest.format_version = gdd.manifest.format_version.max(manifest::SUPPORTED_FORMAT_VERSION);
			}
		}

		if bootstrapped || gdd.migrated {
			gdd.persist_all().await?;
		}

		Ok(gdd)
	}

	/// Backend-agnostic create. Records the working-copy default codecs (see `DEFAULT_*_CODEC`) in
	/// the manifest and writes each payload with its recorded codec.
	pub fn create_in(working: AnyContainer, layout: L, peer: PeerId, document_uuid: u64, editor_version: String, stdlib_version: String) -> Result<Self, Error> {
		let manifest = Manifest::new(document_uuid, editor_version, stdlib_version);
		let codecs = manifest.codecs;
		io::write_single(&working, layout.manifest_basename(), MANIFEST_CODEC, &manifest)?;
		let session_state = SessionState {
			peer_id: peer,
			user_id: UserId(peer.0),
			..Default::default()
		};
		io::write_single(&working, layout.session_basename(), codecs.session, &Sorted(&session_state))?;

		let session = Session::with_peer(peer);
		io::write_single(&working, layout.registry_basename(), codecs.registry, &Sorted(session.registry()))?;

		Ok(Self::assemble(session, working, layout, manifest, HistoryMetadata::default()))
	}
}

/// Carry the persisted cursor onto a freshly loaded session, whichever way it was loaded.
pub(crate) fn restore_cursor(session: &mut Session, state: &SessionState) {
	if let Some(rev) = state.last_broadcast_rev {
		session.publish_up_to(rev);
	}
	session.restore_hot_sequence(state.last_hot_sequence);
	session.restore_clock_counter(state.clock_counter);
	session.restore_next_node_counter(state.next_node_counter);
	session.absorb_retired_marks(&state.retired_marks);
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

	pub fn registry(&self) -> &Registry {
		self.session.registry()
	}

	/// The in-memory manifest. `Gdd` is its sole writer, so this is authoritative without re-reading
	/// disk.
	pub fn manifest(&self) -> &Manifest {
		&self.manifest
	}

	/// Whether opening ran a format step or node migration, and so rewrote the working copy.
	pub fn migrated(&self) -> bool {
		self.migrated
	}

	pub fn layout(&self) -> &L {
		&self.layout
	}

	/// The per-peer view settings read from `session.json` (PTZ, rulers, overlays, snapping, collapse).
	/// Opaque `ui::doc::*` blobs; the editor decodes them. Empty for a fresh document.
	pub fn view_settings(&self) -> &std::collections::BTreeMap<String, document_graph_storage::Value> {
		&self.view_settings
	}

	/// The per-network view settings read from `session.json` (node-graph nav + previewing), keyed by
	/// [`NetworkId`](document_graph_storage::NetworkId). Opaque `ui::nav::*` / `ui::previewing` blobs the editor decodes.
	pub fn network_view_settings(&self) -> &std::collections::BTreeMap<document_graph_storage::NetworkId, std::collections::BTreeMap<String, document_graph_storage::Value>> {
		&self.network_view_settings
	}

	/// The stable [`NetworkId`](document_graph_storage::NetworkId) of each runtime network location, to key per-network view state. See [`Session::network_ids`].
	#[cfg(feature = "conversion")]
	pub fn network_ids<M: NodeMetadataSource>(
		&self,
		network: &graph_craft::document::NodeNetwork,
		metadata: &M,
	) -> Result<std::collections::HashMap<graph_craft::document::OwnedNetworkLocation, document_graph_storage::NetworkId>, CommitError> {
		self.session.network_ids(network, metadata)
	}

	/// Every resource hash referenced by the current registry or anywhere in history, so resource GC keeps
	/// redoable/re-undoable interactions' resources (notably proto-node declaration bytes) alive even when an
	/// undo has dropped them from the current registry.
	pub fn all_referenced_resource_hashes(&self) -> std::collections::HashSet<ResourceHash> {
		self.session.all_referenced_resource_hashes()
	}

	/// Drop the session and return the working-copy container + layout.
	pub fn into_storage(self) -> (AnyContainer, L) {
		(self.working, self.layout)
	}

	/// Resolve every proto-node declaration referenced by the registry or its history into a
	/// [`document_graph_storage::Declarations`] map, loading each `ProtoNode`'s bytes from `byte_store`
	/// (the global cache in the editor, the working-copy container for standalone). Covers history so
	/// the map can serve any undo/redo target. Cold-path (mount / open); async because resource loads are.
	#[cfg(feature = "conversion")]
	pub async fn declarations(&self, byte_store: &dyn LoadResource) -> document_graph_storage::Declarations {
		let mut declarations = document_graph_storage::Declarations::new();

		for (id, hash) in self.session.all_declaration_resources() {
			let Some(hash) = hash else {
				log::error!("Declaration resource {id} has no resolved hash; cannot load ProtoNode");
				continue;
			};
			let Some(resource) = byte_store.load(hash).await else {
				log::error!("Declaration bytes for {id} (hash {hash}) missing from byte store");
				continue;
			};
			match document_graph_storage::decode_declaration(resource.as_ref()) {
				Ok(proto) => {
					declarations.insert(id, proto);
				}
				Err(error) => log::error!("Failed to deserialize ProtoNode for {id}: {error}"),
			}
		}

		declarations
	}

	/// Read the embedded legacy `.graphite` document, if present.
	pub async fn read_legacy_document(&self) -> Option<ByteHolder> {
		self.working.read(self.layout.legacy_path()).await.ok()
	}
}
