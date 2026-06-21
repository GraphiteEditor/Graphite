//! Typed handle for `.gdd` documents.
//!
//! [`Gdd`] owns a [`graph_storage::Session`] plus a working-copy [`document_container::AnyContainer`].
//! Mutations flow through `Gdd` to keep the session and the on-disk working copy mirrored.
//! Export is a separate, explicit operation — see [`export::ExportFormat`].
//!
//! See the "On-disk container" section of `node-graph/rfcs/document-format.md` for the format spec.

use std::sync::Arc;
// `Path` and `FolderBackend` are only used by the native-only path-based open/create, so they're
// gated off wasm to avoid unused-import warnings.
#[cfg(not(target_family = "wasm"))]
use std::path::Path;

#[cfg(not(target_family = "wasm"))]
use document_container::backends::folder::FolderBackend;
use document_container::{AnyContainer, AsyncContainer, ByteHolder, ContainerError};
#[cfg(feature = "conversion")]
use graph_storage::{CommitError, NodeMetadataSource};
use graph_storage::{Delta, HotOp, PeerId, Registry, Session};
#[cfg(feature = "conversion")]
use graphene_resource::LoadResource;
use graphene_resource::ResourceHash;

pub mod codec;
pub mod error;
pub mod export;
pub mod io;
pub mod layout;
pub mod manifest;
pub mod persist;
pub mod resource;
pub mod session_state;

pub use codec::{Codec, CodecError};
pub use error::Error;
pub use export::{ExportFormat, ExportOptions};
pub use io::ReadError;
pub use layout::{GddV1Layout, Layout};
pub use manifest::{Manifest, PayloadCodecs};
pub use resource::ResourceProxy;
pub use session_state::SessionState;

/// The default [`Layout`], so callers write `GddV1` for the common `Gdd<GddV1Layout>` handle.
pub type GddV1 = Gdd<GddV1Layout>;

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
pub struct Gdd<L: Layout = GddV1Layout> {
	pub(crate) session: Session,
	pub(crate) working: Arc<AnyContainer>,
	pub(crate) layout: L,
	/// In-memory copy of the manifest, kept authoritative since `Gdd` is its sole writer. Holds the
	/// per-payload codecs (so the persist path never probes the filesystem) and `last_retired_at`
	/// (so retirement writes the manifest without first reading it). Lets the persist path stay
	/// fully read-free and synchronous.
	pub(crate) manifest: Manifest,
	/// Per-peer view settings (PTZ, rulers, etc.), persisted in `session.json` not the registry, so
	/// they stay out of the CRDT/history. Opaque to the storage layer; the editor owns the keys/values.
	pub(crate) view_settings: std::collections::BTreeMap<String, serde_json::Value>,
	/// Per-network view settings (node-graph nav + previewing), keyed by stable [`NetworkId`]. Same per-peer
	/// `session.json` treatment as [`view_settings`](Self::view_settings), but scoped per network.
	pub(crate) network_view_settings: std::collections::BTreeMap<graph_storage::NetworkId, std::collections::BTreeMap<String, serde_json::Value>>,
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
	#[cfg(any(feature = "zip", feature = "xz"))]
	pub async fn open_from_archive(bytes: &[u8], mut working: AnyContainer, layout: L) -> Result<Self, Error> {
		document_container::archive::open_auto(bytes, &mut working)?;

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
			view_settings: std::collections::BTreeMap::new(),
			network_view_settings: std::collections::BTreeMap::new(),
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

	pub fn registry(&self) -> &Registry {
		self.session.registry()
	}

	/// The in-memory manifest. `Gdd` is its sole writer, so this is authoritative without re-reading
	/// disk.
	pub fn manifest(&self) -> &Manifest {
		&self.manifest
	}

	pub fn layout(&self) -> &L {
		&self.layout
	}

	/// Resolve each runtime `network_path` to its stable [`NetworkId`](graph_storage::NetworkId), so the
	/// editor can key per-network, per-peer view state by a stable id. See [`Session::network_ids`].
	#[cfg(feature = "conversion")]
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

	/// Drop the session and return the working-copy container + layout.
	/// Intended for test code that needs to reopen against the same container; panics if the container
	/// is still shared by a `Gdd` clone (tests don't clone before calling this).
	pub fn into_storage(self) -> (AnyContainer, L) {
		let working = Arc::try_unwrap(self.working).unwrap_or_else(|_| panic!("into_storage called while the working-copy container is still shared by a Gdd clone"));
		(working, self.layout)
	}

	/// Resolve the proto-node declarations referenced by the registry into a [`graph_storage::Declarations`]
	/// map, loading each `ProtoNode`'s bytes from `byte_store` (the global cache in the editor, the
	/// working-copy container for standalone). Only resources referenced by `Implementation::ProtoNode`
	/// are visited, so image/font resources are skipped. Cold-path (open / `to_runtime`); async
	/// because resource loads are.
	#[cfg(feature = "conversion")]
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

	/// Store the legacy `.graphite` document bytes verbatim inside the working copy (dual-write soak).
	/// Synchronous (hot-path safe via `write_non_blocking`): called at the autosave boundary alongside
	/// the registry snapshot. The bytes are opaque to `Gdd` — it never deserializes them.
	pub fn store_legacy_document(&self, bytes: &[u8]) -> Result<(), ContainerError> {
		self.working.write_non_blocking(self.layout.legacy_path(), bytes)
	}

	/// Read back the embedded legacy `.graphite` document, if present. The compare-on-open oracle and
	/// the recovery fallback both go through here. `None` when no legacy blob was ever written.
	pub async fn read_legacy_document(&self) -> Option<ByteHolder> {
		self.working.read(self.layout.legacy_path()).await.ok()
	}
}
