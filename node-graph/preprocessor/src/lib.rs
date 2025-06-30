use graph_craft::document::value::*;
use graph_craft::document::*;
use graph_craft::proto::RegistryValueSource;
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::registry::*;
use graphene_std::*;
use std::collections::{HashMap, HashSet};

pub fn expand_network(network: &mut NodeNetwork, substitutions: &HashMap<String, DocumentNode>) {
	if network.generated {
		return;
	}

	for node in network.nodes.values_mut() {
		match &mut node.implementation {
			DocumentNodeImplementation::Network(node_network) => expand_network(node_network, substitutions),
			DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
				if let Some(new_node) = substitutions.get(proto_node_identifier.name.as_ref()) {
					node.implementation = new_node.implementation.clone();
				}
			}
			DocumentNodeImplementation::Extract => (),
		}
	}
}

pub fn generate_node_substitutions() -> HashMap<String, DocumentNode> {
	let mut custom = HashMap::new();
	let node_registry = graphene_core::registry::NODE_REGISTRY.lock().unwrap();
	for (id, metadata) in graphene_core::registry::NODE_METADATA.lock().unwrap().iter() {
		let id = id.clone();

		let NodeMetadata { fields, .. } = metadata;
		let Some(implementations) = &node_registry.get(&id) else { continue };
		let valid_inputs: HashSet<_> = implementations.iter().map(|(_, node_io)| node_io.call_argument.clone()).collect();
		let first_node_io = implementations.first().map(|(_, node_io)| node_io).unwrap_or(const { &NodeIOTypes::empty() });
		let mut node_io_types = vec![HashSet::new(); fields.len()];
		for (_, node_io) in implementations.iter() {
			for (i, ty) in node_io.inputs.iter().enumerate() {
				node_io_types[i].insert(ty.clone());
			}
		}
		let mut input_type = &first_node_io.call_argument;
		if valid_inputs.len() > 1 {
			input_type = &const { generic!(D) };
		}

		let inputs: Vec<_> = node_inputs(fields, first_node_io);
		let input_count = inputs.len();
		let network_inputs = (0..input_count).map(|i| NodeInput::node(NodeId(i as u64), 0)).collect();

		let identity_node = ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode");

		let into_node_registry = &interpreted_executor::node_registry::NODE_REGISTRY;

		let mut generated_nodes = 0;
		let mut nodes: HashMap<_, _, _> = node_io_types
			.iter()
			.enumerate()
			.map(|(i, inputs)| {
				(
					NodeId(i as u64),
					match inputs.len() {
						1 if false => {
							let input = inputs.iter().next().unwrap();
							let input_ty = input.nested_type();

							let into_node_identifier = ProtoNodeIdentifier {
								name: format!("graphene_core::ops::IntoNode<{}>", input_ty.clone()).into(),
							};
							let convert_node_identifier = ProtoNodeIdentifier {
								name: format!("graphene_core::ops::ConvertNode<{}>", input_ty.clone()).into(),
							};

							let proto_node = if into_node_registry.keys().any(|ident: &ProtoNodeIdentifier| ident.name.as_ref() == into_node_identifier.name.as_ref()) {
								generated_nodes += 1;
								into_node_identifier
							} else if into_node_registry.keys().any(|ident| ident.name.as_ref() == convert_node_identifier.name.as_ref()) {
								generated_nodes += 1;
								convert_node_identifier
							} else {
								identity_node.clone()
							};

							DocumentNode {
								inputs: vec![NodeInput::network(input.clone(), i)],
								// manual_composition: Some(fn_input.clone()),
								implementation: DocumentNodeImplementation::ProtoNode(proto_node),
								visible: true,
								..Default::default()
							}
						}
						_ => DocumentNode {
							inputs: vec![NodeInput::network(generic!(X), i)],
							implementation: DocumentNodeImplementation::ProtoNode(identity_node.clone()),
							visible: false,
							..Default::default()
						},
					},
				)
			})
			.collect();

		if generated_nodes == 0 {
			continue;
		}

		let document_node = DocumentNode {
			inputs: network_inputs,
			manual_composition: Some(input_type.clone()),
			implementation: DocumentNodeImplementation::ProtoNode(id.clone().into()),
			visible: true,
			skip_deduplication: false,
			..Default::default()
		};

		nodes.insert(NodeId(input_count as u64), document_node);

		let node = DocumentNode {
			inputs,
			manual_composition: Some(input_type.clone()),
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::Node {
					node_id: NodeId(input_count as u64),
					output_index: 0,
					lambda: false,
				}],
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
				RegistryValueSource::Default(data) => return NodeInput::value(TaggedValue::from_primitive_string(data, ty).unwrap_or(TaggedValue::None), exposed),
				RegistryValueSource::Scope(data) => return NodeInput::scope(Cow::Borrowed(data)),
			};

			if let Some(type_default) = TaggedValue::from_type(ty) {
				return NodeInput::value(type_default, exposed);
			}
			NodeInput::value(TaggedValue::None, true)
		})
		.collect()
}
