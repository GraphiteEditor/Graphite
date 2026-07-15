//! Human-readable pretty-printers for the dual-write soak's validation checks: registry-vs-registry and
//! runtime-network-vs-runtime-network diffs. The callers are gated behind the `validate_storage_round_trip`
//! preference, so these ship in release but only run when the soak is turned on.

use std::fmt::Write;

pub(crate) fn diff_registries(stored: &graph_storage::Registry, target: &graph_storage::Registry) -> String {
	let mut out = String::new();

	let stored_node_ids: std::collections::BTreeSet<_> = stored.node_instances.keys().copied().collect();
	let target_node_ids: std::collections::BTreeSet<_> = target.node_instances.keys().copied().collect();
	let missing_nodes: Vec<_> = target_node_ids.difference(&stored_node_ids).collect();
	let extra_nodes: Vec<_> = stored_node_ids.difference(&target_node_ids).collect();
	let shared_node_diffs: Vec<_> = stored_node_ids
		.intersection(&target_node_ids)
		.filter(|id| !stored.node_instances[id].value_equal(&target.node_instances[id]))
		.collect();

	let stored_network_ids: std::collections::BTreeSet<_> = stored.networks.keys().copied().collect();
	let target_network_ids: std::collections::BTreeSet<_> = target.networks.keys().copied().collect();
	let missing_networks: Vec<_> = target_network_ids.difference(&stored_network_ids).collect();
	let extra_networks: Vec<_> = stored_network_ids.difference(&target_network_ids).collect();
	let shared_network_diffs: Vec<_> = stored_network_ids
		.intersection(&target_network_ids)
		.filter(|id| !stored.networks[id].value_equal(&target.networks[id]))
		.collect();

	let _ = writeln!(out, "  nodes:    stored={} target={}", stored_node_ids.len(), target_node_ids.len());
	if !missing_nodes.is_empty() {
		let _ = writeln!(out, "    missing from stored: {missing_nodes:?}");
	}
	if !extra_nodes.is_empty() {
		let _ = writeln!(out, "    extra in stored:     {extra_nodes:?}");
	}
	if !shared_node_diffs.is_empty() {
		let _ = writeln!(out, "    differing payloads:  {shared_node_diffs:?}");
		for id in &shared_node_diffs {
			let stored_node = &stored.node_instances[id];
			let target_node = &target.node_instances[id];
			let _ = writeln!(out, "      node {id}:");
			diff_node(&mut out, stored_node, target_node);
		}
	}

	let _ = writeln!(out, "  networks: stored={} target={}", stored_network_ids.len(), target_network_ids.len());
	if !missing_networks.is_empty() {
		let _ = writeln!(out, "    missing from stored: {missing_networks:?}");
	}
	if !extra_networks.is_empty() {
		let _ = writeln!(out, "    extra in stored:     {extra_networks:?}");
	}
	if !shared_network_diffs.is_empty() {
		let _ = writeln!(out, "    differing payloads:  {shared_network_diffs:?}");
		for id in &shared_network_diffs {
			let stored_network = &stored.networks[id];
			let target_network = &target.networks[id];
			let _ = writeln!(out, "      network {id}:");
			diff_network(&mut out, stored_network, target_network);
		}
	}

	let stored_resources: std::collections::BTreeSet<_> = stored.resources.keys().copied().collect();
	let target_resources: std::collections::BTreeSet<_> = target.resources.keys().copied().collect();
	let missing_resources: Vec<_> = target_resources.difference(&stored_resources).collect();
	let extra_resources: Vec<_> = stored_resources.difference(&target_resources).collect();
	let shared_resource_diffs: Vec<_> = stored_resources
		.intersection(&target_resources)
		.filter(|id| !resource_value_equal(&stored.resources[id], &target.resources[id]))
		.collect();
	if !missing_resources.is_empty() || !extra_resources.is_empty() || !shared_resource_diffs.is_empty() {
		let _ = writeln!(out, "  resources: stored={} target={}", stored_resources.len(), target_resources.len());
		if !missing_resources.is_empty() {
			let _ = writeln!(out, "    missing from stored: {missing_resources:?}");
		}
		if !extra_resources.is_empty() {
			let _ = writeln!(out, "    extra in stored:     {extra_resources:?}");
		}
		if !shared_resource_diffs.is_empty() {
			let _ = writeln!(out, "    differing payloads:  {shared_resource_diffs:?}");
			for id in &shared_resource_diffs {
				let _ = writeln!(out, "      resource {id}:");
				diff_resource(&mut out, &stored.resources[id], &target.resources[id]);
			}
		}
	}

	if stored.attributes != target.attributes {
		diff_attributes(&mut out, "  document attributes", &stored.attributes, &target.attributes);
	}

	out
}

fn diff_node(out: &mut String, stored: &graph_storage::Node, target: &graph_storage::Node) {
	if stored.implementation() != target.implementation() {
		let _ = writeln!(out, "        implementation: stored={:?} target={:?}", stored.implementation(), target.implementation());
	}
	if stored.network() != target.network() {
		let _ = writeln!(out, "        network back-pointer: stored={} target={}", stored.network(), target.network());
	}

	let stored_inputs = stored.inputs();
	let target_inputs = target.inputs();
	if stored_inputs.len() != target_inputs.len() {
		let _ = writeln!(out, "        inputs.len: stored={} target={}", stored_inputs.len(), target_inputs.len());
	}
	for (i, (s, t)) in stored_inputs.iter().zip(target_inputs.iter()).enumerate() {
		if s != t {
			let value_differs = s.input != t.input;
			let timestamp_differs = s.timestamp != t.timestamp;
			let _ = writeln!(out, "        input[{i}]: value_differs={value_differs} timestamp_differs={timestamp_differs}");
			if value_differs {
				let _ = writeln!(out, "          stored.value={:?}\n          target.value={:?}", s.input, t.input);
			}
			if s.attributes != t.attributes {
				diff_attributes(out, &format!("        input[{i}].attributes"), &s.attributes, &t.attributes);
			}
		}
	}

	if stored.attributes() != target.attributes() {
		diff_attributes(out, "        attributes", stored.attributes(), target.attributes());
	}
}

fn diff_network(out: &mut String, stored: &graph_storage::Network, target: &graph_storage::Network) {
	if stored.exports.len() != target.exports.len() {
		let _ = writeln!(out, "        exports.len: stored={} target={}", stored.exports.len(), target.exports.len());
	}
	for (i, (s, t)) in stored.exports.iter().zip(target.exports.iter()).enumerate() {
		if s != t {
			let target_differs = s.target != t.target;
			let timestamp_differs = s.timestamp != t.timestamp;
			let _ = writeln!(out, "        export[{i}]: target_differs={target_differs} timestamp_differs={timestamp_differs}");
			if target_differs {
				let _ = writeln!(out, "          stored.target={:?}\n          target.target={:?}", s.target, t.target);
			}
		}
	}

	if stored.attributes != target.attributes {
		diff_attributes(out, "        attributes", &stored.attributes, &target.attributes);
	}
}

/// Value-level resource comparison (same resolved hash, same source bodies keyed by `SourceKey`),
/// ignoring LWW timestamps. Mirrors `graph_storage`'s internal `resources_value_equal` for a single
/// entry, since that helper is crate-private and only operates over a whole store.
fn resource_value_equal(stored: &graph_storage::ResourceEntry, target: &graph_storage::ResourceEntry) -> bool {
	stored.hash == target.hash && stored.sources.len() == target.sources.len() && stored.sources.iter().all(|(key, value)| target.source(key).is_some_and(|other| value.source == other.source))
}

fn diff_resource(out: &mut String, stored: &graph_storage::ResourceEntry, target: &graph_storage::ResourceEntry) {
	if stored.hash != target.hash {
		let _ = writeln!(out, "        hash: stored={:?} target={:?}", stored.hash, target.hash);
	}
	if stored.sources.len() != target.sources.len() {
		let _ = writeln!(out, "        sources.len: stored={} target={}", stored.sources.len(), target.sources.len());
	}

	// Report source bodies that drift for a shared key, plus keys present on only one side.
	for (key, value) in &stored.sources {
		match target.source(key) {
			Some(other) if other.source != value.source => {
				let _ = writeln!(out, "        source[{key:?}]: stored={:?} target={:?}", value.source, other.source);
			}
			None => {
				let _ = writeln!(out, "        source[{key:?}]: only in stored");
			}
			Some(_) => {}
		}
	}
	for (key, _) in &target.sources {
		if stored.source(key).is_none() {
			let _ = writeln!(out, "        source[{key:?}]: only in target");
		}
	}
}

fn diff_attributes(out: &mut String, label: &str, stored: &graph_storage::Attributes, target: &graph_storage::Attributes) {
	let stored_keys: std::collections::BTreeSet<_> = stored.keys().collect();
	let target_keys: std::collections::BTreeSet<_> = target.keys().collect();
	let missing: Vec<_> = target_keys.difference(&stored_keys).collect();
	let extra: Vec<_> = stored_keys.difference(&target_keys).collect();

	// Split value drift from timestamp-only drift: each attribute carries an LWW timestamp, so comparing
	// whole records would flag equal-value entries that merely differ in when they were last set.
	let (differing_values, differing_timestamps): (Vec<_>, Vec<_>) = stored_keys
		.intersection(&target_keys)
		.copied()
		.filter(|k| stored.get(*k) != target.get(*k))
		.partition(|k| stored.get(*k).map(|v| &v.value) != target.get(*k).map(|v| &v.value));

	let _ = writeln!(
		out,
		"{label}: missing_from_stored={missing:?} extra_in_stored={extra:?} differing_values={differing_values:?} differing_timestamps={differing_timestamps:?}"
	);
}

/// Human-readable summary of how two networks differ (exports, node set, per-node payloads, scope injections).
pub(crate) fn diff_networks(expected: &graph_craft::document::NodeNetwork, actual: &graph_craft::document::NodeNetwork) -> String {
	let mut out = String::new();

	if expected.exports != actual.exports {
		let _ = writeln!(out, "  exports differ: expected={} actual={}", expected.exports.len(), actual.exports.len());
		for (i, (exp, act)) in expected.exports.iter().zip(actual.exports.iter()).enumerate() {
			if exp != act {
				let _ = writeln!(out, "    [{i}] expected={exp:?}\n        actual=  {act:?}");
			}
		}
	}

	let expected_ids: std::collections::BTreeSet<_> = expected.nodes.keys().copied().collect();
	let actual_ids: std::collections::BTreeSet<_> = actual.nodes.keys().copied().collect();
	let missing: Vec<_> = expected_ids.difference(&actual_ids).collect();
	let extra: Vec<_> = actual_ids.difference(&expected_ids).collect();
	let differing: Vec<_> = expected_ids.intersection(&actual_ids).filter(|id| expected.nodes.get(id) != actual.nodes.get(id)).collect();

	if !missing.is_empty() || !extra.is_empty() || !differing.is_empty() {
		let _ = writeln!(out, "  nodes: expected={} actual={}", expected_ids.len(), actual_ids.len());
		if !missing.is_empty() {
			let _ = writeln!(out, "    missing from actual: {missing:?}");
		}
		if !extra.is_empty() {
			let _ = writeln!(out, "    extra in actual:     {extra:?}");
		}
		if !differing.is_empty() {
			let _ = writeln!(out, "    differing payloads:  {differing:?}");
			for id in &differing {
				if let (Some(exp), Some(act)) = (expected.nodes.get(id), actual.nodes.get(id)) {
					let _ = writeln!(out, "    node {id}:");
					diff_document_node(&mut out, exp, act);
				}
			}
		}
	}

	if expected.scope_injections != actual.scope_injections {
		let _ = writeln!(out, "  scope_injections differ");
	}

	out
}

/// Field-level diff between two runtime `DocumentNode`s with the same ID, so the compare-on-open log
/// names *which* field diverged rather than just the node ID. `original_location` is `#[serde(skip)]`
/// and recomputed at load, so it's a likely culprit for a payload mismatch that doesn't affect behavior.
fn diff_document_node(out: &mut String, expected: &graph_craft::document::DocumentNode, actual: &graph_craft::document::DocumentNode) {
	if expected.inputs != actual.inputs {
		let _ = writeln!(out, "      inputs differ (len expected={} actual={})", expected.inputs.len(), actual.inputs.len());
		for (i, (e, a)) in expected.inputs.iter().zip(actual.inputs.iter()).enumerate() {
			if e != a {
				let _ = writeln!(out, "        input[{i}]: expected={e:?}\n                   actual=  {a:?}");
			}
		}
	}
	if expected.call_argument != actual.call_argument {
		let _ = writeln!(out, "      call_argument: expected={:?} actual={:?}", expected.call_argument, actual.call_argument);
	}
	if expected.implementation != actual.implementation {
		let _ = writeln!(out, "      implementation: expected={:?} actual={:?}", expected.implementation, actual.implementation);
	}
	if expected.visible != actual.visible {
		let _ = writeln!(out, "      visible: expected={} actual={}", expected.visible, actual.visible);
	}
	if expected.skip_deduplication != actual.skip_deduplication {
		let _ = writeln!(out, "      skip_deduplication: expected={} actual={}", expected.skip_deduplication, actual.skip_deduplication);
	}
	if expected.context_features != actual.context_features {
		let _ = writeln!(out, "      context_features: expected={:?} actual={:?}", expected.context_features, actual.context_features);
	}
	if expected.original_location != actual.original_location {
		let _ = writeln!(out, "      original_location differs (recomputed at load, not stored):");
		let _ = writeln!(out, "        expected={:?}\n        actual=  {:?}", expected.original_location, actual.original_location);
	}
}
