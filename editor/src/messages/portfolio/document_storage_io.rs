//! Document store I/O spawned by `PortfolioMessageHandler` as `FutureMessage`s. A future writes only into a
//! container no registered document owns yet; the message loop is the sole writer once a document is
//! registered, and a completion for a document closed meanwhile removes the store entry.

use super::document::DocumentMessageHandler;
use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::document::utility_types::error::EditorError;
use super::document::utility_types::network_interface::OutputConnector;
use super::document::utility_types::network_interface::storage_metadata::{apply_network_view_settings, build_interface_from_storage, network_ids_from_entries};
use super::document_migration::*;
use super::portfolio_message::PortfolioMessage;
use crate::application::{GRAPHITE_GIT_COMMIT_HASH, generate_uuid};
use crate::messages::message::Message;
use crate::messages::portfolio::document::utility_types::misc::DocumentId;
use crate::messages::resource_storage::ResourcesHandle;
use document_container::store::{DocumentKey, DocumentStore, MemoryStore};
use document_container::{AnyContainer, AsyncContainer};
use document_format::{Error as DocumentFormatError, GddV1, GddV1Layout, Layout, ReadError};
use document_graph_storage::{Declarations, PeerId};
use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceStorage};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct DocumentStoreHandle(Arc<dyn DocumentStore>);

impl DocumentStoreHandle {
	pub fn new(store: Arc<dyn DocumentStore>) -> Self {
		Self(store)
	}
}

impl std::ops::Deref for DocumentStoreHandle {
	type Target = dyn DocumentStore;

	fn deref(&self) -> &(dyn DocumentStore + 'static) {
		self.0.as_ref()
	}
}

impl Default for DocumentStoreHandle {
	fn default() -> Self {
		Self(Arc::new(MemoryStore::default()))
	}
}

impl std::fmt::Debug for DocumentStoreHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DocumentStoreHandle").finish_non_exhaustive()
	}
}

pub(crate) fn document_key(document_id: DocumentId) -> DocumentKey {
	DocumentKey(document_id.0)
}

pub(crate) fn legacy_path() -> &'static str {
	GddV1Layout.legacy_path()
}

pub(super) async fn list_stored_documents(store: DocumentStoreHandle) -> Message {
	let document_ids = match store.list().await {
		Ok(keys) => Some(keys.into_iter().map(|key| DocumentId(key.0)).collect()),
		Err(error) => {
			log::error!("Listing the document store failed: {error}");
			None
		}
	};
	PortfolioMessage::StoredDocumentsListed { document_ids }.into()
}

pub(super) async fn load_stored_document(store: DocumentStoreHandle, document_id: DocumentId, resources: ResourcesHandle, reset_nodes: bool, legacy_only: bool) -> Message {
	let (document, gdd) = match store.open(document_key(document_id)).await {
		Ok(container) => load_from_container(container, document_id, &resources, reset_nodes, legacy_only).await,
		Err(error) => {
			log::error!("Opening the stored document {document_id:?} failed: {error}");
			(None, None)
		}
	};
	let declarations = load_declarations(gdd.as_ref(), &resources).await;
	PortfolioMessage::StoredDocumentLoaded {
		document_id,
		document: document.map(Box::new),
		gdd: gdd.map(Box::new),
		declarations,
	}
	.into()
}

/// Opens the container for a document created in this session.
pub(super) async fn attach_container(store: DocumentStoreHandle, document_id: DocumentId, resources: ResourcesHandle, legacy_only: bool) -> Message {
	let container = match store.open(document_key(document_id)).await {
		Ok(container) => container,
		Err(error) => {
			log::error!("Opening storage for {document_id:?} failed: {error}");
			return PortfolioMessage::StorageAttached {
				document_id,
				container: None,
				gdd: None,
				declarations: Declarations::new(),
			}
			.into();
		}
	};
	let gdd = if !legacy_only {
		open_storage_or_fallback_to_legacy(container.clone(), document_id).await
	} else {
		None
	};
	let declarations = load_declarations(gdd.as_ref(), &resources).await;
	PortfolioMessage::StorageAttached {
		document_id,
		container: Some(container),
		gdd: gdd.map(Box::new),
		declarations,
	}
	.into()
}

/// Opens the storage of a document that already has its container.
pub(super) async fn open_storage(container: AnyContainer, document_id: DocumentId, resources: ResourcesHandle) -> Message {
	let gdd = open_storage_or_fallback_to_legacy(container.clone(), document_id).await;
	let declarations = load_declarations(gdd.as_ref(), &resources).await;
	PortfolioMessage::StorageAttached {
		document_id,
		container: Some(container),
		gdd: gdd.map(Box::new),
		declarations,
	}
	.into()
}

pub(super) async fn remove_stored_document(store: DocumentStoreHandle, document_id: DocumentId) -> Message {
	if let Err(error) = store.remove(document_key(document_id)).await {
		log::error!("Removing the stored document {document_id:?} failed: {error}");
	}
	Message::NoOp
}

/// Materializes a `.gdd` archive into a fresh store container and loads it like a stored document.
pub(super) async fn open_document_file(
	store: DocumentStoreHandle,
	document_id: DocumentId,
	document_name: Option<String>,
	document_path: Option<PathBuf>,
	content: Vec<u8>,
	resources: ResourcesHandle,
	reset_nodes: bool,
	legacy_only: bool,
) -> Message {
	let materialized = async {
		let mut container = store.open(document_key(document_id)).await.map_err(|error| error.to_string())?;
		document_container::archive::open_auto(&content, &mut container).map_err(|error| error.to_string())?;
		Ok::<_, String>(container)
	}
	.await;
	let (document, gdd) = match materialized {
		Ok(container) => load_from_container(container, document_id, &resources, reset_nodes, legacy_only).await,
		Err(error) => {
			log::error!("Opening the document archive for {document_id:?} failed: {error}");
			(None, None)
		}
	};
	let declarations = load_declarations(gdd.as_ref(), &resources).await;
	PortfolioMessage::DocumentFileLoaded {
		document_id,
		document_name,
		document_path,
		document: document.map(Box::new),
		gdd: gdd.map(Box::new),
		declarations,
	}
	.into()
}

/// Builds the document from its legacy file, or from its storage when that file is missing or unreadable.
async fn load_from_container(container: AnyContainer, document_id: DocumentId, resources: &ResourcesHandle, reset_nodes: bool, legacy_only: bool) -> (Option<DocumentMessageHandler>, Option<GddV1>) {
	let legacy = match container.read(legacy_path()).await {
		Ok(bytes) => String::from_utf8(bytes.as_slice().to_vec())
			.map_err(|error| error.to_string())
			.and_then(|text| load_legacy_document(text, resources, reset_nodes).map_err(|error| error.to_string())),
		Err(error) => Err(error.to_string()),
	};
	match legacy {
		Ok(mut document) => {
			document.set_auto_save_state(true);
			document.container = Some(container.clone());
			let gdd = if !legacy_only { open_storage_or_fallback_to_legacy(container, document_id).await } else { None };
			(Some(document), gdd)
		}
		Err(legacy_error) => {
			let gdd = match GddV1::open_in(container.clone(), GddV1Layout).await {
				Ok(gdd) => gdd,
				Err(storage_error) => {
					log::error!("Loading {document_id:?} failed. Legacy: {legacy_error}. Storage: {storage_error}");
					return (None, None);
				}
			};
			log::warn!("Recovering {document_id:?} from storage because its legacy file could not load: {legacy_error}");
			let Some(mut document) = build_document_from_storage(gdd, resources, document_id).await else {
				return (None, None);
			};
			document.container = Some(container);
			if legacy_only {
				document.clear_storage();
			}
			(Some(document), None)
		}
	}
}

async fn open_storage_or_fallback_to_legacy(container: AnyContainer, document_id: DocumentId) -> Option<GddV1> {
	match GddV1::open_in(container.clone(), GddV1Layout).await {
		Ok(gdd) => return Some(gdd),
		Err(DocumentFormatError::MissingManifest) => {}
		// Container I/O and newer-format errors keep the existing files; anything else is broken data the legacy document replaces.
		Err(error @ (DocumentFormatError::Container(_) | DocumentFormatError::Read(ReadError::Container(_)) | DocumentFormatError::UnsupportedVersion { .. })) => {
			log::error!("Opening storage for {document_id:?} failed: {error}");
			return None;
		}
		Err(error) => log::warn!("Storage for {document_id:?} is unusable and will be rebuilt from the legacy document: {error}"),
	}
	let version = GRAPHITE_GIT_COMMIT_HASH.to_string();
	let created = async {
		for path in container.list("").await? {
			if path != legacy_path() {
				container.remove(&path).await?;
			}
		}
		GddV1::create_in(container, GddV1Layout, PeerId(generate_uuid()), document_id.0, version.clone(), version)
	}
	.await;
	match created {
		Ok(gdd) => Some(gdd),
		Err(error) => {
			log::error!("Creating storage for {document_id:?} failed: {error}");
			None
		}
	}
}

/// Decoded proto-node declarations for every node `gdd` or its history references, from the global cache.
async fn load_declarations(gdd: Option<&GddV1>, resources: &ResourcesHandle) -> Declarations {
	match gdd {
		Some(gdd) => gdd.declarations(resources).await,
		None => Declarations::new(),
	}
}

/// Storage session -> runtime interface. Resources stored in the container are hydrated into the global cache first.
async fn build_document_from_storage(gdd: GddV1, byte_store: &ResourcesHandle, document_id: DocumentId) -> Option<DocumentMessageHandler> {
	match gdd.resource_hashes().await {
		Ok(hashes) => {
			for hash in hashes {
				match gdd.read_resource(&hash).await {
					Ok(holder) => {
						byte_store.store(holder.as_slice());
					}
					Err(error) => log::error!("Opening document {document_id:?}: failed to read resource {hash}: {error}"),
				}
			}
		}
		Err(error) => log::error!("Opening document {document_id:?}: failed to list resources: {error}"),
	}

	let declarations = gdd.declarations(byte_store).await;
	let (network, node_entries, network_entries) = match gdd.registry().to_runtime_with_full_metadata(&declarations) {
		Ok(result) => result,
		Err(error) => {
			log::error!("Opening document {document_id:?}: failed to convert registry to runtime: {error}");
			return None;
		}
	};
	let network_ids = network_ids_from_entries(&network_entries);
	let mut interface = match build_interface_from_storage(network, node_entries, network_entries) {
		Ok(interface) => interface,
		Err(error) => {
			log::error!("Opening document {document_id:?}: failed to build interface: {error}");
			return None;
		}
	};
	apply_network_view_settings(&mut interface, &network_ids, gdd.network_view_settings());

	let mut document = DocumentMessageHandler::from_storage(interface, gdd, declarations, String::new(), None);
	document.finalize_storage_load();
	Some(document)
}

/// Deserializes a legacy `.graphite` document, running the migrations and importing its embedded resources.
pub(super) fn load_legacy_document(document_serialized_content: String, byte_store: &impl ResourceStorage, reset_node_definitions_on_open: bool) -> Result<DocumentMessageHandler, EditorError> {
	// Upgrade the document being opened to use fresh copies of all nodes
	let reset_node_definitions_on_open = reset_node_definitions_on_open || document_migration_reset_node_definition(&document_serialized_content);
	// Upgrade the document being opened with string replacements on the original JSON
	let document_serialized_content = document_migration_string_preprocessing(document_serialized_content);
	// Upgrade resources from being referend by hash to beeing referened by ID
	let (document_serialized_content, resource_hash_to_id_migration_map) = document_migration_replace_resources_referenced_by_hash(document_serialized_content);

	let mut document = DocumentMessageHandler::deserialize_document(&document_serialized_content)?;
	// Upgrade the document's nodes to be compatible with the latest version
	document_migration_upgrades(&mut document, reset_node_definitions_on_open);

	// Load the document's embedded resources into the resource storage
	std::mem::take(&mut document.resources.embedded).into_iter().for_each(|(hash, resource)| {
		let data: Arc<[u8]> = Arc::from(resource.as_ref());
		if ResourceHash::from(data.as_ref()) != hash {
			log::error!("Resource hash mismatch for resource with hash {hash}");
			return;
		}
		byte_store.store(&data);

		// TODO: Eventually remove this document upgrade code
		// Register any resources that were previously referenced by hash
		if let Some(id) = resource_hash_to_id_migration_map.get(&hash)
			&& !document.resources.registry.contains(id)
		{
			document.resources.registry.resolve(id, hash);
			document.resources.registry.push_source_back(id, DataSource::Embedded);
		}
	});

	// Ensure each node has the metadata for its inputs
	for (node_id, node, path) in document.network_interface.document_network().clone().recursive_nodes() {
		document.network_interface.validate_input_metadata(node_id, node, &path);
		document.network_interface.validate_output_names(node_id, node, &path);
	}

	// Ensure layers are positioned as stacks if they are upstream siblings of another layer
	document.network_interface.load_structure();
	let all_layers = LayerNodeIdentifier::ROOT_PARENT.descendants(document.network_interface.document_metadata()).collect::<Vec<_>>();
	for layer in all_layers {
		let Some((downstream_node, input_index)) = document
			.network_interface
			.outward_wires(&[])
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::primary_output(layer.to_node())))
			.and_then(|outward_wires| outward_wires.first())
			.and_then(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())))
		else {
			continue;
		};

		// If the downstream node is a layer and the input is the first input and the current layer is not in a stack
		if input_index == 0 && document.network_interface.is_layer(&downstream_node, &[]) && !document.network_interface.is_stack(&layer.to_node(), &[]) {
			// Ensure the layer is horizontally aligned with the downstream layer to prevent changing the layout of old files
			let (Some(layer_position), Some(downstream_position)) = (document.network_interface.position(&layer.to_node(), &[]), document.network_interface.position(&downstream_node, &[])) else {
				log::error!("Could not get position for layer {:?} or downstream node {} when opening file", layer.to_node(), downstream_node);
				continue;
			};

			if layer_position.x == downstream_position.x {
				document.network_interface.set_stack_position_calculated_offset(&layer.to_node(), &downstream_node, &[]);
			}
		}
	}

	document.network_interface.discard_deltas();
	document.require_whole_document_stage();

	Ok(document)
}
