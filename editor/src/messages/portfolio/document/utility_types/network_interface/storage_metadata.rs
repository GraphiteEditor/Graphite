//! Bridge between `NodeNetworkInterface` and `document-graph-storage`'s `NodeMetadataSource` trait.
//! Conversion round-trip tests live in `storage_metadata_tests`.
//!
//! The trait impl lives on the [`StorageMetadataView`] wrapper (not on `NodeNetworkInterface`
//! directly) because several trait method names collide with inherent methods, and Rust silently
//! resolves bare calls to the inherent ones.

use std::collections::{BTreeMap, HashMap};

use document_graph_storage::attr::session;
use document_graph_storage::{InputMetadataEntry, NetworkMetadataEntry, NodeMetadataEntry, NodeMetadataSource, Position, Value, from_value, to_value};
use glam::IVec2;
use graph_craft::document::{DocumentNodeImplementation, NetworkLocation, NodeId, NodeNetwork, OwnedNetworkLocation};
use graphene_std::vector::style::RenderMode;

use super::{
	DocumentNodePersistentMetadata, DocumentNodeTransientMetadata, InputMetadata, InputPersistentMetadata, LayerPosition, NavigationMetadata, NodeNetworkInterface, NodeNetworkMetadata,
	NodeNetworkPersistentMetadata, NodePersistentMetadata, NodePosition, NodeTypePersistentMetadata, PTZ, Previewing,
};
use crate::messages::portfolio::document::overlays::utility_types::OverlaysVisibilitySettings;
use crate::messages::portfolio::document::utility_types::misc::SnappingState;
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;

/// Fired when the storage entry list disagrees with the converted `NodeNetwork`.
#[derive(Debug, thiserror::Error)]
pub enum InterfaceRebuildError {
	#[error("input_metadata length {got} does not match inputs length {expected} for node {node:?} in network {network:?}")]
	InputMetadataLengthMismatch {
		node: NodeId,
		network: OwnedNetworkLocation,
		expected: usize,
		got: usize,
	},
}

/// Per-peer view settings persisted in `session.json` under `ui::doc::*` (viewport view, render mode,
/// overlay/ruler visibility, snapping, collapsed layers). Not part of the registry/CRDT/history; see
/// [`DocumentSettings::to_view_map`].
pub struct DocumentSettings<'a> {
	pub document_ptz: &'a PTZ,
	pub render_mode: &'a RenderMode,
	pub overlays_visibility: &'a OverlaysVisibilitySettings,
	pub rulers_visible: bool,
	pub snapping_state: &'a SnappingState,
	pub collapsed: &'a CollapsedLayers,
}

/// Adapts a `&NodeNetworkInterface` to `document-graph-storage`'s `NodeMetadataSource` (node/network metadata
/// only; document-level view settings live in `session.json`, not the registry).
pub struct StorageMetadataView<'a> {
	interface: &'a NodeNetworkInterface,
}

impl<'a> StorageMetadataView<'a> {
	pub fn new(interface: &'a NodeNetworkInterface) -> Self {
		Self { interface }
	}
}

impl StorageMetadataView<'_> {
	fn network_persistent(&self, network: NetworkLocation) -> Option<&NodeNetworkPersistentMetadata> {
		Some(&self.interface.network_metadata.nested_metadata(network)?.persistent_metadata)
	}

	fn persistent(&self, network: NetworkLocation, local_id: NodeId) -> Option<&DocumentNodePersistentMetadata> {
		Some(&self.network_persistent(network)?.node_metadata.get(&local_id)?.persistent_metadata)
	}

	fn input_persistent(&self, network: NetworkLocation, local_id: NodeId, input_index: usize) -> Option<&InputPersistentMetadata> {
		Some(&self.persistent(network, local_id)?.input_metadata.get(input_index)?.persistent_metadata)
	}

	fn subnetwork_persistent(&self, network: NetworkLocation) -> Option<&NodeNetworkPersistentMetadata> {
		if network == NetworkLocation::ROOT {
			return None;
		}
		self.network_persistent(network)
	}
}

impl NodeMetadataSource for StorageMetadataView<'_> {
	fn position(&self, network: NetworkLocation, local_id: NodeId) -> Option<Position> {
		Some(position_from_runtime(&self.persistent(network, local_id)?.node_type_metadata))
	}

	fn is_layer(&self, network: NetworkLocation, local_id: NodeId) -> bool {
		self.persistent(network, local_id).is_some_and(|p| matches!(p.node_type_metadata, NodeTypePersistentMetadata::Layer(_)))
	}

	fn display_name(&self, network: NetworkLocation, local_id: NodeId) -> Option<&str> {
		Some(self.persistent(network, local_id)?.display_name.as_str())
	}

	fn locked(&self, network: NetworkLocation, local_id: NodeId) -> bool {
		self.persistent(network, local_id).is_some_and(|p| p.locked)
	}

	fn pinned(&self, network: NetworkLocation, local_id: NodeId) -> bool {
		self.persistent(network, local_id).is_some_and(|p| p.pinned)
	}

	fn input_name(&self, network: NetworkLocation, local_id: NodeId, input_index: usize) -> Option<&str> {
		Some(self.input_persistent(network, local_id, input_index)?.input_name.as_str())
	}

	fn input_description(&self, network: NetworkLocation, local_id: NodeId, input_index: usize) -> Option<&str> {
		Some(self.input_persistent(network, local_id, input_index)?.input_description.as_str())
	}

	fn widget_override(&self, network: NetworkLocation, local_id: NodeId, input_index: usize) -> Option<&str> {
		self.input_persistent(network, local_id, input_index)?.widget_override.as_deref()
	}

	fn input_data(&self, network: NetworkLocation, local_id: NodeId, input_index: usize) -> HashMap<String, Value> {
		self.input_persistent(network, local_id, input_index)
			.map(|p| p.input_data.iter().map(|(key, value)| (key.clone(), value.clone().into())).collect())
			.unwrap_or_default()
	}

	fn output_names(&self, network: NetworkLocation, local_id: NodeId) -> Vec<String> {
		self.persistent(network, local_id).map(|p| p.output_names.clone()).unwrap_or_default()
	}

	fn output_descriptions(&self, network: NetworkLocation, local_id: NodeId) -> Vec<String> {
		self.persistent(network, local_id).map(|p| p.output_descriptions.clone()).unwrap_or_default()
	}

	fn import_metadata(&self, network: NetworkLocation) -> Vec<InputMetadataEntry> {
		self.subnetwork_persistent(network)
			.map(|persistent| persistent.import_metadata.iter().map(input_metadata_entry_from_runtime).collect())
			.unwrap_or_default()
	}

	fn export_names(&self, network: NetworkLocation) -> Vec<String> {
		self.subnetwork_persistent(network).map(|persistent| persistent.export_names.clone()).unwrap_or_default()
	}

	fn export_descriptions(&self, network: NetworkLocation) -> Vec<String> {
		self.subnetwork_persistent(network).map(|persistent| persistent.export_descriptions.clone()).unwrap_or_default()
	}

	fn storage_node_id(&self, network: NetworkLocation, local_id: NodeId) -> Option<document_graph_storage::NodeId> {
		self.persistent(network, local_id)?.storage_id.map(document_graph_storage::NodeId)
	}
	fn reference(&self, network: NetworkLocation) -> Option<&str> {
		self.network_persistent(network)?.reference.as_deref()
	}

	fn pinned_order(&self, network: NetworkLocation) -> Vec<NodeId> {
		self.network_persistent(network).map(|persistent| persistent.pinned_node_order.clone()).unwrap_or_default()
	}
}

impl DocumentSettings<'_> {
	/// Serialize the per-peer view settings into the `ui::doc::*`-keyed map persisted in `session.json`
	/// (via `Gdd::set_view_settings`). A field that fails to serialize is skipped, not fatal.
	pub fn to_view_map(&self) -> BTreeMap<String, Value> {
		let entries = [
			(session::doc::PTZ, to_value(self.document_ptz)),
			(session::doc::RENDER_MODE, to_value(self.render_mode)),
			(session::doc::OVERLAYS, to_value(self.overlays_visibility)),
			(session::doc::RULERS_VISIBLE, to_value(&self.rulers_visible)),
			(session::doc::SNAPPING, to_value(self.snapping_state)),
			(session::doc::COLLAPSED, to_value(self.collapsed)),
		];

		entries
			.into_iter()
			.filter_map(|(key, value)| match value {
				Ok(value) => Some((key.to_string(), value)),
				Err(error) => {
					log::error!("Failed to serialize document setting {key}: {error}");
					None
				}
			})
			.collect()
	}
}

/// The stored position of a node, which pairs with `is_layer` to reconstruct the runtime node type.
pub fn position_from_runtime(node_type: &NodeTypePersistentMetadata) -> Position {
	match node_type {
		NodeTypePersistentMetadata::Layer(layer) => match layer.position {
			LayerPosition::Absolute(v) => Position::Absolute([v.x, v.y]),
			LayerPosition::Stack(offset) => Position::Stack(offset),
		},
		NodeTypePersistentMetadata::Node(node) => match *node.position() {
			NodePosition::Absolute(v) => Position::Absolute([v.x, v.y]),
			NodePosition::Chain => Position::Chain,
		},
	}
}

/// Inverse of the position extraction in `NodeMetadataSource::position`.
/// `(Stack, !is_layer)` and `(Chain, is_layer)` shouldn't arise from a faithful round-trip; they fall back to a default of the matching variant.
pub fn position_to_runtime(position: Position, is_layer: bool) -> NodeTypePersistentMetadata {
	match (position, is_layer) {
		(Position::Absolute([x, y]), true) => NodeTypePersistentMetadata::layer(IVec2::new(x, y)),
		(Position::Absolute([x, y]), false) => NodeTypePersistentMetadata::node(IVec2::new(x, y)),
		(Position::Stack(offset), _) => {
			let mut metadata = NodeTypePersistentMetadata::layer(IVec2::ZERO);
			if let NodeTypePersistentMetadata::Layer(layer) = &mut metadata {
				layer.position = LayerPosition::Stack(offset);
			}
			metadata
		}
		(Position::Chain, _) => NodeTypePersistentMetadata::Node(NodePersistentMetadata::new(NodePosition::Chain)),
	}
}

/// Builds a `NodeNetworkInterface` from a `NodeNetwork` plus the metadata vecs `Registry::to_runtime_with_full_metadata` emits.
pub fn build_interface_from_storage(network: NodeNetwork, node_entries: Vec<NodeMetadataEntry>, network_entries: Vec<NetworkMetadataEntry>) -> Result<NodeNetworkInterface, InterfaceRebuildError> {
	let mut network_metadata = NodeNetworkMetadata::default();
	seed_metadata_tree(&network, &mut network_metadata);
	apply_entries_into_tree(&network, &mut network_metadata, node_entries)?;
	apply_network_entries_into_tree(&mut network_metadata, network_entries);

	Ok(NodeNetworkInterface::from_trees(network, network_metadata))
}

/// Maps each runtime network location to its stable `NetworkId`, from the entries `to_runtime_with_full_metadata` returns.
pub fn network_ids_from_entries(network_entries: &[NetworkMetadataEntry]) -> HashMap<OwnedNetworkLocation, document_graph_storage::NetworkId> {
	network_entries.iter().map(|entry| (entry.network.clone(), entry.network_id)).collect()
}

/// Collects each network's view state (node graph navigation and previewing) into a `session.json` map.
/// The map is keyed by the [`NetworkId`](document_graph_storage::NetworkId) that `network_ids` gives each runtime location.
/// Networks with default navigation and no preview get no entry.
pub fn collect_network_view_settings(
	interface: &NodeNetworkInterface,
	network_ids: &HashMap<OwnedNetworkLocation, document_graph_storage::NetworkId>,
) -> BTreeMap<document_graph_storage::NetworkId, BTreeMap<String, Value>> {
	let mut out = BTreeMap::new();

	for (network, &network_id) in network_ids {
		let Some(network_metadata) = interface.network_metadata.nested_metadata(network.as_location()) else {
			continue;
		};
		let navigation = &network_metadata.persistent_metadata.navigation_metadata;
		let default_navigation = NavigationMetadata::default();

		// Only persist nav fields that diverge from the default, so a network at its default view stays out
		// of `session.json` (the `if !settings.is_empty()` guard below skips it entirely).
		let mut settings = BTreeMap::new();
		if navigation.node_graph_ptz != default_navigation.node_graph_ptz
			&& let Ok(value) = to_value(&navigation.node_graph_ptz)
		{
			settings.insert(session::network::NAV_PTZ.to_string(), value);
		}
		if navigation.node_graph_to_viewport != default_navigation.node_graph_to_viewport
			&& let Ok(value) = to_value(&navigation.node_graph_to_viewport)
		{
			settings.insert(session::network::NAV_TRANSFORM.to_string(), value);
		}
		if navigation.node_graph_width != default_navigation.node_graph_width
			&& let Ok(value) = to_value(&navigation.node_graph_width)
		{
			settings.insert(session::network::NAV_WIDTH.to_string(), value);
		}

		// Skip the inert `Previewing::No` default so a network that has never been previewed stays empty.
		if !matches!(network_metadata.persistent_metadata.previewing, Previewing::No)
			&& let Ok(value) = to_value(&network_metadata.persistent_metadata.previewing)
		{
			settings.insert(session::network::PREVIEWING.to_string(), value);
		}

		if !settings.is_empty() {
			out.insert(network_id, settings);
		}
	}

	out
}

/// Applies the view state stored in `session.json` onto `interface`. The inverse of [`collect_network_view_settings`].
pub fn apply_network_view_settings(
	interface: &mut NodeNetworkInterface,
	network_ids: &HashMap<OwnedNetworkLocation, document_graph_storage::NetworkId>,
	network_view_settings: &BTreeMap<document_graph_storage::NetworkId, BTreeMap<String, Value>>,
) {
	for (network, network_id) in network_ids {
		let Some(settings) = network_view_settings.get(network_id) else { continue };
		let Some(view_state) = interface.located_view_state_mut(network.as_location()) else {
			continue;
		};
		let navigation = view_state.navigation;

		if let Some(value) = settings.get(session::network::NAV_PTZ)
			&& let Ok(ptz) = from_value::<PTZ>(value)
		{
			navigation.node_graph_ptz = ptz;
		}
		if let Some(value) = settings.get(session::network::NAV_TRANSFORM)
			&& let Ok(transform) = from_value(value)
		{
			navigation.node_graph_to_viewport = transform;
		}
		if let Some(value) = settings.get(session::network::NAV_WIDTH)
			&& let Ok(width) = from_value(value)
		{
			navigation.node_graph_width = width;
		}

		if let Some(value) = settings.get(session::network::PREVIEWING)
			&& let Ok(previewing) = from_value::<Previewing>(value)
		{
			*view_state.previewing = match previewing.previewed() {
				Some(previewed) if !view_state.nodes.contains_key(&previewed.node_id) => Previewing::No,
				_ => previewing,
			};
		}
	}
}

/// Entries with an unresolved location are skipped with a warning, since that is usually a stale location and not corruption.
fn apply_network_entries_into_tree(metadata: &mut NodeNetworkMetadata, entries: Vec<NetworkMetadataEntry>) {
	for entry in entries {
		let Some(network_metadata) = metadata.nested_metadata_mut(entry.network.as_location()) else {
			log::warn!("apply_network_entries_into_tree: network at {:?} not found, skipping", entry.network);
			continue;
		};

		if let Some(reference) = entry.reference {
			network_metadata.persistent_metadata.reference = Some(reference);
		}
		network_metadata.persistent_metadata.pinned_node_order = entry.pinned_order;
		network_metadata.persistent_metadata.import_metadata = entry.import_metadata.into_iter().map(input_metadata_entry_to_runtime).collect();
		network_metadata.persistent_metadata.export_names = entry.export_names;
		network_metadata.persistent_metadata.export_descriptions = entry.export_descriptions;
	}
}

/// Gives every node of `network` and its definitions table a default `DocumentNodeMetadata` entry, at every nesting level.
/// The editor requires `NodeNetworkPersistentMetadata::node_metadata` to hold every node.
fn seed_metadata_tree(network: &NodeNetwork, metadata: &mut NodeNetworkMetadata) {
	seed_network_metadata(network, metadata);

	for (id, definition) in &network.definitions {
		let definition_metadata = metadata.persistent_metadata.definitions.entry(*id).or_default();
		seed_network_metadata(definition, definition_metadata);
	}
}

fn seed_network_metadata(network: &NodeNetwork, metadata: &mut NodeNetworkMetadata) {
	for (&local_id, node) in &network.nodes {
		let node_metadata = metadata.persistent_metadata.node_metadata.entry(local_id).or_default();

		if let DocumentNodeImplementation::Network(nested) = &node.implementation {
			let child = node_metadata.persistent_metadata.network_metadata.get_or_insert_with(NodeNetworkMetadata::default);
			seed_network_metadata(nested, child);
		}
	}
}

/// Patches entries onto the metadata tree previously seeded by [`seed_metadata_tree`].
/// Entries with a stale `(network, local_id)` are skipped with a warning.
/// An `input_metadata` length mismatch returns an `InterfaceRebuildError`.
fn apply_entries_into_tree(network: &NodeNetwork, metadata: &mut NodeNetworkMetadata, entries: Vec<NodeMetadataEntry>) -> Result<(), InterfaceRebuildError> {
	// Group entries by network so we do one nested_metadata_mut lookup per network.
	let mut by_network: HashMap<OwnedNetworkLocation, Vec<NodeMetadataEntry>> = HashMap::new();
	for entry in entries {
		by_network.entry(entry.network.clone()).or_default().push(entry);
	}

	for (location, entries) in by_network {
		let nested_network = network.located_network(location.as_location());

		let Some(network_metadata) = metadata.nested_metadata_mut(location.as_location()) else {
			log::warn!("apply_entries_into_tree: network at {location:?} not found, skipping {} entries", entries.len());
			continue;
		};

		for entry in entries {
			let Some(document_node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(&entry.local_id) else {
				log::warn!("apply_entries_into_tree: node {:?} not seeded under network {location:?}, skipping", entry.local_id);
				continue;
			};

			let persistent = &mut document_node_metadata.persistent_metadata;

			// Pins the node to the identity storage holds for it, so the next conversion reuses it rather
			// than re-deriving one from the node's location.
			persistent.storage_id = Some(entry.storage_id.0);

			if let Some(position) = entry.position {
				persistent.node_type_metadata = position_to_runtime(position, entry.is_layer);
			} else if entry.is_layer {
				persistent.node_type_metadata = NodeTypePersistentMetadata::layer(IVec2::ZERO);
			}

			if let Some(name) = entry.display_name {
				persistent.display_name = name;
			}
			persistent.locked = entry.locked;
			persistent.pinned = entry.pinned;

			// The converted runtime is the source of truth for input count; drift indicates a bug worth surfacing.
			let expected_inputs = nested_network.and_then(|net| net.nodes.get(&entry.local_id)).map(|doc_node| doc_node.inputs.len());
			if let Some(expected) = expected_inputs
				&& expected != entry.input_metadata.len()
			{
				return Err(InterfaceRebuildError::InputMetadataLengthMismatch {
					node: entry.local_id,
					network: location.clone(),
					expected,
					got: entry.input_metadata.len(),
				});
			}

			persistent.input_metadata = entry.input_metadata.into_iter().map(input_metadata_entry_to_runtime).collect();

			if !entry.output_names.is_empty() {
				persistent.output_names = entry.output_names;
			}
			if !entry.output_descriptions.is_empty() {
				persistent.output_descriptions = entry.output_descriptions;
			}

			document_node_metadata.transient_metadata = DocumentNodeTransientMetadata::default();
		}
	}

	Ok(())
}

pub(super) fn input_metadata_entry_from_runtime(input_metadata: &InputMetadata) -> InputMetadataEntry {
	let persistent_metadata = &input_metadata.persistent_metadata;
	let non_empty = |value: &str| (!value.is_empty()).then(|| value.to_string());
	InputMetadataEntry {
		input_name: non_empty(&persistent_metadata.input_name),
		input_description: non_empty(&persistent_metadata.input_description),
		widget_override: persistent_metadata.widget_override.as_deref().and_then(non_empty),
		input_data: persistent_metadata.input_data.iter().map(|(key, value)| (key.clone(), value.clone().into())).collect(),
	}
}

/// Storage-side `None` restores as `""` (the runtime's "unset" sentinel for `input_name` / `input_description`).
fn input_metadata_entry_to_runtime(entry: InputMetadataEntry) -> InputMetadata {
	InputMetadata {
		persistent_metadata: InputPersistentMetadata {
			input_name: entry.input_name.unwrap_or_default(),
			input_description: entry.input_description.unwrap_or_default(),
			widget_override: entry.widget_override,
			input_data: entry.input_data.into_iter().map(|(key, value)| (key, value.into())).collect(),
		},
	}
}
