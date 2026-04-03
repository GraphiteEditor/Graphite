#[macro_use]
extern crate log;

use graph_craft::document::value::*;
use graph_craft::document::*;
use graph_craft::proto::RegistryValueSource;
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::registry::*;
use graphene_std::*;
use std::collections::{HashMap, HashSet};

pub fn expand_network(network: &mut NodeNetwork, substitutions: &HashMap<ProtoNodeIdentifier, DocumentNode>) {
	if network.generated {
		return;
	}

	for node in network.nodes.values_mut() {
		match &mut node.implementation {
			DocumentNodeImplementation::Network(node_network) => expand_network(node_network, substitutions),
			DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
				if let Some(new_node) = substitutions.get(proto_node_identifier) {
					// Reconcile the document node's inputs with what the current node definition expects,
					// since the saved document may have fewer or more inputs than the current version
					while node.inputs.len() < new_node.inputs.len() {
						node.inputs.push(new_node.inputs[node.inputs.len()].clone());
					}
					node.inputs.truncate(new_node.inputs.len());

					node.implementation = new_node.implementation.clone();
				}
			}
			DocumentNodeImplementation::Extract => (),
		}
	}
}

pub fn generate_node_substitutions() -> HashMap<ProtoNodeIdentifier, DocumentNode> {
	let mut custom = HashMap::new();
	// We pre initialize the node registry here to avoid a deadlock
	let into_node_registry = &*interpreted_executor::node_registry::NODE_REGISTRY;
	let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
	for (id, metadata) in core_types::registry::NODE_METADATA.lock().unwrap().iter() {
		let id = id.clone();

		let NodeMetadata { fields, output_fields, .. } = metadata;
		let Some(implementations) = node_registry.get(&id) else { continue };
		let valid_call_args: HashSet<_> = implementations.iter().map(|(_, node_io)| node_io.call_argument.clone()).collect();
		let first_node_io = implementations.first().map(|(_, node_io)| node_io).unwrap_or(const { &NodeIOTypes::empty() });
		let mut node_io_types = vec![HashSet::new(); fields.len()];
		for (_, node_io) in implementations.iter() {
			for (i, ty) in node_io.inputs.iter().enumerate() {
				node_io_types[i].insert(ty.clone());
			}
		}
		let mut input_type = &first_node_io.call_argument;
		if valid_call_args.len() > 1 {
			input_type = &const { generic!(D) };
		}

		let inputs: Vec<_> = node_inputs(fields, first_node_io);
		let input_count = inputs.len();
		let network_inputs = (0..input_count).map(|i| NodeInput::node(NodeId(i as u64), 0)).collect();

		let identity_node = ops::identity::IDENTIFIER;
		let memo_node = memo::memo::IDENTIFIER;

		let mut generated_nodes = 0;
		let mut nodes: HashMap<_, _, _> = node_io_types
			.iter()
			.take(input_count)
			.enumerate()
			.map(|(i, inputs)| {
				(
					NodeId(i as u64),
					match inputs.len() {
						1 => {
							let input = inputs.iter().next().unwrap();
							let input_ty = input.nested_type();
							let mut inputs = vec![NodeInput::import(input.clone(), i)];

							let into_node_identifier = ProtoNodeIdentifier::with_owned_string(format!("graphene_core::ops::IntoNode<{}>", input_ty.identifier_name()));
							let convert_node_identifier = ProtoNodeIdentifier::with_owned_string(format!("graphene_core::ops::ConvertNode<{}>", input_ty.identifier_name()));

							let proto_node = if into_node_registry.keys().any(|ident: &ProtoNodeIdentifier| ident.as_str() == into_node_identifier.as_str()) {
								generated_nodes += 1;
								into_node_identifier
							} else if into_node_registry.keys().any(|ident| ident.as_str() == convert_node_identifier.as_str()) {
								generated_nodes += 1;
								inputs.push(NodeInput::value(TaggedValue::None, false));
								convert_node_identifier
							} else {
								identity_node.clone()
							};
							let mut original_location = OriginalLocation::default();
							original_location.auto_convert_index = Some(i);
							DocumentNode {
								inputs,
								implementation: DocumentNodeImplementation::ProtoNode(proto_node),
								visible: true,
								original_location,
								..Default::default()
							}
						}
						_ => DocumentNode {
							inputs: vec![NodeInput::import(generic!(X), i)],
							implementation: DocumentNodeImplementation::ProtoNode(identity_node.clone()),
							visible: false,
							..Default::default()
						},
					},
				)
			})
			.collect();

		// A leading `()` field (empty node_path) indicates a hidden primary output placeholder
		let has_hidden_primary = output_fields.first().is_some_and(|field| field.node_path.is_empty());
		let real_output_fields = if has_hidden_primary { &output_fields[1..] } else { output_fields };

		let available_output_fields: Vec<_> = real_output_fields
			.iter()
			.filter_map(|field| {
				let identifier = ProtoNodeIdentifier::with_owned_string(field.node_path.to_string());
				if node_registry.contains_key(&identifier) {
					Some((field, identifier))
				} else {
					warn!("Failed to find output field extractor node '{}' for '{}'", field.node_path, id.as_str());
					None
				}
			})
			.collect();

		if generated_nodes == 0 && available_output_fields.is_empty() {
			continue;
		}

		let source_node_id = NodeId(input_count as u64);
		let mut generated_node_count = 1;
		let mut output_source_node = source_node_id;

		let document_node = DocumentNode {
			inputs: network_inputs,
			call_argument: input_type.clone(),
			implementation: DocumentNodeImplementation::ProtoNode(id.clone()),
			visible: true,
			skip_deduplication: false,
			context_features: ContextDependencies::from(metadata.context_features.as_slice()),
			..Default::default()
		};

		nodes.insert(source_node_id, document_node);

		let use_memo = !available_output_fields.is_empty()
			&& node_registry.get(&memo_node).is_some_and(|memo_implementations| {
				memo_implementations
					.iter()
					.any(|(_, node_io)| node_io.call_argument == *input_type && node_io.return_value == first_node_io.return_value)
			});

		if use_memo {
			let memo_node_id = NodeId((input_count + generated_node_count) as u64);
			generated_node_count += 1;
			nodes.insert(
				memo_node_id,
				DocumentNode {
					inputs: vec![NodeInput::node(source_node_id, 0)],
					call_argument: input_type.clone(),
					implementation: DocumentNodeImplementation::ProtoNode(memo_node.clone()),
					visible: false,
					..Default::default()
				},
			);
			output_source_node = memo_node_id;
		}

		let exports = if available_output_fields.is_empty() {
			vec![NodeInput::Node {
				node_id: source_node_id,
				output_index: 0,
			}]
		} else {
			let mut exports = if has_hidden_primary { vec![NodeInput::value(TaggedValue::None, false)] } else { Vec::new() };

			exports.extend(available_output_fields.iter().map(|(_, field_identifier)| {
				let accessor_call_argument = node_registry
					.get(field_identifier)
					.and_then(|implementations| implementations.first().map(|(_, node_io)| node_io.call_argument.clone()))
					.unwrap_or_else(|| input_type.clone());

				let accessor_node_id = NodeId((input_count + generated_node_count) as u64);
				generated_node_count += 1;

				nodes.insert(
					accessor_node_id,
					DocumentNode {
						inputs: vec![NodeInput::node(output_source_node, 0)],
						call_argument: accessor_call_argument,
						implementation: DocumentNodeImplementation::ProtoNode(field_identifier.clone()),
						visible: false,
						..Default::default()
					},
				);

				NodeInput::Node {
					node_id: accessor_node_id,
					output_index: 0,
				}
			}));

			exports
		};

		let node = DocumentNode {
			inputs,
			call_argument: input_type.clone(),
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports,
				nodes,
				scope_injections: Default::default(),
				generated: true,
			}),
			visible: true,
			skip_deduplication: false,
			..Default::default()
		};

		custom.insert(id.clone(), node);
	}

	custom
}

pub fn node_inputs(fields: &[registry::FieldMetadata], first_node_io: &NodeIOTypes) -> Vec<NodeInput> {
	fields
		.iter()
		.zip(first_node_io.inputs.iter())
		.enumerate()
		.map(|(index, (field, node_io_ty))| {
			let ty = field.default_type.as_ref().unwrap_or(node_io_ty);
			let exposed = if index == 0 { *ty != fn_type_fut!(Context, ()) } else { field.exposed };

			match field.value_source {
				RegistryValueSource::None => {}
				RegistryValueSource::Default(data) => {
					if let Some(custom_default) = TaggedValue::from_primitive_string(data, ty) {
						return NodeInput::value(custom_default, exposed);
					} else {
						// It is incredibly useful to get a warning when the default type cannot be parsed rather than defaulting to `()`.
						warn!("Failed to parse default value for type `{ty:?}` with data `{data}`");
					}
				}
				RegistryValueSource::Scope(data) => return NodeInput::scope(Cow::Borrowed(data)),
			};

			if let Some(type_default) = TaggedValue::from_type(ty) {
				return NodeInput::value(type_default, exposed);
			}
			NodeInput::value(TaggedValue::None, true)
		})
		.collect()
}
