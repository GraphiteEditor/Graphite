use super::DocumentNodeDefinition;
use crate::messages::portfolio::document::utility_types::network_interface::{
	DocumentNodeMetadata, DocumentNodePersistentMetadata, NodeNetworkMetadata, NodeNetworkPersistentMetadata, NodePersistentMetadata, NodePosition, NodeTemplate, NodeTypePersistentMetadata,
	PropertiesRow, WidgetOverride,
};
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::value::*;
use graph_craft::document::*;
use graphene_std::registry::*;
use graphene_std::*;
use std::collections::{HashMap, HashSet};

pub(super) fn post_process_nodes(mut custom: Vec<DocumentNodeDefinition>) -> Vec<DocumentNodeDefinition> {
	// Remove struct generics
	for DocumentNodeDefinition { node_template, .. } in custom.iter_mut() {
		let NodeTemplate {
			document_node: DocumentNode { implementation, .. },
			..
		} = node_template;
		if let DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier { name }) = implementation {
			if let Some((new_name, _suffix)) = name.rsplit_once("<") {
				*name = Cow::Owned(new_name.to_string())
			}
		};
	}
	let node_registry = graphene_core::registry::NODE_REGISTRY.lock().unwrap();
	'outer: for (id, metadata) in NODE_METADATA.lock().unwrap().iter() {
		let id = id.clone();

		for node in custom.iter() {
			let DocumentNodeDefinition {
				node_template: NodeTemplate {
					document_node: DocumentNode { implementation, .. },
					..
				},
				..
			} = node;
			match implementation {
				DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier { name }) if name == &id => continue 'outer,
				_ => (),
			}
		}

		let NodeMetadata {
			display_name,
			category,
			fields,
			description,
			properties,
		} = metadata;
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
		let output_type = &first_node_io.return_value;

		let inputs: Vec<_> = fields
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
			.collect();
		let input_count = inputs.len();
		let network_inputs = (0..input_count).map(|i| NodeInput::node(NodeId(i as u64), 0)).collect();
		let identity_node = ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode");
		let into_node_registry = &interpreted_executor::node_registry::NODE_REGISTRY;
		let mut nodes: HashMap<_, _, _> = node_io_types
			.iter()
			.enumerate()
			.map(|(i, inputs)| {
				(
					NodeId(i as u64),
					match inputs.len() {
						1 => {
							let input = inputs.iter().next().unwrap();
							let input_ty = input.nested_type();
							let into_node_identifier = ProtoNodeIdentifier {
								name: format!("graphene_core::ops::IntoNode<{}>", input_ty.clone()).into(),
							};
							let convert_node_identifier = ProtoNodeIdentifier {
								name: format!("graphene_core::ops::ConvertNode<{}>", input_ty.clone()).into(),
							};
							let proto_node = if into_node_registry.iter().any(|(ident, _)| {
								let ident = ident.name.as_ref();
								ident == into_node_identifier.name.as_ref()
							}) {
								into_node_identifier
							} else if into_node_registry.iter().any(|(ident, _)| {
								let ident = ident.name.as_ref();
								ident == convert_node_identifier.name.as_ref()
							}) {
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

		let document_node = DocumentNode {
			inputs: network_inputs,
			manual_composition: Some(input_type.clone()),
			implementation: DocumentNodeImplementation::ProtoNode(id.clone().into()),
			visible: true,
			skip_deduplication: false,
			..Default::default()
		};
		let mut node_names: HashMap<NodeId, String> = nodes
			.iter()
			.map(|(id, node)| (*id, node.implementation.get_proto_node().unwrap().name.rsplit_once("::").unwrap().1.to_string()))
			.collect();
		nodes.insert(NodeId(input_count as u64), document_node);
		node_names.insert(NodeId(input_count as u64), display_name.to_string());
		let node_type_metadata = |id: NodeId| {
			NodeTypePersistentMetadata::Node(NodePersistentMetadata::new(NodePosition::Absolute(if id.0 == input_count as u64 {
				IVec2::default()
			} else {
				IVec2 { x: -10, y: id.0 as i32 }
			})))
		};

		let node = DocumentNodeDefinition {
			identifier: display_name,
			node_template: NodeTemplate {
				document_node: DocumentNode {
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
					}),
					visible: true,
					skip_deduplication: false,
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					// TODO: Store information for input overrides in the node macro
					input_properties: fields
						.iter()
						.map(|f| match f.widget_override {
							RegistryWidgetOverride::None => (f.name, f.description).into(),
							RegistryWidgetOverride::Hidden => PropertiesRow::with_override(f.name, f.description, WidgetOverride::Hidden),
							RegistryWidgetOverride::String(str) => PropertiesRow::with_override(f.name, f.description, WidgetOverride::String(str.to_string())),
							RegistryWidgetOverride::Custom(str) => PropertiesRow::with_override(f.name, f.description, WidgetOverride::Custom(str.to_string())),
						})
						.collect(),
					output_names: vec![output_type.to_string()],
					has_primary_output: true,
					locked: false,

					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: node_names
								.into_iter()
								.map(|(id, display_name)| {
									let node_type_metadata = node_type_metadata(id);
									(
										id,
										DocumentNodeMetadata {
											persistent_metadata: DocumentNodePersistentMetadata {
												display_name,
												node_type_metadata,
												..Default::default()
											},
											..Default::default()
										},
									)
								})
								.collect(),
							..Default::default()
						},
						..Default::default()
					}),

					..Default::default()
				},
			},
			category: category.unwrap_or("UNCATEGORIZED"),
			description: Cow::Borrowed(description),
			properties: *properties,
		};
		custom.push(node);
	}
	custom
}
