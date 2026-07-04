//! Bridge between `NodeNetworkInterface` and `graph-storage`'s `NodeMetadataSource` trait.
//! Conversion round-trip tests live in `storage_metadata_tests`.
//!
//! The trait impl lives on the [`StorageMetadataView`] wrapper (not on `NodeNetworkInterface`
//! directly) because several trait method names collide with inherent methods, and Rust silently
//! resolves bare calls to the inherent ones.

use std::collections::{BTreeMap, HashMap};

use glam::IVec2;
use graph_craft::document::{DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_storage::attr::session;
use graph_storage::{InputMetadataEntry, NetworkMetadataEntry, NodeMetadataEntry, NodeMetadataSource, Position};
use graphene_std::vector::style::RenderMode;

use super::memo_network::MemoNetwork;
use super::{
	DocumentNodePersistentMetadata, DocumentNodeTransientMetadata, InputMetadata, InputPersistentMetadata, LayerPosition, NavigationMetadata, NodeNetworkInterface, NodeNetworkMetadata,
	NodePersistentMetadata, NodePosition, NodeTypePersistentMetadata, PTZ, Previewing,
};
use crate::messages::portfolio::document::overlays::utility_types::OverlaysVisibilitySettings;
use crate::messages::portfolio::document::utility_types::misc::SnappingState;
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;

/// Fired when the storage entry list disagrees with the converted `NodeNetwork`.
#[derive(Debug, thiserror::Error)]
pub enum InterfaceRebuildError {
	#[error("input_metadata length {got} does not match inputs length {expected} for node {node:?} in network {network_path:?}")]
	InputMetadataLengthMismatch { node: NodeId, network_path: Vec<NodeId>, expected: usize, got: usize },
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

/// Adapts a `&NodeNetworkInterface` to `graph-storage`'s `NodeMetadataSource` (node/network metadata
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
	fn persistent(&self, network_path: &[NodeId], local_id: NodeId) -> Option<&DocumentNodePersistentMetadata> {
		Some(&self.interface.node_metadata(&local_id, network_path)?.persistent_metadata)
	}

	fn input_persistent(&self, network_path: &[NodeId], local_id: NodeId, input_index: usize) -> Option<&InputPersistentMetadata> {
		Some(&self.persistent(network_path, local_id)?.input_metadata.get(input_index)?.persistent_metadata)
	}
}

impl NodeMetadataSource for StorageMetadataView<'_> {
	fn position(&self, network_path: &[NodeId], local_id: NodeId) -> Option<Position> {
		match &self.persistent(network_path, local_id)?.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer) => match layer.position {
				LayerPosition::Absolute(v) => Some(Position::Absolute([v.x, v.y])),
				LayerPosition::Stack(offset) => Some(Position::Stack(offset)),
			},
			NodeTypePersistentMetadata::Node(node) => match *node.position() {
				NodePosition::Absolute(v) => Some(Position::Absolute([v.x, v.y])),
				NodePosition::Chain => Some(Position::Chain),
			},
		}
	}

	fn is_layer(&self, network_path: &[NodeId], local_id: NodeId) -> bool {
		self.persistent(network_path, local_id)
			.is_some_and(|p| matches!(p.node_type_metadata, NodeTypePersistentMetadata::Layer(_)))
	}

	fn display_name(&self, network_path: &[NodeId], local_id: NodeId) -> Option<&str> {
		Some(self.persistent(network_path, local_id)?.display_name.as_str())
	}

	fn locked(&self, network_path: &[NodeId], local_id: NodeId) -> bool {
		self.persistent(network_path, local_id).is_some_and(|p| p.locked)
	}

	fn pinned(&self, network_path: &[NodeId], local_id: NodeId) -> bool {
		self.persistent(network_path, local_id).is_some_and(|p| p.pinned)
	}

	fn input_name(&self, network_path: &[NodeId], local_id: NodeId, input_index: usize) -> Option<&str> {
		Some(self.input_persistent(network_path, local_id, input_index)?.input_name.as_str())
	}

	fn input_description(&self, network_path: &[NodeId], local_id: NodeId, input_index: usize) -> Option<&str> {
		Some(self.input_persistent(network_path, local_id, input_index)?.input_description.as_str())
	}

	fn widget_override(&self, network_path: &[NodeId], local_id: NodeId, input_index: usize) -> Option<&str> {
		self.input_persistent(network_path, local_id, input_index)?.widget_override.as_deref()
	}

	fn input_data(&self, network_path: &[NodeId], local_id: NodeId, input_index: usize) -> HashMap<String, serde_json::Value> {
		self.input_persistent(network_path, local_id, input_index).map(|p| p.input_data.clone()).unwrap_or_default()
	}

	fn output_names(&self, network_path: &[NodeId], local_id: NodeId) -> Vec<String> {
		self.persistent(network_path, local_id).map(|p| p.output_names.clone()).unwrap_or_default()
	}

	fn reference(&self, network_path: &[NodeId]) -> Option<&str> {
		let network_metadata = self.interface.network_metadata.nested_metadata(network_path)?;
		network_metadata.persistent_metadata.reference.as_deref()
	}
}

impl DocumentSettings<'_> {
	/// Serialize the per-peer view settings into the `ui::doc::*`-keyed map persisted in `session.json`
	/// (via `Gdd::set_view_settings`). A field that fails to serialize is skipped, not fatal.
	pub fn to_view_map(&self) -> BTreeMap<String, serde_json::Value> {
		let entries = [
			(session::doc::PTZ, serde_json::to_value(self.document_ptz)),
			(session::doc::RENDER_MODE, serde_json::to_value(self.render_mode)),
			(session::doc::OVERLAYS, serde_json::to_value(self.overlays_visibility)),
			(session::doc::RULERS_VISIBLE, serde_json::to_value(self.rulers_visible)),
			(session::doc::SNAPPING, serde_json::to_value(self.snapping_state)),
			(session::doc::COLLAPSED, serde_json::to_value(self.collapsed)),
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
///
/// Sets private fields directly (rather than via public setters) because we're constructing a fresh self-consistent snapshot;
/// the setters' transient-cache invalidation isn't needed.
pub fn build_interface_from_storage(network: NodeNetwork, node_entries: Vec<NodeMetadataEntry>, network_entries: Vec<NetworkMetadataEntry>) -> Result<NodeNetworkInterface, InterfaceRebuildError> {
	let mut network_metadata = NodeNetworkMetadata::default();
	seed_metadata_tree(&network, &mut network_metadata);
	apply_entries_into_tree(&network, &mut network_metadata, node_entries)?;
	apply_network_entries_into_tree(&mut network_metadata, network_entries);

	let interface = NodeNetworkInterface {
		network: MemoNetwork::new(network),
		network_metadata,
		..Default::default()
	};
	Ok(interface)
}

/// Build the runtime-`network_path` -> stable-`NetworkId` map from the `NetworkMetadataEntry`s that
/// `to_runtime_with_full_metadata` emits, so the open path can apply per-network view settings.
pub fn network_ids_from_entries(network_entries: &[NetworkMetadataEntry]) -> HashMap<Vec<NodeId>, graph_storage::NetworkId> {
	network_entries.iter().map(|entry| (entry.network_path.clone(), entry.network_id)).collect()
}

/// Collect the per-network, per-peer view state (node-graph nav + previewing) from `interface` into a
/// `session.json` map keyed by the stable storage [`NetworkId`](graph_storage::NetworkId), which
/// `network_ids` resolves from each runtime `network_path`. Networks at their default nav with no preview
/// produce no entry.
pub fn collect_network_view_settings(
	interface: &NodeNetworkInterface,
	network_ids: &HashMap<Vec<NodeId>, graph_storage::NetworkId>,
) -> BTreeMap<graph_storage::NetworkId, BTreeMap<String, serde_json::Value>> {
	let mut out = BTreeMap::new();

	for (network_path, &network_id) in network_ids {
		let Some(network_metadata) = interface.network_metadata.nested_metadata(network_path) else {
			continue;
		};
		let navigation = &network_metadata.persistent_metadata.navigation_metadata;
		let default_navigation = NavigationMetadata::default();

		// Only persist nav fields that diverge from the default, so a network at its default view stays out
		// of `session.json` (the `if !settings.is_empty()` guard below skips it entirely).
		let mut settings = BTreeMap::new();
		if navigation.node_graph_ptz != default_navigation.node_graph_ptz
			&& let Ok(value) = serde_json::to_value(navigation.node_graph_ptz)
		{
			settings.insert(session::network::NAV_PTZ.to_string(), value);
		}
		if navigation.node_graph_to_viewport != default_navigation.node_graph_to_viewport
			&& let Ok(value) = serde_json::to_value(navigation.node_graph_to_viewport)
		{
			settings.insert(session::network::NAV_TRANSFORM.to_string(), value);
		}
		if navigation.node_graph_width != default_navigation.node_graph_width
			&& let Ok(value) = serde_json::to_value(navigation.node_graph_width)
		{
			settings.insert(session::network::NAV_WIDTH.to_string(), value);
		}

		// Skip the inert `Previewing::No` default so a network that has never been previewed stays empty.
		if !matches!(network_metadata.persistent_metadata.previewing, Previewing::No)
			&& let Ok(value) = serde_json::to_value(network_metadata.persistent_metadata.previewing)
		{
			settings.insert(session::network::PREVIEWING.to_string(), value);
		}

		if !settings.is_empty() {
			out.insert(network_id, settings);
		}
	}

	out
}

/// Apply persisted per-network view state (node-graph nav + previewing) from `session.json` onto
/// `interface`. Inverse of [`collect_network_view_settings`]: `network_ids` resolves each runtime
/// `network_path` to its [`NetworkId`](graph_storage::NetworkId), and the matching inner map is decoded
/// back onto the network's navigation/previewing metadata.
pub fn apply_network_view_settings(
	interface: &mut NodeNetworkInterface,
	network_ids: &HashMap<Vec<NodeId>, graph_storage::NetworkId>,
	network_view_settings: &BTreeMap<graph_storage::NetworkId, BTreeMap<String, serde_json::Value>>,
) {
	for (network_path, network_id) in network_ids {
		let Some(settings) = network_view_settings.get(network_id) else { continue };
		let Some(network_metadata) = interface.network_metadata.nested_metadata_mut(network_path) else {
			continue;
		};
		let persistent = &mut network_metadata.persistent_metadata;

		if let Some(value) = settings.get(session::network::NAV_PTZ)
			&& let Ok(ptz) = serde_json::from_value::<PTZ>(value.clone())
		{
			persistent.navigation_metadata.node_graph_ptz = ptz;
		}
		if let Some(value) = settings.get(session::network::NAV_TRANSFORM)
			&& let Ok(transform) = serde_json::from_value(value.clone())
		{
			persistent.navigation_metadata.node_graph_to_viewport = transform;
		}
		if let Some(value) = settings.get(session::network::NAV_WIDTH)
			&& let Ok(width) = serde_json::from_value(value.clone())
		{
			persistent.navigation_metadata.node_graph_width = width;
		}
		if let Some(value) = settings.get(session::network::PREVIEWING)
			&& let Ok(previewing) = serde_json::from_value::<Previewing>(value.clone())
		{
			persistent.previewing = previewing;
		}
	}
}

/// Entries whose `network_path` doesn't resolve are skipped with a warning (per-network drift is more often a stale path than corruption).
fn apply_network_entries_into_tree(metadata: &mut NodeNetworkMetadata, entries: Vec<NetworkMetadataEntry>) {
	for entry in entries {
		let Some(network_metadata) = metadata.nested_metadata_mut(&entry.network_path) else {
			log::warn!("apply_network_entries_into_tree: nested network at {:?} not found, skipping", entry.network_path);
			continue;
		};

		if let Some(reference) = entry.reference {
			network_metadata.persistent_metadata.reference = Some(reference);
		}
	}
}

/// Walks `network` recursively, ensuring `metadata` has a default `DocumentNodeMetadata` slot for every node at every nesting level.
/// Mirrors the editor invariant that `NodeNetworkPersistentMetadata::node_metadata` contains every document-node key.
fn seed_metadata_tree(network: &NodeNetwork, metadata: &mut NodeNetworkMetadata) {
	for (&local_id, node) in &network.nodes {
		let node_metadata = metadata.persistent_metadata.node_metadata.entry(local_id).or_default();

		if let DocumentNodeImplementation::Network(nested) = &node.implementation {
			let child = node_metadata.persistent_metadata.network_metadata.get_or_insert_with(NodeNetworkMetadata::default);
			seed_metadata_tree(nested, child);
		}
	}
}

/// Patches entries onto the metadata tree previously seeded by [`seed_metadata_tree`].
/// Entries with stale `(network_path, local_id)` are skipped with a warning; `input_metadata` length mismatches escalate to `InterfaceRebuildError`.
fn apply_entries_into_tree(network: &NodeNetwork, metadata: &mut NodeNetworkMetadata, entries: Vec<NodeMetadataEntry>) -> Result<(), InterfaceRebuildError> {
	// Group entries by network_path so we do one nested_metadata_mut lookup per network.
	let mut by_path: HashMap<Vec<NodeId>, Vec<NodeMetadataEntry>> = HashMap::new();
	for entry in entries {
		by_path.entry(entry.network_path.clone()).or_default().push(entry);
	}

	for (path, entries) in by_path {
		let nested_network = nested_network_at(network, &path);

		let Some(network_metadata) = metadata.nested_metadata_mut(&path) else {
			log::warn!("apply_entries_into_tree: nested network at {path:?} not found, skipping {} entries", entries.len());
			continue;
		};

		for entry in entries {
			let Some(document_node_metadata) = network_metadata.persistent_metadata.node_metadata.get_mut(&entry.local_id) else {
				log::warn!("apply_entries_into_tree: node {:?} not seeded under network {path:?}, skipping", entry.local_id);
				continue;
			};

			let persistent = &mut document_node_metadata.persistent_metadata;

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
					network_path: path.clone(),
					expected,
					got: entry.input_metadata.len(),
				});
			}

			persistent.input_metadata = entry.input_metadata.into_iter().map(input_metadata_entry_to_runtime).collect();

			if !entry.output_names.is_empty() {
				persistent.output_names = entry.output_names;
			}

			document_node_metadata.transient_metadata = DocumentNodeTransientMetadata::default();
		}
	}

	Ok(())
}

/// `None` when the path is invalid; callers treat that as "skip silently" since the surrounding `nested_metadata_mut` already warns.
fn nested_network_at<'a>(root: &'a NodeNetwork, path: &[NodeId]) -> Option<&'a NodeNetwork> {
	let mut current = root;
	for segment in path {
		let node = current.nodes.get(segment)?;
		let DocumentNodeImplementation::Network(nested) = &node.implementation else { return None };
		current = nested;
	}
	Some(current)
}

/// Storage-side `None` restores as `""` (the runtime's "unset" sentinel for `input_name` / `input_description`).
fn input_metadata_entry_to_runtime(entry: InputMetadataEntry) -> InputMetadata {
	InputMetadata {
		persistent_metadata: InputPersistentMetadata {
			input_name: entry.input_name.unwrap_or_default(),
			input_description: entry.input_description.unwrap_or_default(),
			widget_override: entry.widget_override,
			input_data: entry.input_data,
		},
		..Default::default()
	}
}
