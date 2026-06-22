//! Bridge between `NodeNetworkInterface` and `graph-storage`'s `NodeMetadataSource` trait, plus
//! an integration round-trip test against a demo `.graphite` document.
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
	DocumentNodePersistentMetadata, DocumentNodeTransientMetadata, InputMetadata, InputPersistentMetadata, LayerPosition, NodeNetworkInterface, NodeNetworkMetadata, NodePersistentMetadata,
	NodePosition, NodeTypePersistentMetadata, PTZ, Previewing,
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
/// map keyed by the stable storage [`NetworkId`](graph_storage::NetworkId). `network_ids` maps each
/// runtime `network_path` to its id (from the `from_runtime`/`to_runtime` conversion). This is what the
/// editor persists into `session.json` so it stays out of the CRDT/history. Networks at their default
/// nav with no preview produce no entry.
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

		let mut settings = BTreeMap::new();
		if let Ok(value) = serde_json::to_value(navigation.node_graph_ptz) {
			settings.insert(session::network::NAV_PTZ.to_string(), value);
		}
		if let Ok(value) = serde_json::to_value(navigation.node_graph_to_viewport) {
			settings.insert(session::network::NAV_TRANSFORM.to_string(), value);
		}
		if let Ok(value) = serde_json::to_value(navigation.node_graph_width) {
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

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use graph_storage::{NodeMetadataSource, PeerId, Registry};

	use super::*;
	use crate::messages::portfolio::document::document_message_handler::DocumentMessageHandler;

	/// Load a demo `.graphite` straight into a `DocumentMessageHandler` for inspection.
	fn load_demo(file_name: &str) -> DocumentMessageHandler {
		let path = format!("../demo-artwork/{file_name}");
		let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
		DocumentMessageHandler::deserialize_document(&content).unwrap_or_else(|e| panic!("Failed to deserialize {path}: {e:?}"))
	}

	/// Walk every node in every nested network and collect `(network_path, local_id)` pairs so the
	/// test can iterate every node addressable from the metadata side.
	fn collect_all_node_paths(interface: &NodeNetworkInterface) -> Vec<(Vec<NodeId>, NodeId)> {
		fn walk(interface: &NodeNetworkInterface, path: Vec<NodeId>, out: &mut Vec<(Vec<NodeId>, NodeId)>) {
			let Some(network) = interface.nested_network(&path) else { return };
			for (&local_id, node) in &network.nodes {
				out.push((path.clone(), local_id));

				if matches!(&node.implementation, graph_craft::document::DocumentNodeImplementation::Network(_)) {
					let mut child = path.clone();
					child.push(local_id);
					walk(interface, child, out);
				}
			}
		}

		let mut out = Vec::new();
		walk(interface, Vec::new(), &mut out);
		out
	}

	/// Loads a demo artwork, round-trips its `NodeNetwork + NodeNetworkInterface metadata` through
	/// `Registry`, and asserts every node's `ui::*` attributes survive unchanged.
	///
	/// Uses one demo (rather than the full set) because this is an exhaustive per-node check.
	#[test]
	fn editor_metadata_round_trip_against_demo() {
		let document = load_demo("changing-seasons.graphite");
		let interface = &document.network_interface;
		let source = StorageMetadataView::new(interface);

		let network = interface.document_network().clone();

		let conversion = Registry::convert_from_runtime(&network, &source, &Default::default(), PeerId(0)).expect("convert_from_runtime failed");
		let declarations = conversion.declarations().expect("rebuild declarations");
		let registry = conversion.registry;

		let (_converted_network, entries) = registry.to_runtime_with_metadata(&declarations).expect("to_runtime_with_metadata failed");

		// Index emitted entries by their (network_path, local_id) address.
		let entries_by_address: HashMap<(Vec<NodeId>, NodeId), &graph_storage::NodeMetadataEntry> = entries.iter().map(|e| ((e.network_path.clone(), e.local_id), e)).collect();

		let mut checked_any_position = false;
		let mut checked_any_layer = false;
		let mut checked_any_input_metadata = false;

		for (network_path, local_id) in collect_all_node_paths(interface) {
			let expected_position = source.position(&network_path, local_id);
			let expected_is_layer = source.is_layer(&network_path, local_id);
			let expected_display = source.display_name(&network_path, local_id).map(str::to_owned);
			let expected_locked = source.locked(&network_path, local_id);
			let expected_pinned = source.pinned(&network_path, local_id);
			let expected_output_names = source.output_names(&network_path, local_id);

			// Pull per-input metadata directly off the runtime side. We use the runtime invariant
			// (`input_metadata.len() == inputs.len()`) as the iteration bound rather than calling the
			// trait per index, since the trait would return `None` past the end and we want a strict
			// slot-by-slot comparison.
			let input_count = interface.document_node(&local_id, &network_path).map(|node| node.inputs.len()).unwrap_or(0);
			let any_input_metadata_present = (0..input_count).any(|i| {
				source.input_name(&network_path, local_id, i).is_some_and(|s| !s.is_empty())
					|| source.input_description(&network_path, local_id, i).is_some_and(|s| !s.is_empty())
					|| source.widget_override(&network_path, local_id, i).is_some()
					|| !source.input_data(&network_path, local_id, i).is_empty()
			});

			let any_metadata = expected_position.is_some()
				|| expected_is_layer
				|| expected_display.as_deref().is_some_and(|s| !s.is_empty())
				|| expected_locked
				|| expected_pinned
				|| any_input_metadata_present
				|| !expected_output_names.is_empty();

			if !any_metadata {
				assert!(
					entries_by_address.get(&(network_path.clone(), local_id)).is_none(),
					"node {local_id:?} in network {network_path:?} has no editor metadata but produced an entry"
				);
				continue;
			}

			let entry = entries_by_address
				.get(&(network_path.clone(), local_id))
				.unwrap_or_else(|| panic!("missing entry for node {local_id:?} in network {network_path:?}"));

			assert_eq!(entry.position, expected_position, "position mismatch for node {local_id:?} in network {network_path:?}");
			assert_eq!(entry.is_layer, expected_is_layer, "is_layer mismatch for node {local_id:?} in network {network_path:?}");
			// The trait round-trip drops empty display names (treats them as "unset") so the entry's
			// `display_name` is `None` when the runtime carries `""`. Normalize before comparing.
			let normalized_expected_display = expected_display.as_deref().filter(|s| !s.is_empty()).map(str::to_owned);
			assert_eq!(
				entry.display_name, normalized_expected_display,
				"display_name mismatch for node {local_id:?} in network {network_path:?}"
			);
			assert_eq!(entry.locked, expected_locked, "locked mismatch for node {local_id:?} in network {network_path:?}");
			assert_eq!(entry.pinned, expected_pinned, "pinned mismatch for node {local_id:?} in network {network_path:?}");
			assert_eq!(entry.output_names, expected_output_names, "output_names mismatch for node {local_id:?} in network {network_path:?}");

			// Per-input metadata: the entry's `input_metadata` vec has the same length as the node's
			// input slot count, and each slot matches the trait's per-field view (with the same
			// "empty string ↔ None" normalization as `display_name`).
			assert_eq!(
				entry.input_metadata.len(),
				input_count,
				"input_metadata length mismatch for node {local_id:?} in network {network_path:?}: entry has {}, node has {input_count}",
				entry.input_metadata.len(),
			);
			for (index, input_entry) in entry.input_metadata.iter().enumerate() {
				let expected_name = source.input_name(&network_path, local_id, index).filter(|s| !s.is_empty()).map(str::to_owned);
				let expected_description = source.input_description(&network_path, local_id, index).filter(|s| !s.is_empty()).map(str::to_owned);
				let expected_widget = source.widget_override(&network_path, local_id, index).map(str::to_owned);
				let expected_data = source.input_data(&network_path, local_id, index);

				assert_eq!(
					input_entry.input_name, expected_name,
					"input_name mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
				assert_eq!(
					input_entry.input_description, expected_description,
					"input_description mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
				assert_eq!(
					input_entry.widget_override, expected_widget,
					"widget_override mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
				assert_eq!(
					input_entry.input_data, expected_data,
					"input_data mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
			}

			if entry.position.is_some() {
				checked_any_position = true;
			}
			if entry.is_layer {
				checked_any_layer = true;
			}
			if any_input_metadata_present {
				checked_any_input_metadata = true;
			}
		}

		// Sanity: a real artwork should exercise at least these two shapes; otherwise the test is
		// just iterating empty metadata and proving nothing.
		assert!(checked_any_position, "demo artwork produced no positioned nodes — fixture is wrong or extraction is broken");
		assert!(checked_any_layer, "demo artwork produced no layer nodes — fixture is wrong or extraction is broken");
		// Demo artworks always have some custom widget overrides / input names; if not, the
		// per-input round-trip isn't actually being exercised here.
		assert!(checked_any_input_metadata, "demo artwork produced no per-input metadata — fixture is wrong or extraction is broken");
	}

	/// Full editor-side round-trip: original interface → Registry → (NodeNetwork, Vec<entry>) →
	/// freshly-built interface. Asserts the rebuilt interface presents the same `ui::*` state as
	/// the original when read through `StorageMetadataView`.
	#[test]
	fn editor_interface_rebuild_round_trip() {
		let document = load_demo("changing-seasons.graphite");
		let original = &document.network_interface;
		let original_view = StorageMetadataView::new(original);

		let network = original.document_network().clone();
		let conversion = Registry::convert_from_runtime(&network, &original_view, &Default::default(), PeerId(0)).expect("convert_from_runtime failed");
		let declarations = conversion.declarations().expect("rebuild declarations");
		let registry = conversion.registry;
		let (rebuilt_network, node_entries, network_entries) = registry.to_runtime_with_full_metadata(&declarations).expect("to_runtime_with_full_metadata failed");

		let rebuilt = build_interface_from_storage(rebuilt_network, node_entries, network_entries).expect("build_interface_from_storage failed");
		let rebuilt_view = StorageMetadataView::new(&rebuilt);

		// Every node the original carried must also resolve identically through the rebuilt view.
		// Iterating over the *rebuilt* interface verifies that the rebuild covered every node, not
		// just the ones the entries vec mentioned.
		for (network_path, local_id) in collect_all_node_paths(&rebuilt) {
			assert_eq!(
				rebuilt_view.position(&network_path, local_id),
				original_view.position(&network_path, local_id),
				"position mismatch for node {local_id:?} in network {network_path:?}"
			);
			assert_eq!(
				rebuilt_view.is_layer(&network_path, local_id),
				original_view.is_layer(&network_path, local_id),
				"is_layer mismatch for node {local_id:?} in network {network_path:?}"
			);
			// Original display names are returned by the source as-is (including `""`). After
			// round-trip the rebuilt interface also stores `""` for nodes that had no name set,
			// so this comparison is exact.
			assert_eq!(
				rebuilt_view.display_name(&network_path, local_id),
				original_view.display_name(&network_path, local_id),
				"display_name mismatch for node {local_id:?} in network {network_path:?}"
			);
			assert_eq!(
				rebuilt_view.locked(&network_path, local_id),
				original_view.locked(&network_path, local_id),
				"locked mismatch for node {local_id:?} in network {network_path:?}"
			);
			assert_eq!(
				rebuilt_view.pinned(&network_path, local_id),
				original_view.pinned(&network_path, local_id),
				"pinned mismatch for node {local_id:?} in network {network_path:?}"
			);

			// Per-input metadata: walk both interfaces' input slots in lockstep.
			let original_inputs = original.document_node(&local_id, &network_path).map(|n| n.inputs.len()).unwrap_or(0);
			let rebuilt_inputs = rebuilt.document_node(&local_id, &network_path).map(|n| n.inputs.len()).unwrap_or(0);
			assert_eq!(
				rebuilt_inputs, original_inputs,
				"input count mismatch for node {local_id:?} in network {network_path:?}: rebuilt={rebuilt_inputs} original={original_inputs}"
			);

			for index in 0..rebuilt_inputs {
				assert_eq!(
					rebuilt_view.input_name(&network_path, local_id, index),
					original_view.input_name(&network_path, local_id, index),
					"input_name mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
				assert_eq!(
					rebuilt_view.input_description(&network_path, local_id, index),
					original_view.input_description(&network_path, local_id, index),
					"input_description mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
				assert_eq!(
					rebuilt_view.widget_override(&network_path, local_id, index),
					original_view.widget_override(&network_path, local_id, index),
					"widget_override mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
				assert_eq!(
					rebuilt_view.input_data(&network_path, local_id, index),
					original_view.input_data(&network_path, local_id, index),
					"input_data mismatch at slot {index} for node {local_id:?} in network {network_path:?}"
				);
			}

			assert_eq!(
				rebuilt_view.output_names(&network_path, local_id),
				original_view.output_names(&network_path, local_id),
				"output_names mismatch for node {local_id:?} in network {network_path:?}"
			);
		}

		// Symmetric: every node in the original must also exist in the rebuilt interface.
		for (network_path, local_id) in collect_all_node_paths(original) {
			assert!(
				rebuilt.nested_network(&network_path).and_then(|n| n.nodes.get(&local_id)).is_some(),
				"original node {local_id:?} in network {network_path:?} missing after rebuild"
			);
		}

		// Per-network metadata: every nested network path (including the root) must resolve to the same
		// `reference` in the rebuilt interface. Node-graph nav + previewing are per-peer view state that
		// lives in `session.json`, not the registry, so they're not round-tripped here.
		let mut network_paths_to_check: Vec<Vec<NodeId>> = vec![Vec::new()];
		for (network_path, local_id) in collect_all_node_paths(original) {
			let mut child = network_path.clone();
			child.push(local_id);
			if original.nested_network(&child).is_some() {
				network_paths_to_check.push(child);
			}
		}

		let mut checked_any_reference = false;
		for path in &network_paths_to_check {
			assert_eq!(rebuilt_view.reference(path), original_view.reference(path), "reference mismatch for network {path:?}");
			if original_view.reference(path).is_some() {
				checked_any_reference = true;
			}
		}

		assert!(checked_any_reference, "demo artwork produced no reference metadata — fixture is wrong or extraction is broken");
	}

	/// Per-peer view settings (`ui::doc::*`) survive the `session.json` round-trip: serialize them into
	/// the view map, then apply it onto a fresh handler and confirm each field matches.
	#[test]
	fn document_settings_round_trip() {
		let mut document = load_demo("changing-seasons.graphite");

		// Set distinctive, non-default values so the round-trip proves real data moved, not defaults.
		document.render_mode = RenderMode::Outline;
		document.rulers_visible = false;
		document.collapsed = CollapsedLayers(vec![vec![NodeId(7)], vec![NodeId(7), NodeId(42)]]);

		let view_settings = DocumentSettings {
			document_ptz: &document.document_ptz,
			render_mode: &document.render_mode,
			overlays_visibility: &document.overlays_visibility_settings,
			rulers_visible: document.rulers_visible,
			snapping_state: &document.snapping_state,
			collapsed: &document.collapsed,
		}
		.to_view_map();

		// Apply the serialized view settings onto a fresh handler and compare each field's serialized
		// form (avoids requiring `PartialEq` on every setting type).
		let mut restored = DocumentMessageHandler::default();
		restored.apply_stored_document_settings(&view_settings);

		assert_eq!(serde_json::to_value(restored.render_mode).unwrap(), serde_json::to_value(document.render_mode).unwrap(), "render_mode");
		assert_eq!(
			serde_json::to_value(restored.rulers_visible).unwrap(),
			serde_json::to_value(document.rulers_visible).unwrap(),
			"rulers_visible"
		);
		assert_eq!(
			serde_json::to_value(restored.document_ptz).unwrap(),
			serde_json::to_value(document.document_ptz).unwrap(),
			"document_ptz"
		);
		assert_eq!(
			serde_json::to_value(restored.overlays_visibility_settings).unwrap(),
			serde_json::to_value(document.overlays_visibility_settings).unwrap(),
			"overlays"
		);
		assert_eq!(
			serde_json::to_value(restored.snapping_state).unwrap(),
			serde_json::to_value(document.snapping_state).unwrap(),
			"snapping_state"
		);
		assert_eq!(serde_json::to_value(restored.collapsed).unwrap(), serde_json::to_value(document.collapsed).unwrap(), "collapsed");
	}
}
