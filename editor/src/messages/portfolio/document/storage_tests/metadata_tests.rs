//! Conversion-level round-trip tests for the [`storage_metadata`](crate::messages::portfolio::document::utility_types::network_interface::storage_metadata)
//! bridge: drive a demo `.graphite` document through `Registry` (and back through
//! `build_interface_from_storage`) without an actual save/reopen, asserting the editor's `ui::*`
//! metadata survives the conversion. The end-to-end save/reopen pipeline is covered separately in
//! [`round_trip_tests`](super::round_trip_tests).

use std::collections::HashMap;

use graph_storage::{NodeMetadataSource, PeerId, Registry};

use super::test_support::{load_demo, node_paths};
use crate::messages::portfolio::document::document_message_handler::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::network_interface::storage_metadata::{DocumentSettings, StorageMetadataView, build_interface_from_storage};
use crate::messages::portfolio::document::utility_types::nodes::CollapsedLayers;
use graph_craft::document::NodeId;
use graphene_std::vector::style::RenderMode;

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

	for (network_path, local_id) in node_paths(interface) {
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
				!entries_by_address.contains_key(&(network_path.clone(), local_id)),
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
	for (network_path, local_id) in node_paths(&rebuilt) {
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
	for (network_path, local_id) in node_paths(original) {
		assert!(
			rebuilt.nested_network(&network_path).and_then(|n| n.nodes.get(&local_id)).is_some(),
			"original node {local_id:?} in network {network_path:?} missing after rebuild"
		);
	}

	// Per-network metadata: every nested network path (including the root) must resolve to the same
	// `reference` in the rebuilt interface. Node-graph nav + previewing are per-peer view state that
	// lives in `session.json`, not the registry, so they're not round-tripped here.
	let mut network_paths_to_check: Vec<Vec<NodeId>> = vec![Vec::new()];
	for (network_path, local_id) in node_paths(original) {
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
