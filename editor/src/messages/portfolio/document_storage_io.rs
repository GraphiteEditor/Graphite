//! Asynchronous `.gdd` working-copy IO, spawned by `PortfolioMessageHandler` as `FutureMessage`s:
//! building/opening containers, opening `.gdd` archives into documents, and rebuilding the interface
//! from the undo/redo cursor. The `validate` flag is the `validate_storage_round_trip` preference; when
//! set, the registry build is compared against the legacy oracle (logged, not fatal) for the soak.

use document_container::AnyContainer;
use document_format::{Error as DocumentFormatError, GddV1, GddV1Layout};
use graph_craft::application_io::resource::{LoadResource, ResourceStorage};
use graph_craft::document::NodeNetwork;

use super::document::DocumentMessageHandler;
use super::document::diff_networks;
use super::document::utility_types::network_interface::storage_metadata::{apply_network_view_settings, build_interface_from_storage, network_ids_from_entries};
use super::document_migration::document_migration_string_preprocessing;
use super::portfolio_message::PortfolioMessage;
use crate::messages::message::Message;
use crate::messages::portfolio::document::utility_types::misc::DocumentId;
use crate::messages::resource_storage::ResourcesHandle;

/// Build the container backend at `path` (folder on native, OPFS directory name on web) or in-memory
/// when `None`. Also returns whether an existing working copy was found (its `manifest.json` is present).
async fn build_per_document_container(path: Option<&std::path::Path>) -> Result<(AnyContainer, bool), DocumentFormatError> {
	let Some(path) = path else {
		return Ok((AnyContainer::Memory(document_container::backends::memory::MemoryBackend::new()), false));
	};

	#[cfg(not(target_family = "wasm"))]
	{
		use document_container::backends::folder::FolderBackend;
		let exists = path.join("manifest.json").is_file();
		let backend = if exists { FolderBackend::open(path)? } else { FolderBackend::create(path)? };
		Ok((AnyContainer::Folder(backend), exists))
	}

	#[cfg(target_family = "wasm")]
	{
		use document_container::AsyncContainer;
		use document_container::backends::opfs::OpfsBackend;
		let directory_name = path
			.to_str()
			.ok_or(DocumentFormatError::Container(document_container::ContainerError::InvalidPath(path.display().to_string())))?;
		let backend = OpfsBackend::open(directory_name).await?;
		let exists = backend.exists("manifest.json").await;
		Ok((AnyContainer::Opfs(backend), exists))
	}
}

/// Open the existing working copy at `path`, or create a fresh one bound to `peer` (in-memory when
/// `path` is `None`). Returns the working copy plus whether it was reopened (vs freshly created); only a
/// reopen has independently-stored state worth comparing against the legacy load.
pub(super) async fn build_or_open_working_copy(path: Option<&std::path::Path>, peer: graph_storage::PeerId, document_uuid: u64, version: String) -> Result<(GddV1, bool), DocumentFormatError> {
	let (container, exists) = build_per_document_container(path).await?;

	let gdd = if exists {
		GddV1::open_in(container, GddV1Layout).await?
	} else {
		GddV1::create_in(container, GddV1Layout, peer, document_uuid, version.clone(), version).await?
	};
	Ok((gdd, exists))
}

/// `FutureMessage` that opens a `.gdd` archive into a document, delivered via
/// [`PortfolioMessage::GddDocumentLoaded`]. See [`build_document_from_gdd`] for the build itself.
pub(super) fn open_gdd_document_future(
	working_copy_root: Option<std::path::PathBuf>,
	document_id: DocumentId,
	document_name: Option<String>,
	document_path: Option<std::path::PathBuf>,
	content: Vec<u8>,
	store_handle: ResourcesHandle,
	validate: bool,
) -> Message {
	let path = working_copy_root.map(|root| root.join(format!("{:x}", document_id.0)));

	let future = async move {
		let document = build_document_from_gdd(path.as_deref(), &content, &store_handle, document_id, validate).await;
		Message::Portfolio(PortfolioMessage::GddDocumentLoaded {
			document_id,
			document_name,
			document_path,
			document: document.map(Box::new),
		})
	};
	future.into()
}

/// `FutureMessage` that rebuilds a document's interface from a post-move `Gdd` cursor snapshot and
/// delivers it via [`PortfolioMessage::GddUndoRedoRebuilt`] (`None` interface on failure, logged here).
pub(crate) fn rebuild_gdd_cursor_future(gdd: GddV1, store_handle: ResourcesHandle, document_id: DocumentId, had_oracle: bool) -> Message {
	let future = async move {
		let declarations = gdd.declarations(&store_handle).await;
		let interface = match gdd.registry().to_runtime_with_full_metadata(&declarations) {
			Ok((network, node_entries, network_entries)) => match build_interface_from_storage(network, node_entries, network_entries) {
				Ok(interface) => Some(Box::new(interface)),
				Err(error) => {
					log::error!("Gdd undo/redo rebuild for {document_id:?}: failed to build interface: {error}");
					None
				}
			},
			Err(error) => {
				log::error!("Gdd undo/redo rebuild for {document_id:?}: failed to convert registry to runtime: {error}");
				None
			}
		};

		Message::Portfolio(PortfolioMessage::GddUndoRedoRebuilt { document_id, had_oracle, interface })
	};
	future.into()
}

/// Core of the `.gdd` open: archive -> working copy -> `Gdd` -> runtime interface. The registry build is
/// authoritative; the embedded legacy blob is the soak oracle and the fallback if the build fails.
/// Returns `None` only if neither the build nor the legacy fallback worked.
async fn build_document_from_gdd(path: Option<&std::path::Path>, content: &[u8], store_handle: &impl ResourceStorage, document_id: DocumentId, validate: bool) -> Option<DocumentMessageHandler> {
	let (container, _exists) = match build_per_document_container(path).await {
		Ok(result) => result,
		Err(error) => {
			log::error!("Opening .gdd for {document_id:?}: failed to build working copy container: {error}");
			return None;
		}
	};

	let gdd = match GddV1::open_from_archive(content, container, GddV1Layout).await {
		Ok(gdd) => gdd,
		Err(error) => {
			log::error!("Opening .gdd for {document_id:?}: failed to open archive: {error}");
			return None;
		}
	};

	// Extract archived resource bytes into the global cache so declarations + runtime resolve.
	match gdd.resource_hashes().await {
		Ok(hashes) => {
			for hash in hashes {
				match gdd.read_resource(&hash).await {
					Ok(holder) => {
						store_handle.store(holder.as_slice());
					}
					Err(error) => log::error!("Opening .gdd for {document_id:?}: failed to read resource {hash}: {error}"),
				}
			}
		}
		Err(error) => log::error!("Opening .gdd for {document_id:?}: failed to list resources: {error}"),
	}

	let legacy_document = gdd
		.read_legacy_document()
		.await
		.and_then(|holder| String::from_utf8(holder.as_slice().to_vec()).ok())
		.and_then(|serialized| DocumentMessageHandler::deserialize_document(&document_migration_string_preprocessing(serialized)).ok());

	let declarations = gdd.declarations(store_handle).await;
	let interface = match gdd.registry().to_runtime_with_full_metadata(&declarations) {
		Ok((network, node_entries, network_entries)) => {
			let network_ids = network_ids_from_entries(&network_entries);
			match build_interface_from_storage(network, node_entries, network_entries) {
				Ok(mut interface) => {
					// Per-network view state lives in `session.json`, not the registry, so restore it here.
					apply_network_view_settings(&mut interface, &network_ids, gdd.network_view_settings());
					Some(interface)
				}
				Err(error) => {
					log::error!("Opening .gdd for {document_id:?}: failed to build interface: {error}");
					None
				}
			}
		}
		Err(error) => {
			log::error!("Opening .gdd for {document_id:?}: failed to convert registry to runtime: {error}");
			None
		}
	};

	// Soak compare against the legacy oracle, normalizing the benign generic-vs-resolved `call_argument`.
	if let Some(interface) = interface {
		if validate && let Some(legacy) = &legacy_document {
			let mut built = interface.document_network().clone();
			let mut oracle = legacy.network_interface.document_network().clone();
			normalize_call_arguments_for_compare(&mut built);
			normalize_call_arguments_for_compare(&mut oracle);
			if built != oracle {
				log::error!(
					"Open-.gdd divergence for {document_id:?}: registry-built document differs from the embedded legacy blob\n{}",
					diff_networks(&oracle, &built)
				);
			}
		}
		return Some(DocumentMessageHandler::from_storage(interface, gdd, String::new(), None));
	}

	log::warn!("Opening .gdd for {document_id:?}: registry build failed, falling back to embedded legacy document");
	if legacy_document.is_none() {
		log::error!("Opening .gdd for {document_id:?}: no embedded legacy document to fall back to");
	}
	legacy_document
}

/// Soak check that the reopened `.gdd`'s stored registry, converted back to a runtime network, matches
/// the legacy load. Logs divergence only (legacy stays authoritative); runs once per open.
pub(super) async fn compare_storage_against_runtime(gdd: &GddV1, legacy_network: &NodeNetwork, byte_store: &dyn LoadResource, document_id: DocumentId) {
	let declarations = gdd.declarations(byte_store).await;
	let mut candidate = match gdd.registry().to_runtime_with_metadata(&declarations) {
		Ok((network, _entries)) => network,
		Err(error) => {
			log::error!("Compare-on-open for {document_id:?}: .gdd registry failed to convert to runtime: {error}");
			return;
		}
	};

	// Normalize the benign generic-vs-resolved `call_argument` on both sides before comparing.
	let mut legacy = legacy_network.clone();
	normalize_call_arguments_for_compare(&mut legacy);
	normalize_call_arguments_for_compare(&mut candidate);

	if candidate != legacy {
		log::error!(
			"Compare-on-open divergence for {document_id:?}: the reopened .gdd does not match the legacy load\n{}",
			diff_networks(&legacy, &candidate)
		);
	}
}

/// Recursively rewrite a concrete `Context` `call_argument` back to the generic `T` storage authored, so
/// the generic-vs-resolved difference (recomputed during compilation) compares equal. Other types untouched.
fn normalize_call_arguments_for_compare(network: &mut NodeNetwork) {
	use graph_craft::Type;

	let context = graph_craft::concrete!(graphene_std::Context);
	for node in network.nodes.values_mut() {
		if node.call_argument == context {
			node.call_argument = Type::Generic(std::borrow::Cow::Borrowed("T"));
		}
		if let graph_craft::document::DocumentNodeImplementation::Network(inner) = &mut node.implementation {
			normalize_call_arguments_for_compare(inner);
		}
	}
}
