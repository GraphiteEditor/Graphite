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

// Derives the ids for generated nodes from their contents / context to be deterministic arcoss graph compilations.
// The identifier is needed such that different nodes with the same hash bytes get different node ids.
fn producer_node_id(ident: &ProtoNodeIdentifier, key: impl Hash) -> NodeId {
	let mut hasher = DefaultHasher::new();
	"preprocessor producer".hash(&mut hasher);
	ident.hash(&mut hasher);
	key.hash(&mut hasher);
	NodeId(hasher.finish())
}

#[derive(Debug, Clone)]
struct Substitution {
	node: DocumentNode,
	wrapped_node_id: NodeId,
}

#[derive(Debug, Default, Clone)]
pub struct Preprocessor {
	substitutions: HashMap<ProtoNodeIdentifier, Substitution>,
	inject_scopes: HashMap<ProtoNodeIdentifier, (DocumentNode, Type)>,
	compile_metadata: HashMap<ProtoNodeIdentifier, NodeCompileMetadata>,
}

impl Preprocessor {
	pub fn preprocess(&self, network: &mut NodeNetwork, resolve_resource: &dyn Fn(ResourceId) -> Option<ResourceHash>) -> Result<(), PreprocessorError> {
		network.inline_definitions().map_err(PreprocessorError::Definition)?;
		self.insert_inject_scopes(network);
		self.replace_inputs_with_producer_nodes(network, resolve_resource)?;
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

	/// Replace every `TaggedValue::Resource(hash)` and `TaggedValue::AnimationCurve(curve)` input with a reference to a freshly inserted `resource` / `evaluate_curve` proto node.
	fn replace_inputs_with_producer_nodes(&self, network: &mut NodeNetwork, resolve_resource: &dyn Fn(ResourceId) -> Option<ResourceHash>) -> Result<(), PreprocessorError> {
		let mut hash_to_node_id: HashMap<ResourceHash, NodeId> = HashMap::new();
		let mut new_nodes: Vec<(NodeId, DocumentNode)> = Vec::new();

		for (&consumer_id, node) in &mut network.nodes {
			if let DocumentNodeImplementation::Network(nested) = &mut node.implementation {
				self.replace_inputs_with_producer_nodes(nested, resolve_resource)?;
				continue;
			}

			for (input_index, input) in node.inputs.iter_mut().enumerate() {
				let NodeInput::Value { tagged_value, .. } = input else { continue };

				let node_id = match &**tagged_value {
					TaggedValue::Resource(resource_id) => {
						let resource_id = *resource_id;
						let Some(hash) = resolve_resource(resource_id) else {
							return Err(PreprocessorError::ResourceNotFound(resource_id));
						};

						*hash_to_node_id.entry(hash).or_insert_with(|| {
							let id = producer_node_id(&platform_application_io::editor_api::IDENTIFIER, resource_id);
							let resource_node = DocumentNode {
								inputs: vec![
									NodeInput::scope(platform_application_io::application_io::IDENTIFIER),
									NodeInput::value(TaggedValue::ResourceHash(hash), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(platform_application_io::resource::IDENTIFIER),
								..Default::default()
							};
							new_nodes.push((id, resource_node));
							id
						})
					}
					TaggedValue::AnimationCurve(curve) => {
						let id = producer_node_id(&graphene_core::animation::evaluate_curve::IDENTIFIER, (consumer_id, input_index));
						let curve_node = DocumentNode {
							inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::value(TaggedValue::AnimationCurve(curve.clone()), false)],
							implementation: DocumentNodeImplementation::ProtoNode(graphene_core::animation::evaluate_curve::IDENTIFIER),
							..Default::default()
						};
						new_nodes.push((id, curve_node));
						id
					}
					_ => continue,
				};

				*input = NodeInput::node(node_id, 0);
			}
		}

		for (id, node) in new_nodes {
			network.nodes.insert(id, node);
		}

		Ok(())
	}

	fn expand_network(&self, network: &mut NodeNetwork) {
		for node in network.nodes.values_mut() {
			match &mut node.implementation {
				DocumentNodeImplementation::Network(node_network) => self.expand_network(node_network),
				DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
					let compile_metadata = node.compile_metadata.take().or_else(|| self.compile_metadata.get(proto_node_identifier).copied());

					if let Some(Substitution { node: new_node, wrapped_node_id }) = self.substitutions.get(proto_node_identifier) {
						// Reconcile the document node's inputs with what the current node definition expects,
						// since the saved document may have fewer or more inputs than the current version
						while node.inputs.len() < new_node.inputs.len() {
							node.inputs.push(new_node.inputs[node.inputs.len()].clone());
						}
						node.inputs.truncate(new_node.inputs.len());

						node.implementation = new_node.implementation.clone();

						// Move the compile metadata to the wrapped node because networks are not allowed to have compile metadata
						if let DocumentNodeImplementation::Network(network) = &mut node.implementation
							&& let Some(wrapped_node) = network.nodes.get_mut(wrapped_node_id)
						{
							wrapped_node.compile_metadata = compile_metadata;
						}
					} else {
						node.compile_metadata = compile_metadata;
					}
				}
				DocumentNodeImplementation::Extract | DocumentNodeImplementation::Definition(_) => (),
			}
		}
	}

	pub fn new() -> Self {
		let mut substitutions = HashMap::new();
		let mut compile_metadata = HashMap::new();
		let mut inject_scopes = HashMap::new();
		// We pre initialize the node registry here to avoid a deadlock
		let into_node_registry = &*interpreted_executor::node_registry::NODE_REGISTRY;
		let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
		for (id, metadata) in core_types::registry::NODE_METADATA.lock().unwrap().iter() {
			let id = id.clone();

			let NodeMetadata {
				fields,
				flags: NodeFlags {
					memoize,
					footprint_invariant,
					forwards_input,
					inject_scope,
					asynchronous,
				},
				output_fields,
				context_features,
				..
			} = metadata;
			let constant_inputs = fields.iter().enumerate().filter(|(_, field)| field.constant).fold(0, |mask, (index, _)| mask | 1 << index);
			let node_compile_metadata = NodeCompileMetadata {
				footprint_invariant: *footprint_invariant,
				forwards_input: *forwards_input,
				constant_inputs,
				..NodeCompileMetadata::from(context_features.as_slice())
			};
			compile_metadata.insert(id.clone(), node_compile_metadata);

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
			let forwarding = Some(NodeCompileMetadata {
				forwards_input: true,
				..Default::default()
			});

			let mut generated_nodes = 0;
			let mut nodes: HashMap<_, _, _> = node_io_types
				.iter()
				.take(input_count)
				.enumerate()
				.map(|(i, inputs)| {
					if let Some(input_adapter_identifier) = input_adapter_identifier(inputs, into_node_registry) {
						generated_nodes += 1;
						let mut original_location = OriginalLocation::default();
						original_location.auto_convert_index = Some(i);
						// An adapter whose rows also take a second input gets the pipeline, which it uploads a CPU raster wire with
						let takes_pipeline = into_node_registry
							.get(&input_adapter_identifier)
							.is_some_and(|rows| rows.keys().any(|node_io| node_io.inputs.len() == 2));
						let mut adapter_inputs = vec![NodeInput::import(generic!(X), i)];
						if takes_pipeline {
							adapter_inputs.push(NodeInput::scope(graphene_std::raster_nodes::raster_kernel::raster_kernel_pipeline::IDENTIFIER));
						}

						let document_node = DocumentNode {
							inputs: adapter_inputs,
							implementation: DocumentNodeImplementation::ProtoNode(input_adapter_identifier),
							visible: true,
							compile_metadata: forwarding,
							original_location,
							..Default::default()
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
								let mut original_location = OriginalLocation::default();
								original_location.auto_convert_index = Some(i);
								DocumentNode {
									inputs: vec![NodeInput::import(input.clone(), i)],
									implementation: DocumentNodeImplementation::ProtoNode(passthrough_node.clone()),
									visible: true,
									compile_metadata: forwarding,
									original_location,
									..Default::default()
								}
							}
							None => DocumentNode {
								inputs: vec![NodeInput::import(generic!(X), i)],
								implementation: DocumentNodeImplementation::ProtoNode(passthrough_node.clone()),
								visible: false,
								compile_metadata: forwarding,
								..Default::default()
							},
						},
					)
				})
				.collect();

			let output_fields = output_fields.as_ref();

			// A multi-output node's outputs share one evaluation through a Memoize node
			let memoize = *memoize || output_fields.is_some();
			debug_assert!(
				output_fields.is_none()
					|| into_node_registry.get(&graphene_core::memo::memoize::IDENTIFIER).is_some_and(|memoize_implementations| {
						implementations.iter().all(|(_, node_io)| {
							let struct_type = node_io.return_value.nested_type();
							memoize_implementations.keys().any(|memoize_io| memoize_io.return_value.nested_type() == struct_type)
						})
					}),
				"The multi-output node {} has no Memoize implementation for the struct it returns",
				id.as_str()
			);

			if generated_nodes == 0 && !memoize && !inject_scope && output_fields.is_none() && !asynchronous {
				continue;
			}

			let document_node = DocumentNode {
				inputs: network_inputs,
				call_argument: input_type.clone(),
				implementation: DocumentNodeImplementation::ProtoNode(id.clone()),
				visible: true,
				skip_deduplication: false,
				..Default::default()
			};

			let wrapped_node_id = NodeId(input_count as u64);
			nodes.insert(wrapped_node_id, document_node);

			// If memoize is requested, append a Memoize node after the main node and redirect the export through it
			let export_node_id = if memoize {
				let memoize_node_id = NodeId(input_count as u64 + 1);
				let memoize_node = DocumentNode {
					inputs: vec![NodeInput::node(wrapped_node_id, 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::memo::memoize::IDENTIFIER.clone()),
					visible: true,
					compile_metadata: forwarding,
					..Default::default()
				};
				nodes.insert(memoize_node_id, memoize_node);
				memoize_node_id
			} else {
				wrapped_node_id
			};

			// A multi-output node exports each field through its generated extractor node, after the hidden primary output if it has one
			let mut exports = Vec::new();
			if output_fields.is_none_or(|output_fields| output_fields.hidden_primary_output()) {
				exports.push(NodeInput::Node {
					node_id: export_node_id,
					output_index: 0,
				});
			}
			if let Some(output_fields) = output_fields {
				for (field_index, field) in output_fields.fields.iter().enumerate() {
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
					definitions: Default::default(),
					generated: true,
				}),
				visible: true,
				skip_deduplication: false,
				..Default::default()
			};

			substitutions.insert(id.clone(), Substitution { node, wrapped_node_id });

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
					..Default::default()
				};
				inject_scopes.insert(id.clone(), (template, node_io.return_value.clone()));
			}
		}

		Self {
			substitutions,
			compile_metadata,
			inject_scopes,
		}
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
			let exposed = if index == 0 { ty != fn_type!(Context, ()) } else { field.exposed };

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
				RegistryValueSource::SourceId => return NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::SourceId),
			};

			// A ranked `Item<T>` type prefers a bare `T` value (promoted at resolution), since bare values drive the Inspector widgets
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
	Definition(DefinitionError),
}

#[cfg(test)]
mod destructure_tests {
	use super::*;
	use core_types::list::{Item, List};
	use core_types::registry::Destructure;
	use glam::DVec2;
	use graph_craft::graphene_compiler::Compiler;
	use interpreted_executor::dynamic_executor::DynamicExecutor;

	/// Test-only struct with a `#[primary]` field, exercising the primary-output layout.
	#[derive(Debug, Clone, dyn_any::DynAny, node_macro::Destructure)]
	pub struct SumProduct {
		/// The sum of the two inputs.
		#[primary]
		sum: Item<f64>,
		/// The product of the two inputs.
		product: Item<f64>,
	}

	#[node_macro::node(category(""))]
	fn sum_product(_: impl core_types::Ctx, a: Item<f64>, b: Item<f64>) -> SumProduct {
		let (a, b) = (a.into_element(), b.into_element());

		SumProduct {
			sum: Item::new_from_element(a + b),
			product: Item::new_from_element(a * b),
		}
	}

	/// Test-only struct of mixed ranks, returned by a whole-list node.
	#[derive(Debug, Clone, dyn_any::DynAny, node_macro::Destructure)]
	pub struct DoubledAndCount {
		/// Each input value doubled.
		doubled: List<f64>,
		/// How many values were given.
		count: Item<f64>,
	}

	#[node_macro::node(category(""))]
	fn doubled_and_count(_: impl core_types::Ctx, values: List<f64>) -> DoubledAndCount {
		let count = Item::new_from_element(values.len() as f64);
		let doubled = values
			.into_iter()
			.map(|value| {
				let (value, attributes) = value.into_parts();
				Item::from_parts(value * 2., attributes)
			})
			.collect();

		DoubledAndCount { doubled, count }
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

	/// A network where a multi-output Vec2 to Numbers node's X and Y outputs (indices 1 and 2, after the hidden primary) feed an Add node.
	fn vec2_to_numbers_network() -> NodeNetwork {
		let vec2_to_numbers = DocumentNode {
			inputs: vec![NodeInput::value(TaggedValue::DVec2(DVec2::new(3., 5.)), false)],
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::math_nodes::vec_2_to_numbers::IDENTIFIER),
			..Default::default()
		};
		multi_output_into_add_network(vec2_to_numbers, [1, 2])
	}

	fn assert_execution_result(network: NodeNetwork, expected: TaggedValue) {
		let proto_network = Compiler {}.compile_single(network).expect("Compilation should succeed").network;
		let executor = DynamicExecutor::new(proto_network).expect("The executor should type check and build");

		let context: core_types::Context = None;
		let result = executor.tree().eval_tagged_value(executor.output(), &context).expect("Execution should succeed");
		assert_eq!(result, core_types::Poll::Final(expected));
	}

	#[test]
	fn multi_output_node_expands_into_generated_destructure_network() {
		let vec2_to_numbers_identifier = graphene_std::math_nodes::vec_2_to_numbers::IDENTIFIER;
		let destructure = registry::MULTI_OUTPUT_NODES
			.get(&vec2_to_numbers_identifier)
			.expect("Vec2 to Numbers should be registered as a multi-output node");
		assert_eq!(destructure.fields.iter().map(|field| field.name).collect::<Vec<_>>(), vec!["X", "Y"]);
		assert!(destructure.hidden_primary_output());

		let mut network = vec2_to_numbers_network();
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");

		// The multi-output node is substituted with a transient generated network: the struct as the hidden primary export,
		// followed by one export per field, each pulled out of the struct by that field's extractor node
		let node = network.nodes.get(&NodeId(0)).unwrap();
		let DocumentNodeImplementation::Network(generated) = &node.implementation else {
			panic!("The multi-output node should be substituted with a generated network")
		};
		assert!(generated.generated, "The substituted network must be marked as generated so it stays out of node paths");
		assert_eq!(generated.exports.len(), destructure.number_of_outputs());

		// The struct is computed once and shared through a Memoize node
		let Some(NodeInput::Node { node_id: struct_source_id, .. }) = generated.exports.first() else {
			panic!("Export 0 should come from a node")
		};
		let struct_source = generated.nodes.get(struct_source_id).unwrap();
		assert_eq!(struct_source.implementation, DocumentNodeImplementation::ProtoNode(graphene_core::memo::memoize::IDENTIFIER));

		let Some(NodeInput::Node { node_id: main_node_id, .. }) = struct_source.inputs.first() else {
			panic!("The Memoize node should pull from the struct-producing node")
		};
		let main_node = generated.nodes.get(main_node_id).unwrap();
		assert_eq!(main_node.implementation, DocumentNodeImplementation::ProtoNode(vec2_to_numbers_identifier));

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
		let mut network = vec2_to_numbers_network();
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");

		// X + Y of (3, 5) should be 8
		assert_execution_result(network, TaggedValue::Number(8.));
	}

	#[test]
	fn primary_field_becomes_the_primary_output() {
		let identifier = sum_product::IDENTIFIER;
		let destructure = registry::MULTI_OUTPUT_NODES.get(&identifier).expect("Sum Product should be registered as a multi-output node");
		assert!(!destructure.hidden_primary_output());
		assert_eq!(destructure.fields.iter().map(|field| field.name).collect::<Vec<_>>(), vec!["Sum", "Product"]);

		let node = DocumentNode {
			inputs: vec![NodeInput::value(TaggedValue::Number(3.), false), NodeInput::value(TaggedValue::Number(5.), false)],
			implementation: DocumentNodeImplementation::ProtoNode(identifier),
			..Default::default()
		};
		let mut network = multi_output_into_add_network(node, [0, 1]);
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");

		// With a `#[primary]` field there is no hidden primary output, so there's one export per field
		let node = network.nodes.get(&NodeId(0)).unwrap();
		let DocumentNodeImplementation::Network(generated) = &node.implementation else {
			panic!("The multi-output node should be substituted with a generated network")
		};
		assert_eq!(generated.exports.len(), destructure.number_of_outputs());
		for (field, export) in destructure.fields.iter().zip(&generated.exports) {
			let NodeInput::Node { node_id: extractor_id, .. } = export else {
				panic!("Each field export should come from an extractor node")
			};
			let extractor = generated.nodes.get(extractor_id).unwrap();
			assert_eq!(extractor.implementation, DocumentNodeImplementation::ProtoNode(field.extractor.clone()));
		}

		// Sum + product of (3, 5) should be 8 + 15 = 23
		assert_execution_result(network, TaggedValue::Number(23.));
	}

	#[test]
	fn framed_multi_output_node_returns_the_mapped_struct() {
		let destructure = registry::MULTI_OUTPUT_NODES
			.get(&sum_product::IDENTIFIER)
			.expect("Sum Product should be registered as a multi-output node");
		assert_eq!(destructure.mapped_type, concrete!(SumProductMapped));
		assert!(destructure.fields.iter().all(|field| field.ty == core_types::item!(f64) && field.mapped_ty == core_types::list!(f64)));

		// The node registers a mapped variant returning the mapped struct alongside its single-item form
		let node_registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
		let rows = node_registry.get(&sum_product::IDENTIFIER).expect("Sum Product should have registered implementations");
		assert!(rows.iter().any(|(_, node_io)| *node_io.return_value.nested_type() == concrete!(SumProduct)));
		assert!(rows.iter().any(|(_, node_io)| *node_io.return_value.nested_type() == destructure.mapped_type));
		drop(node_registry);

		// A list on one connector frames the node, so the sums [11, 12, 13] and products [10, 20, 30] add to [21, 32, 43]
		let node = DocumentNode {
			inputs: vec![NodeInput::value(TaggedValue::Numbers(vec![1., 2., 3.]), false), NodeInput::value(TaggedValue::Number(10.), false)],
			implementation: DocumentNodeImplementation::ProtoNode(sum_product::IDENTIFIER),
			..Default::default()
		};
		let mut network = multi_output_into_add_network(node, [0, 1]);
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");
		assert_execution_result(network, TaggedValue::Numbers(vec![21., 32., 43.]));
	}

	#[test]
	fn whole_list_multi_output_node_returns_fields_of_mixed_rank() {
		let destructure = registry::MULTI_OUTPUT_NODES
			.get(&doubled_and_count::IDENTIFIER)
			.expect("Doubled And Count should be registered as a multi-output node");
		assert_eq!(
			destructure.fields.iter().map(|field| field.ty.clone()).collect::<Vec<_>>(),
			vec![core_types::list!(f64), core_types::item!(f64)]
		);

		// The doubled list [2, 4, 6] and the count 3 add to [5, 7, 9]
		let node = DocumentNode {
			inputs: vec![NodeInput::value(TaggedValue::Numbers(vec![1., 2., 3.]), false)],
			implementation: DocumentNodeImplementation::ProtoNode(doubled_and_count::IDENTIFIER),
			..Default::default()
		};
		let mut network = multi_output_into_add_network(node, [1, 2]);
		Preprocessor::new().preprocess(&mut network, &|_| None).expect("Preprocessing should succeed");
		assert_execution_result(network, TaggedValue::Numbers(vec![5., 7., 9.]));
	}

	#[test]
	fn pushing_into_the_mapped_struct_lifts_item_fields_and_concatenates_list_fields() {
		let mut mapped = DoubledAndCount::mapped_with_capacity(2);
		for count in [1., 2.] {
			let doubled = [count, count].into_iter().map(Item::new_from_element).collect();
			DoubledAndCount {
				doubled,
				count: Item::new_from_element(count),
			}
			.push_into(&mut mapped);
		}

		assert_eq!(mapped.doubled.iter_element_values().copied().collect::<Vec<_>>(), vec![1., 1., 2., 2.]);
		assert_eq!(mapped.count.iter_element_values().copied().collect::<Vec<_>>(), vec![1., 2.]);
	}
}

impl std::fmt::Display for PreprocessorError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PreprocessorError::ResourceNotFound(id) => write!(f, "Resource not found: {id:?}"),
			PreprocessorError::Definition(error) => write!(f, "{error}"),
		}
	}
}

/// The `input_adapter` placed ahead of a node input registered with `input_types`, so ranked wires pass through and convertible elements are cast.
/// Only an input registered with one ranked type, or with the `Item` and `List` forms of one element, gets one (if the registry has it for that element).
pub fn input_adapter_identifier<Implementations>(input_types: &HashSet<Type>, node_registry: &HashMap<ProtoNodeIdentifier, Implementations>) -> Option<ProtoNodeIdentifier> {
	let input_type = match input_types.len() {
		1 => input_types.iter().next()?,
		_ => collapse_item_list_pair(input_types)?,
	};
	let element_name = match input_type.nested_type() {
		// Type names drop the generics that tell a GPU raster from a CPU one
		Type::Item(element) | Type::List(element) if **element == concrete!(raster_types::Raster<raster_types::GPU>) => "Raster<GPU>".to_string(),
		Type::Item(element) | Type::List(element) => element.identifier_name(),
		nested if nested.identifier_name() == "ListDyn" => "ListDyn".to_string(),
		_ => return None,
	};

	let identifier = ProtoNodeIdentifier::with_owned_string(format!("input_adapter<{element_name}>"));
	node_registry.contains_key(&identifier).then_some(identifier)
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
		let identifier = ProtoNodeIdentifier::new("core_types::vector::DimensionsNode");
		let implementations = registry.get(&identifier).expect("Dimensions should be registered");

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
	fn fill_paint_color_default_parses_against_its_graphic_wire() {
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
			"The paint input's `Color::BLACK` default should parse against its `Item<Graphic>` wire type"
		);
	}

	#[test]
	fn substitution_moves_compile_metadata_onto_the_wrapped_node() {
		let identifier = graphene_std::path_bool_nodes::boolean_operation::IDENTIFIER;
		let compile_metadata = NodeCompileMetadata {
			extract: ContextFeatures::FOOTPRINT,
			inject: ContextFeatures::INDEX,
			..Default::default()
		};
		let mut network = NodeNetwork {
			nodes: [(
				NodeId(0),
				DocumentNode {
					implementation: DocumentNodeImplementation::ProtoNode(identifier.clone()),
					compile_metadata: Some(compile_metadata),
					..Default::default()
				},
			)]
			.into_iter()
			.collect(),
			..Default::default()
		};

		Preprocessor::new().expand_network(&mut network);

		let node = &network.nodes[&NodeId(0)];
		assert_eq!(node.compile_metadata, None, "The network node replacing the substituted node should not keep its compile metadata");
		let DocumentNodeImplementation::Network(substitution) = &node.implementation else {
			panic!("Boolean Operation is memoized, so it should be substituted with a network");
		};

		let wrapped_node = substitution
			.nodes
			.values()
			.find(|inner| inner.implementation == DocumentNodeImplementation::ProtoNode(identifier.clone()))
			.expect("The substitution network should contain the wrapped proto node");
		assert_eq!(wrapped_node.compile_metadata, Some(compile_metadata), "The wrapped proto node should carry the override");

		let NodeInput::Node { node_id: export_id, .. } = substitution.exports[0] else {
			panic!("The substitution network should export a node");
		};
		let export = &substitution.nodes[&export_id];
		assert_eq!(export.implementation, DocumentNodeImplementation::ProtoNode(graphene_core::memo::memoize::IDENTIFIER));
		let export_compile_metadata = export.compile_metadata.unwrap_or_default();
		assert!(export_compile_metadata.forwards_input, "The generated memoize node should forward its input");
		assert_eq!(
			export_compile_metadata.extract,
			ContextFeatures::empty(),
			"The memoize node should not take over the wrapped node's context features"
		);
	}

	#[test]
	fn node_attributes_reach_the_compile_metadata() {
		use graphene_std::graphic::{read_attribute_number, write_attribute};

		let preprocessor = Preprocessor::new();
		let compile_metadata = |identifier: &ProtoNodeIdentifier| preprocessor.compile_metadata.get(identifier).copied().unwrap_or_default();

		for (identifier, name_index) in [
			(write_attribute::IDENTIFIER, <write_attribute::NameInput as NodeParameter>::INDEX),
			(read_attribute_number::IDENTIFIER, <read_attribute_number::NameInput as NodeParameter>::INDEX),
		] {
			let info = compile_metadata(&identifier);
			assert!(info.requires_constant_input(name_index), "{identifier:?} should require a constant name");
			assert_eq!(info.constant_inputs.count_ones(), 1, "{identifier:?} should only require its name to be constant");
		}

		for identifier in [graphene_core::memo::memoize::IDENTIFIER, graphene_core::memo::monitor::IDENTIFIER, ops::passthrough::IDENTIFIER] {
			assert!(compile_metadata(&identifier).forwards_input, "{identifier:?} should forward its input");
		}
	}

	#[test]
	fn a_cpu_raster_wire_uploads_through_one_adapter_that_ignores_the_footprint() {
		use graph_craft::application_io::PlatformEditorApi;
		use graph_craft::document::value::TaggedValue;
		use graph_craft::graphene_compiler::Compiler;
		use graph_craft::item;
		use graph_craft::proto::TypingContext;
		use graphene_std::raster_nodes::raster_kernel::raster_kernel_pipeline;
		use graphene_std::raster_nodes::std_nodes::channels_to_image;
		use graphene_std::raster_types::{CPU, Raster};
		use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
		use interpreted_executor::util::wrap_network_in_scope;
		use std::sync::Arc;

		struct NoSpawner;
		impl Spawn for NoSpawner {
			fn spawn(&self, _: Task) {}
		}

		// A CPU raster wired to all four of a node's inputs which only take GPU rasters
		let source = NodeId(0);
		let wire = || NodeInput::node(source, 0);
		let network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [
				(
					source,
					DocumentNode {
						inputs: vec![NodeInput::value(TaggedValue::TypeDefault(item!(Raster<CPU>)), false)],
						implementation: DocumentNodeImplementation::ProtoNode(ops::passthrough::IDENTIFIER),
						..Default::default()
					},
				),
				(
					NodeId(1),
					DocumentNode {
						inputs: vec![
							NodeInput::value(TaggedValue::None, false),
							wire(),
							wire(),
							wire(),
							wire(),
							NodeInput::scope(raster_kernel_pipeline::IDENTIFIER),
						],
						implementation: DocumentNodeImplementation::ProtoNode(channels_to_image::IDENTIFIER),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};

		let runtime = AsyncRuntime::new(Arc::new(NoSpawner), Arc::new(std::time::Instant::now()));
		let mut network = wrap_network_in_scope(network, Arc::new(PlatformEditorApi::default()), runtime.handle());
		Preprocessor::new().preprocess(&mut network, &|_| None).unwrap();
		let proto_network = Compiler {}.compile_single(network).unwrap().network;

		// The compiled graph type checks, since a CPU raster feeds the GPU-only inputs through the adapter
		let mut typing_context = TypingContext::new(&interpreted_executor::node_registry::NODE_REGISTRY);
		typing_context.update(&proto_network).expect("the CPU raster wire should reach the GPU-only inputs");

		let identifier = ProtoNodeIdentifier::new("input_adapter<Raster<GPU>>");
		let adapters: Vec<_> = proto_network.nodes.iter().filter(|(_, node)| node.identifier == identifier).collect();
		let distinct_ids: HashSet<_> = adapters.iter().map(|(id, _)| *id).collect();
		assert_eq!(
			distinct_ids.len(),
			1,
			"the identical adapters of the four inputs should share one stable ID, and so one node in the executor"
		);

		let (_, adapter) = adapters[0];
		assert!(
			!adapter.upstream_context_features.contains(core_types::ContextFeatures::FOOTPRINT),
			"neither the adapter nor what feeds it should read the footprint, leaving it unaffected by panning"
		);
	}
	#[test]
	fn a_footprint_dependent_raster_warns_at_a_footprint_invariant_node_and_feeds_others_unconverted() {
		use graph_craft::application_io::PlatformEditorApi;
		use graph_craft::document::value::TaggedValue;
		use graph_craft::graphene_compiler::Compiler;
		use graph_craft::proto::{GraphErrorType, TypingContext};
		use graphene_std::raster_nodes::dehaze::dehaze;
		use graphene_std::raster_nodes::filter::median_filter;
		use graphene_std::raster_nodes::raster_kernel::raster_kernel_pipeline;
		use graphene_std::raster_nodes::std_nodes::mandelbrot;
		use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
		use interpreted_executor::util::wrap_network_in_scope;
		use std::sync::Arc;

		struct NoSpawner;
		impl Spawn for NoSpawner {
			fn spawn(&self, _: Task) {}
		}

		let compile = |consumer: ProtoNodeIdentifier| {
			let network = NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					(
						NodeId(0),
						DocumentNode {
							inputs: vec![NodeInput::scope(raster_kernel_pipeline::IDENTIFIER)],
							implementation: DocumentNodeImplementation::ProtoNode(mandelbrot::IDENTIFIER),
							..Default::default()
						},
					),
					(
						NodeId(1),
						DocumentNode {
							inputs: vec![
								NodeInput::node(NodeId(0), 0),
								NodeInput::value(TaggedValue::Number(10.), false),
								NodeInput::scope(raster_kernel_pipeline::IDENTIFIER),
							],
							implementation: DocumentNodeImplementation::ProtoNode(consumer),
							..Default::default()
						},
					),
				]
				.into_iter()
				.collect(),
				..Default::default()
			};

			let runtime = AsyncRuntime::new(Arc::new(NoSpawner), Arc::new(std::time::Instant::now()));
			let mut network = wrap_network_in_scope(network, Arc::new(PlatformEditorApi::default()), runtime.handle());
			Preprocessor::new().preprocess(&mut network, &|_| None).unwrap();
			let compiled = Compiler {}.compile_single(network).unwrap();

			let mut typing_context = TypingContext::new(&interpreted_executor::node_registry::NODE_REGISTRY);
			typing_context.update(&compiled.network).expect("a GPU raster should reach the GPU-only input");
			compiled.warnings
		};

		let warnings = compile(dehaze::IDENTIFIER);
		assert_eq!(warnings.len(), 1, "Dehaze should warn about reading the footprint through its input: {warnings:?}");
		assert!(warnings[0].error == GraphErrorType::FootprintDependentInput);

		assert!(compile(median_filter::IDENTIFIER).is_empty(), "a node that isn't footprint invariant shouldn't warn");
	}

	#[test]
	fn generator_nodes_compile_on_their_own_and_dont_reach_gpu_only_inputs() {
		use graph_craft::application_io::PlatformEditorApi;
		use graph_craft::document::value::TaggedValue;
		use graph_craft::graphene_compiler::Compiler;
		use graph_craft::proto::TypingContext;
		use graphene_std::raster_nodes::filter::median_filter;
		use graphene_std::raster_nodes::raster_kernel::raster_kernel_pipeline;
		use graphene_std::raster_nodes::std_nodes::{mandelbrot_generator, noise_pattern_generator};
		use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
		use interpreted_executor::util::wrap_network_in_scope;
		use std::sync::Arc;

		struct NoSpawner;
		impl Spawn for NoSpawner {
			fn spawn(&self, _: Task) {}
		}

		// Each generator's settings at their type's default, after the empty primary input
		let generator_node = |identifier: &ProtoNodeIdentifier| {
			let setting_types: Vec<Type> = {
				let registry = core_types::registry::NODE_REGISTRY.lock().unwrap();
				let (_, io) = registry[identifier].iter().next().unwrap();
				io.inputs.iter().skip(1).map(|input| input.nested_type().clone()).collect()
			};
			let settings = setting_types.into_iter().map(|setting| NodeInput::value(TaggedValue::TypeDefault(setting), false));
			DocumentNode {
				inputs: std::iter::once(NodeInput::value(TaggedValue::None, false)).chain(settings).collect(),
				implementation: DocumentNodeImplementation::ProtoNode(identifier.clone()),
				..Default::default()
			}
		};
		let compile = |nodes: Vec<(NodeId, DocumentNode)>, export: NodeId| {
			let network = NodeNetwork {
				exports: vec![NodeInput::node(export, 0)],
				nodes: nodes.into_iter().collect(),
				..Default::default()
			};
			let runtime = AsyncRuntime::new(Arc::new(NoSpawner), Arc::new(std::time::Instant::now()));
			let mut network = wrap_network_in_scope(network, Arc::new(PlatformEditorApi::default()), runtime.handle());
			Preprocessor::new().preprocess(&mut network, &|_| None).unwrap();
			Compiler {}.compile_single(network).unwrap()
		};

		for identifier in [mandelbrot_generator::IDENTIFIER, noise_pattern_generator::IDENTIFIER] {
			let compiled = compile(vec![(NodeId(0), generator_node(&identifier))], NodeId(0));
			let mut typing_context = TypingContext::new(&interpreted_executor::node_registry::NODE_REGISTRY);
			typing_context.update(&compiled.network).expect("a generator should compile on its own");
			assert!(compiled.warnings.is_empty(), "{identifier:?} shouldn't warn: {:?}", compiled.warnings);

			let median = DocumentNode {
				inputs: vec![
					NodeInput::node(NodeId(0), 0),
					NodeInput::value(TaggedValue::Number(10.), false),
					NodeInput::scope(raster_kernel_pipeline::IDENTIFIER),
				],
				implementation: DocumentNodeImplementation::ProtoNode(median_filter::IDENTIFIER),
				..Default::default()
			};
			let compiled = compile(vec![(NodeId(0), generator_node(&identifier)), (NodeId(1), median)], NodeId(1));
			let mut typing_context = TypingContext::new(&interpreted_executor::node_registry::NODE_REGISTRY);
			assert!(typing_context.update(&compiled.network).is_err(), "{identifier:?} shouldn't reach a GPU-only input");
		}
	}
}
