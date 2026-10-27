use crate::{Preprocessor, Substitution};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;
use graph_craft::graphene_compiler::Compiler;
use graph_craft::util::{DEMO_ART, load_from_name, load_network};
use graph_craft::{ProtoNodeIdentifier, Type};
use graphene_std::NodeCompileMetadata;
use graphene_std::application_io::EditorApi;
use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
use interpreted_executor::util::wrap_network_in_scope;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const OUTPUT_DIRECTORY: &str = "../compiler-spike/workloads";

struct DummySpawner;
impl Spawn for DummySpawner {
	fn spawn(&self, _task: Task) {}
}

fn export_type(ty: &Type) -> Value {
	match ty {
		Type::Generic(name) => json!({ "Generic": name }),
		Type::Concrete(descriptor) => json!({ "Concrete": descriptor.name }),
		Type::Fn(input, output) => json!({ "Fn": [export_type(input), export_type(output)] }),
		Type::Item(element) => json!({ "Item": export_type(element) }),
		Type::List(element) => json!({ "List": export_type(element) }),
	}
}

fn export_input(input: &NodeInput) -> Value {
	match input {
		NodeInput::Node { node_id, output_index } => json!({ "Node": { "node": node_id.0, "output": output_index } }),
		NodeInput::Value { tagged_value, exposed } => {
			let producer = match &**tagged_value {
				TaggedValue::Resource(_) => Some("resource"),
				TaggedValue::AnimationCurve(_) => Some("curve"),
				_ => None,
			};
			json!({ "Value": {
				"ty": export_type(&tagged_value.ty()),
				"hash": tagged_value.hash_code(),
				"size": serde_json::to_string(&**tagged_value).map_or(0, |serialized| serialized.len()),
				"exposed": exposed,
				"producer": producer,
			} })
		}
		NodeInput::Import { import_type, import_index } => json!({ "Import": { "ty": export_type(import_type), "index": import_index } }),
		NodeInput::Scope(key) => json!({ "Scope": key }),
		NodeInput::Reflection(metadata) => json!({ "Reflection": format!("{metadata:?}") }),
		NodeInput::Inline(_) => json!("Inline"),
	}
}

fn export_compile_metadata(metadata: &NodeCompileMetadata) -> Value {
	json!({
		"extract": metadata.extract.bits(),
		"inject": metadata.inject.bits(),
		"footprint_invariant": metadata.footprint_invariant,
		"forwards_input": metadata.forwards_input,
		"constant_inputs": metadata.constant_inputs,
	})
}

fn export_node(node: &DocumentNode) -> Value {
	let implementation = match &node.implementation {
		DocumentNodeImplementation::Network(network) => json!({ "Network": export_network(network) }),
		DocumentNodeImplementation::ProtoNode(identifier) => json!({ "Proto": identifier.as_str() }),
		DocumentNodeImplementation::Extract => json!("Extract"),
	};
	json!({
		"inputs": node.inputs.iter().map(export_input).collect::<Vec<_>>(),
		"call_argument": export_type(&node.call_argument),
		"implementation": implementation,
		"visible": node.visible,
		"skip_deduplication": node.skip_deduplication,
		"compile_metadata": node.compile_metadata.as_ref().map(export_compile_metadata),
	})
}

fn export_network(network: &NodeNetwork) -> Value {
	let mut nodes: Vec<_> = network.nodes.iter().collect();
	nodes.sort_by_key(|(id, _)| id.0);
	let mut scope_injections: Vec<_> = network.scope_injections.iter().collect();
	scope_injections.sort_by_key(|(key, _)| key.as_str());

	json!({
		"exports": network.exports.iter().map(export_input).collect::<Vec<_>>(),
		"nodes": nodes.into_iter().map(|(id, node)| json!([id.0, export_node(node)])).collect::<Vec<_>>(),
		"scope_injections": scope_injections.into_iter().map(|(key, (id, ty))| json!([key, id.0, export_type(ty)])).collect::<Vec<_>>(),
		"generated": network.generated,
	})
}

fn export_registry() -> Value {
	let registry = &*interpreted_executor::node_registry::NODE_REGISTRY;
	let mut identifiers: Vec<&ProtoNodeIdentifier> = registry.keys().collect();
	identifiers.sort_by_key(|identifier| identifier.as_str());

	let entries: Vec<_> = identifiers
		.into_iter()
		.map(|identifier| {
			let mut implementations: Vec<_> = registry[identifier]
				.keys()
				.map(|io| json!({ "call_argument": export_type(&io.call_argument), "inputs": io.inputs.iter().map(export_type).collect::<Vec<_>>(), "output": export_type(&io.return_value) }))
				.collect();
			implementations.sort_by_key(|implementation| implementation.to_string());
			json!({ "identifier": identifier.as_str(), "implementations": implementations })
		})
		.collect();
	json!(entries)
}

fn sorted<T>(map: &HashMap<ProtoNodeIdentifier, T>) -> Vec<&ProtoNodeIdentifier> {
	let mut identifiers: Vec<_> = map.keys().collect();
	identifiers.sort_by_key(|identifier| identifier.as_str());
	identifiers
}

fn export_preprocessor(preprocessor: &Preprocessor) -> Value {
	let substitutions: Vec<_> = sorted(&preprocessor.substitutions)
		.into_iter()
		.map(|identifier| {
			let Substitution { node, wrapped_node_id } = &preprocessor.substitutions[identifier];
			json!({ "identifier": identifier.as_str(), "node": export_node(node), "wrapped_node_id": wrapped_node_id.0 })
		})
		.collect();
	let inject_scopes: Vec<_> = sorted(&preprocessor.inject_scopes)
		.into_iter()
		.map(|identifier| {
			let (template, ty) = &preprocessor.inject_scopes[identifier];
			json!({ "identifier": identifier.as_str(), "template": export_node(template), "ty": export_type(ty) })
		})
		.collect();
	let compile_metadata: Vec<_> = sorted(&preprocessor.compile_metadata)
		.into_iter()
		.map(|identifier| json!({ "identifier": identifier.as_str(), "metadata": export_compile_metadata(&preprocessor.compile_metadata[identifier]) }))
		.collect();

	json!({ "substitutions": substitutions, "inject_scopes": inject_scopes, "compile_metadata": compile_metadata })
}

/// Writes the compiler spike's workloads: each demo's scoped network before preprocessing, the node registry,
/// the preprocessor's tables, and the real compiler's results for validating the spike's baseline port.
#[test]
#[ignore]
fn export_compiler_spike_workloads() {
	std::fs::create_dir_all(OUTPUT_DIRECTORY).unwrap();
	let write = |name: &str, value: &Value| std::fs::write(format!("{OUTPUT_DIRECTORY}/{name}.json"), serde_json::to_string(value).unwrap()).unwrap();

	let preprocessor = Preprocessor::new();
	write("registry", &export_registry());
	write("preprocessor", &export_preprocessor(&preprocessor));

	// Templates are documents the editor generates for the spike's synthetic workloads, such as `rectangle-layers`
	let templates = std::fs::read_dir(format!("{OUTPUT_DIRECTORY}/templates")).into_iter().flatten().filter_map(|entry| {
		let path = entry.ok()?.path();
		let name = path.file_stem()?.to_str()?.to_string();
		Some((name, load_network(&std::fs::read_to_string(&path).ok()?)))
	});
	let documents = DEMO_ART.iter().map(|name| (name.to_string(), load_from_name(name))).chain(templates);

	let runtime = AsyncRuntime::new(Arc::new(DummySpawner));
	let mut expected = serde_json::Map::new();
	for (name, document) in documents {
		let name = name.as_str();
		let scoped = wrap_network_in_scope(document, Arc::new(EditorApi::default()), runtime.handle());
		write(name, &export_network(&scoped));

		let mut preprocessed = scoped;
		preprocessor.preprocess(&mut preprocessed, &|_| None).unwrap();
		let proto_network = Compiler {}.compile_single(preprocessed).unwrap().network;
		let unique_stable_ids: HashSet<_> = proto_network.nodes.iter().map(|(id, _)| *id).collect();
		let distinct_identifiers: HashSet<_> = proto_network.nodes.iter().map(|(_, node)| node.identifier.clone()).collect();
		let executor = interpreted_executor::dynamic_executor::DynamicExecutor::new(proto_network.clone()).unwrap();

		expected.insert(
			name.to_string(),
			json!({
				"proto_nodes": proto_network.nodes.len(),
				"unique_stable_ids": unique_stable_ids.len(),
				"distinct_identifiers": distinct_identifiers.len(),
				"output_type": executor.output_type().as_ref().map(export_type),
			}),
		);
	}
	write("expected", &Value::Object(expected));
}
