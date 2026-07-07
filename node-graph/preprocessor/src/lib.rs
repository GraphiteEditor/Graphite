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

			// Nodes returning a `#[node_macro::destructure]` struct are multi-output: they always need a substitution
			// so their generated network can export each struct field through a hidden extractor node
			let destructure = destructure_metadata_for_type(&first_node_io.return_value);

			// A multi-output node is otherwise evaluated once per connected output, so when a Memoize implementation
			// is registered for its struct type, wrap the struct in one so all the extractors share a single evaluation.
			// Rows are matched by element type name, since the executor registry's structural rows carry no element TypeId.
			let memoize_row_for_struct = |ty: &Type| {
				let element_type = match ty.nested_type() {
					Type::Item(inner) | Type::List(inner) => inner.nested_type(),
					other => other,
				};
				let Type::Concrete(descriptor) = element_type else { return false };
				destructure.as_ref().is_some_and(|metadata| descriptor.name == metadata.struct_name)
			};
			let memoize = *memoize
				|| into_node_registry
					.get(&graphene_core::memo::memoize::IDENTIFIER)
					.is_some_and(|implementations| implementations.keys().any(|node_io| memoize_row_for_struct(&node_io.return_value)));

			if generated_nodes == 0 && !memoize && !inject_scope && destructure.is_none() {
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
			let export_node_id = if memoize {
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

			// A multi-output node exports each struct field through that field's generated extractor node. When one
			// field is marked `#[primary]` its extractor becomes export 0; otherwise export 0 carries the struct
			// itself, which stays hidden in the UI as the node's primary output
			let mut exports = Vec::new();
			if destructure.as_ref().is_none_or(|destructure| !destructure.has_primary) {
				exports.push(NodeInput::Node {
					node_id: export_node_id,
					output_index: 0,
				});
			}
			if let Some(destructure) = &destructure {
				for (field_index, field) in destructure.fields.iter().enumerate() {
					let extractor_node_id = NodeId(export_node_id.0 + 1 + field_index as u64);
					let extractor_node = DocumentNode {
						inputs: vec![NodeInput::node(export_node_id, 0)],
						implementation: DocumentNodeImplementation::ProtoNode(field.extractor.clone()),
						visible: true,
						..Default::default()
					};
					nodes.insert(extractor_node_id, extractor_node);
					exports.push(NodeInput::node(extractor_node_id, 0));
				}
			}

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

#[cfg(test)]
mod destructure_tests {
	use super::*;
	use core_types::list::Item;
	use glam::DVec2;
	use graph_craft::graphene_compiler::Compiler;
	use interpreted_executor::dynamic_executor::DynamicExecutor;

	/// Test-only multi-output struct with a `#[primary]` field, exercising the primary-output layout and the
	/// unmemoized path (no Memoize implementation is registered for this struct type).
	#[node_macro::destructure]
	#[derive(Debug, Clone, Copy, dyn_any::DynAny)]
	pub struct SumProduct {
		/// The sum of the two inputs.
		#[primary]
		sum: f64,
		/// The product of the two inputs.
		product: f64,
	}

	#[node_macro::node(category(""))]
	fn sum_product(_: impl core_types::Ctx, a: Item<f64>, b: Item<f64>) -> Item<SumProduct> {
		let (a, b) = (a.into_element(), b.into_element());

		Item::new_from_element(SumProduct { sum: a + b, product: a * b })
	}

	/// A network where the outputs of the given multi-output node feed an Add node.
	/// Includes a stub "editor-api" scope injection, which preprocessing requires and `wrap_network_in_scope` normally provides.
	fn multi_output_into_add_network(node: DocumentNode, added_output_indices: [usize; 2]) -> NodeNetwork {
		NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [
				(NodeId(0), node),
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![NodeInput::node(NodeId(0), added_output_indices[0]), NodeInput::node(NodeId(0), added_output_indices[1])],
						implementation: DocumentNodeImplementation::ProtoNode(graphene_std::math_nodes::add::IDENTIFIER),
						..Default::default()
					},
				),
				(
					NodeId(2),
					DocumentNode {
						inputs: vec![NodeInput::value(TaggedValue::EditorApi(std::sync::Arc::default()), false)],
						implementation: DocumentNodeImplementation::ProtoNode(ops::passthrough::IDENTIFIER),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			scope_injections: [("editor-api".to_string(), (NodeId(2), concrete!(&graph_craft::application_io::PlatformEditorApi)))]
				.into_iter()
				.collect(),
			..Default::default()
		}
	}

	/// A network where a multi-output Split Vec2 node's X and Y outputs (indices 1 and 2, after the hidden primary) feed an Add node.
	fn split_vec2_network() -> NodeNetwork {
		let split_vec2 = DocumentNode {
			inputs: vec![NodeInput::value(TaggedValue::DVec2(DVec2::new(3., 5.)), false)],
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::extract_xy::split_vec_2::IDENTIFIER),
			..Default::default()
		};
		multi_output_into_add_network(split_vec2, [1, 2])
	}

	fn assert_execution_result(network: NodeNetwork, expected: TaggedValue) {
		let proto_network = Compiler {}.compile_single(network).expect("Compilation should succeed");
		let executor = futures::executor::block_on(DynamicExecutor::new(proto_network)).expect("The executor should type check and build");

		let context: core_types::Context = None;
		let result = futures::executor::block_on(executor.tree().eval_tagged_value(executor.output(), context)).expect("Execution should succeed");
		assert_eq!(result, expected);
	}

	#[test]
	fn multi_output_node_expands_into_generated_destructure_network() {
		let split_vec2_identifier = graphene_std::extract_xy::split_vec_2::IDENTIFIER;
		let destructure = registry::MULTI_OUTPUT_NODES
			.get(&split_vec2_identifier)
			.expect("Split Vec2 should be registered as a multi-output node");
		assert_eq!(destructure.fields.iter().map(|field| field.name).collect::<Vec<_>>(), vec!["X", "Y"]);
		assert!(!destructure.has_primary);

		let mut network = split_vec2_network();
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");

		// The multi-output node is substituted with a transient generated network: the struct as the hidden primary export,
		// followed by one export per field, each pulled out of the struct by that field's extractor node
		let node = network.nodes.get(&NodeId(0)).unwrap();
		let DocumentNodeImplementation::Network(generated) = &node.implementation else {
			panic!("The multi-output node should be substituted with a generated network")
		};
		assert!(generated.generated, "The substituted network must be marked as generated so it stays out of node paths");
		assert_eq!(generated.exports.len(), 1 + destructure.fields.len());

		// A Memoize implementation is registered for Vec2Components, so the struct is computed once and shared through it
		let Some(NodeInput::Node { node_id: struct_source_id, .. }) = generated.exports.first() else {
			panic!("Export 0 should come from a node")
		};
		let struct_source = generated.nodes.get(struct_source_id).unwrap();
		assert_eq!(struct_source.implementation, DocumentNodeImplementation::ProtoNode(graphene_core::memo::memoize::IDENTIFIER));

		let Some(NodeInput::Node { node_id: main_node_id, .. }) = struct_source.inputs.first() else {
			panic!("The Memoize node should pull from the struct-producing node")
		};
		let main_node = generated.nodes.get(main_node_id).unwrap();
		assert_eq!(main_node.implementation, DocumentNodeImplementation::ProtoNode(split_vec2_identifier));

		for (field, export) in destructure.fields.iter().zip(&generated.exports[1..]) {
			let NodeInput::Node { node_id: extractor_id, .. } = export else {
				panic!("Each field export should come from an extractor node")
			};
			let extractor = generated.nodes.get(extractor_id).unwrap();
			assert_eq!(extractor.implementation, DocumentNodeImplementation::ProtoNode(field.extractor.clone()));
			assert_eq!(extractor.inputs, vec![NodeInput::node(*struct_source_id, 0)], "Each extractor should share the memoized struct");
		}
	}

	#[test]
	fn multi_output_node_compiles_and_executes() {
		let mut network = split_vec2_network();
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");

		// X + Y of (3, 5) should be 8
		assert_execution_result(network, TaggedValue::F64(8.));
	}

	#[test]
	fn primary_field_becomes_the_primary_output() {
		let identifier = sum_product::IDENTIFIER;
		let destructure = registry::MULTI_OUTPUT_NODES.get(&identifier).expect("Sum Product should be registered as a multi-output node");
		assert!(destructure.has_primary);
		assert_eq!(destructure.fields.iter().map(|field| field.name).collect::<Vec<_>>(), vec!["Sum", "Product"]);

		let node = DocumentNode {
			inputs: vec![NodeInput::value(TaggedValue::F64(3.), false), NodeInput::value(TaggedValue::F64(5.), false)],
			implementation: DocumentNodeImplementation::ProtoNode(identifier),
			..Default::default()
		};
		let mut network = multi_output_into_add_network(node, [0, 1]);
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");

		// With a `#[primary]` field there is no hidden struct export: one export per field, with the primary field first
		let node = network.nodes.get(&NodeId(0)).unwrap();
		let DocumentNodeImplementation::Network(generated) = &node.implementation else {
			panic!("The multi-output node should be substituted with a generated network")
		};
		assert_eq!(generated.exports.len(), destructure.fields.len());
		for (field, export) in destructure.fields.iter().zip(&generated.exports) {
			let NodeInput::Node { node_id: extractor_id, .. } = export else {
				panic!("Each field export should come from an extractor node")
			};
			let extractor = generated.nodes.get(extractor_id).unwrap();
			assert_eq!(extractor.implementation, DocumentNodeImplementation::ProtoNode(field.extractor.clone()));
		}

		// Sum + product of (3, 5) should be 8 + 15 = 23
		assert_execution_result(network, TaggedValue::F64(23.));
	}
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
