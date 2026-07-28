#[macro_use]
extern crate log;

use graph_craft::Type;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::*;
use graph_craft::document::*;
use graph_craft::proto::RegistryValueSource;
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::platform_application_io::ResourceHash;
use graphene_std::registry::*;
use graphene_std::*;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Debug, Default, Clone)]
pub struct Preprocessor {
	substitutions: HashMap<ProtoNodeIdentifier, DocumentNode>,
	inject_scopes: HashMap<ProtoNodeIdentifier, (DocumentNode, Type)>,
}

impl Preprocessor {
	pub fn preprocess(&self, network: &mut NodeNetwork, resolve_resource: &dyn Fn(ResourceId) -> Option<ResourceHash>) -> Result<(), PreprocessorError> {
		self.insert_inject_scopes(network);
		self.replace_resource_inputs(network, resolve_resource)?;
		self.expand_network(network);
		Ok(())
	}
}

impl Preprocessor {
	fn insert_inject_scopes(&self, network: &mut NodeNetwork) {
		for (identifier, (template, ty)) in self.inject_scopes.iter() {
			let mut hasher = DefaultHasher::new();

			identifier.as_str().hash(&mut hasher);
			let producer_id = NodeId(hasher.finish());
			network.nodes.insert(producer_id, template.clone());

			network.scope_injections.insert(identifier.as_str().to_string(), (producer_id, ty.clone()));
		}
	}

	/// Replace every `TaggedValue::Resource(hash)` input with a reference to a freshly inserted `resource` proto node.
	fn replace_resource_inputs(&self, network: &mut NodeNetwork, resolve_resource: &dyn Fn(ResourceId) -> Option<ResourceHash>) -> Result<(), PreprocessorError> {
		let mut hash_to_node_id: HashMap<graph_craft::application_io::resource::ResourceHash, NodeId> = HashMap::new();
		let mut new_resource_nodes: Vec<(NodeId, DocumentNode)> = Vec::new();

		for node in network.nodes.values_mut() {
			if let DocumentNodeImplementation::Network(nested) = &mut node.implementation {
				self.replace_resource_inputs(nested, resolve_resource)?;
				continue;
			}

			if matches!(&node.implementation, DocumentNodeImplementation::ProtoNode(identifier) if *identifier == platform_application_io::resource::IDENTIFIER) {
				continue;
			}

			for input in node.inputs.iter_mut() {
				let NodeInput::Value { tagged_value, .. } = input else { continue };
				let TaggedValue::Resource(resource_id) = **tagged_value else { continue };

				let Some(hash) = resolve_resource(resource_id) else {
					return Err(PreprocessorError::ResourceNotFound(resource_id));
				};

				let resource_id = *hash_to_node_id.entry(hash).or_insert_with(|| {
					let id = NodeId::new();
					let resource_node = DocumentNode {
						inputs: vec![
							NodeInput::scope(platform_application_io::editor_api::IDENTIFIER),
							NodeInput::value(TaggedValue::ResourceHash(hash), false),
						],
						implementation: DocumentNodeImplementation::ProtoNode(platform_application_io::resource::IDENTIFIER),
						..Default::default()
					};
					new_resource_nodes.push((id, resource_node));
					id
				});

				*input = NodeInput::node(resource_id, 0);
			}
		}

		for (id, node) in new_resource_nodes {
			network.nodes.insert(id, node);
		}

		Ok(())
	}

	fn expand_network(&self, network: &mut NodeNetwork) {
		for node in network.nodes.values_mut() {
			match &mut node.implementation {
				DocumentNodeImplementation::Network(node_network) => self.expand_network(node_network),
				DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
					if let Some(new_node) = self.substitutions.get(proto_node_identifier) {
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

	pub fn new() -> Self {
		let mut substitutions = HashMap::new();
		let mut inject_scopes = HashMap::new();
		// We pre initialize the node registry here to avoid a deadlock
		let into_node_registry = &*interpreted_executor::node_registry::NODE_REGISTRY;
		let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
		for (id, metadata) in core_types::registry::NODE_METADATA.lock().unwrap().iter() {
			let id = id.clone();

			let NodeMetadata { fields, memoize, inject_scope, .. } = metadata;
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

			let passthrough_node = ops::passthrough::IDENTIFIER;

			let mut generated_nodes = 0;
			let mut nodes: HashMap<_, _, _> = node_io_types
				.iter()
				.take(input_count)
				.enumerate()
				.map(|(i, inputs)| {
					// A field registering the Item/List wire pair gets a input adapter instead of a typed conversion
					if inputs.len() != 1
						&& let Some(list_input) = collapse_item_list_pair(inputs)
					{
						let element_name = match list_input.nested_type() {
							Type::List(element) => element.identifier_name(),
							nested => nested.identifier_name(),
						};
						let input_adapter_identifier = ProtoNodeIdentifier::with_owned_string(format!("input_adapter<{element_name}>"));

						let document_node = if into_node_registry.keys().any(|ident| ident.as_str() == input_adapter_identifier.as_str()) {
							generated_nodes += 1;
							let mut original_location = OriginalLocation::default();
							original_location.auto_convert_index = Some(i);
							DocumentNode {
								inputs: vec![NodeInput::import(generic!(X), i)],
								implementation: DocumentNodeImplementation::ProtoNode(input_adapter_identifier),
								visible: true,
								original_location,
								..Default::default()
							}
						} else {
							DocumentNode {
								inputs: vec![NodeInput::import(generic!(X), i)],
								implementation: DocumentNodeImplementation::ProtoNode(passthrough_node.clone()),
								visible: false,
								..Default::default()
							}
						};
						return (NodeId(i as u64), document_node);
					}

					let single_wire_type = match inputs.len() {
						1 => inputs.iter().next(),
						_ => None,
					};
					(
						NodeId(i as u64),
						match single_wire_type {
							Some(input) => {
								let input_ty = input.nested_type();

								// A single-registered ranked field gets the input adapter, so ranked wires pass through and convertible elements cast
								let element_name = match input_ty {
									Type::Item(element) => Some(element.identifier_name()),
									Type::List(element) => Some(element.identifier_name()),
									_ => (input_ty.identifier_name() == "ListDyn").then(|| "ListDyn".to_string()),
								};
								if let Some(element_name) = element_name {
									let input_adapter_identifier = ProtoNodeIdentifier::with_owned_string(format!("input_adapter<{element_name}>"));
									if into_node_registry.keys().any(|ident| ident.as_str() == input_adapter_identifier.as_str()) {
										generated_nodes += 1;
										let mut original_location = OriginalLocation::default();
										original_location.auto_convert_index = Some(i);
										let document_node = DocumentNode {
											inputs: vec![NodeInput::import(generic!(X), i)],
											implementation: DocumentNodeImplementation::ProtoNode(input_adapter_identifier),
											visible: true,
											original_location,
											..Default::default()
										};
										return (NodeId(i as u64), document_node);
									}
								}

								let mut original_location = OriginalLocation::default();
								original_location.auto_convert_index = Some(i);
								DocumentNode {
									inputs: vec![NodeInput::import(input.clone(), i)],
									implementation: DocumentNodeImplementation::ProtoNode(passthrough_node.clone()),
									visible: true,
									original_location,
									..Default::default()
								}
							}
							None => DocumentNode {
								inputs: vec![NodeInput::import(generic!(X), i)],
								implementation: DocumentNodeImplementation::ProtoNode(passthrough_node.clone()),
								visible: false,
								..Default::default()
							},
						},
					)
				})
				.collect();

			if generated_nodes == 0 && !memoize && !inject_scope {
				continue;
			}

			let document_node = DocumentNode {
				inputs: network_inputs,
				call_argument: input_type.clone(),
				implementation: DocumentNodeImplementation::ProtoNode(id.clone()),
				visible: true,
				skip_deduplication: false,
				context_features: ContextDependencies::from(metadata.context_features.as_slice()),
				..Default::default()
			};

			nodes.insert(NodeId(input_count as u64), document_node);

			// If memoize is requested, append a Memoize node after the main node and redirect the export through it
			let export_node_id = if *memoize {
				let memoize_node_id = NodeId(input_count as u64 + 1);
				let memoize_node = DocumentNode {
					inputs: vec![NodeInput::node(NodeId(input_count as u64), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::memo::memoize::IDENTIFIER.clone()),
					visible: true,
					..Default::default()
				};
				nodes.insert(memoize_node_id, memoize_node);
				memoize_node_id
			} else {
				NodeId(input_count as u64)
			};

			let node = DocumentNode {
				inputs,
				call_argument: input_type.clone(),
				implementation: DocumentNodeImplementation::Network(NodeNetwork {
					exports: vec![NodeInput::Node {
						node_id: export_node_id,
						output_index: 0,
					}],
					nodes,
					scope_injections: Default::default(),
					generated: true,
				}),
				visible: true,
				skip_deduplication: false,
				..Default::default()
			};

			substitutions.insert(id.clone(), node);

			// If `inject_scope` is requested, prepare the proto node template and type info needed
			if *inject_scope
				&& let Some(implementations) = node_registry.get(&id)
				&& let Some((_, node_io)) = implementations.first()
			{
				let template = DocumentNode {
					inputs: node_inputs(fields, node_io),
					call_argument: node_io.call_argument.clone(),
					implementation: DocumentNodeImplementation::ProtoNode(id.clone()),
					visible: true,
					context_features: ContextDependencies::from(metadata.context_features.as_slice()),
					..Default::default()
				};
				inject_scopes.insert(id.clone(), (template, node_io.return_value.clone()));
			}
		}

		Self { substitutions, inject_scopes }
	}
}

pub fn node_inputs(fields: &[registry::FieldMetadata], first_node_io: &NodeIOTypes) -> Vec<NodeInput> {
	fields
		.iter()
		.enumerate()
		.map(|(index, field)| {
			// `skip_impl` nodes have no concrete implementations, so `first_node_io.inputs` is shorter than `fields`.
			// When no type info is available for a field, fall through to the unspecified `None` value.
			let Some(ty) = field.default_type.as_ref().or_else(|| first_node_io.inputs.get(index)) else {
				return NodeInput::value(TaggedValue::None, true);
			};
			let ty = ty.clone().normalize_rank();
			let exposed = if index == 0 { ty != fn_type_fut!(Context, ()) } else { field.exposed };

			match &field.value_source {
				RegistryValueSource::None => {}
				RegistryValueSource::Default(data) => {
					if let Some(custom_default) = TaggedValue::from_primitive_string(data, &ty) {
						return NodeInput::value(custom_default, exposed);
					} else {
						// It is incredibly useful to get a warning when the default type cannot be parsed rather than defaulting to `()`.
						warn!("Failed to parse default value for type `{ty:?}` with data `{data}`");
					}
				}
				RegistryValueSource::Scope(data) => return NodeInput::scope(*data),
			};

			// A ranked `Item<T>` type prefers a bare `T` value (promoted at resolution), since bare values drive the Properties panel widgets
			if let Type::Item(element) = &ty
				&& let Some(type_default) = TaggedValue::from_type(element)
			{
				return NodeInput::value(type_default, exposed);
			}

			if let Some(type_default) = TaggedValue::from_type(&ty) {
				return NodeInput::value(type_default, exposed);
			}

			NodeInput::value(TaggedValue::None, true)
		})
		.collect()
}

#[derive(Debug)]
pub enum PreprocessorError {
	ResourceNotFound(ResourceId),
}

impl std::fmt::Display for PreprocessorError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PreprocessorError::ResourceNotFound(id) => write!(f, "Resource not found: {id:?}"),
		}
	}
}

/// Collapses an element-wise node's dual wire registration for one field, `{Item<X>, List<X>}`, to its `List<X>` document wire form.
fn collapse_item_list_pair(types: &HashSet<Type>) -> Option<&Type> {
	let mut types_iterator = types.iter();
	let (first, second) = (types_iterator.next()?, types_iterator.next()?);
	if types_iterator.next().is_some() {
		return None;
	}

	for (item, list) in [(first, second), (second, first)] {
		if let Type::List(list_element) = list.nested_type()
			&& let Type::Item(item_element) = item.nested_type()
			&& list_element == item_element
		{
			return Some(list);
		}
	}

	None
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn item_list_wire_pair_collapses_to_list() {
		let registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
		let identifier = ProtoNodeIdentifier::new("core_types::vector::BoundingBoxNode");
		let implementations = registry.get(&identifier).expect("Bounding Box should be registered");

		let primary_types: HashSet<_> = implementations.iter().map(|(_, node_io)| node_io.inputs[0].clone()).collect();
		assert_eq!(primary_types.len(), 2, "An element-wise node should register Item and List wire variants for its primary input");

		let collapsed = collapse_item_list_pair(&primary_types).expect("The Item/List wire pair should collapse");
		assert!(
			matches!(collapsed.nested_type(), Type::List(_)),
			"The collapse should pick the structural List form, but got {}",
			collapsed.nested_type()
		);
	}

	#[test]
	fn fill_paint_color_default_parses_against_its_list_wire() {
		let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
		let metadata_registry = core_types::registry::NODE_METADATA.lock().unwrap();

		let identifier = graphene_std::vector::fill::IDENTIFIER;
		let implementations = node_registry.get(&identifier).expect("Fill should be registered");
		let first_node_io = implementations.first().map(|(_, node_io)| node_io).expect("Fill should have at least one implementation");
		let metadata = metadata_registry.get(&identifier).expect("Fill should have registered metadata");

		let inputs = node_inputs(&metadata.fields, first_node_io);
		let paint = inputs[1].as_value().expect("The paint input should hold a value");
		assert_eq!(
			*paint,
			TaggedValue::Color(Color::BLACK),
			"The paint input's `Color::BLACK` default should parse against its `List<Graphic>` wire type"
		);
	}
}
