use std::collections::{HashMap, HashSet};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNodeImplementation, InlineRust, NodeInput};
use graph_craft::{ProtoNodeIdentifier, Type, concrete};
use graphene_std::uuid::NodeId;
use interpreted_executor::dynamic_executor::{NodeTypes, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::node_registry::NODE_REGISTRY;

use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};

// This file contains utility methods for interfacing with the resolved types returned from the compiler
#[derive(Debug, Default)]
pub struct ResolvedDocumentNodeTypes {
	pub types: HashMap<Vec<NodeId>, NodeTypes>,
}

impl ResolvedDocumentNodeTypes {
	pub fn update(&mut self, delta: ResolvedDocumentNodeTypesDelta) {
		for (path, node_type) in delta.add {
			self.types.insert(path.to_vec(), node_type);
		}
		for path in delta.remove {
			self.types.remove(&path.to_vec());
		}
	}
}

/// Represents the result of a type query for an input or output connector.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeSource {
	// A type that has been compiled based on all upstream types
	Compiled(Type),
	// The type of value inputs
	TaggedValue(Type),
	// A type that is guessed from the document node definition
	DocumentNodeDefinition(Type),
	// When the input is not compiled, the type is unknown and must be guessed from the valid types
	Unknown,

	Error(&'static str),
}

impl TypeSource {
	/// The reduced set of frontend types for displaying color.
	pub fn displayed_type(&self) -> FrontendGraphDataType {
		match self.compiled_nested_type() {
			Some(nested_type) => match TaggedValue::from_type_or_none(nested_type) {
				TaggedValue::U32(_)
				| TaggedValue::U64(_)
				| TaggedValue::F32(_)
				| TaggedValue::F64(_)
				| TaggedValue::DVec2(_)
				| TaggedValue::F64Array4(_)
				| TaggedValue::VecF64(_)
				| TaggedValue::VecDVec2(_)
				| TaggedValue::DAffine2(_) => FrontendGraphDataType::Number,
				TaggedValue::Artboard(_) => FrontendGraphDataType::Artboard,
				TaggedValue::Graphic(_) => FrontendGraphDataType::Graphic,
				TaggedValue::Raster(_) => FrontendGraphDataType::Raster,
				TaggedValue::Vector(_) => FrontendGraphDataType::Vector,
				TaggedValue::Color(_) => FrontendGraphDataType::Color,
				TaggedValue::Gradient(_) | TaggedValue::GradientStops(_) | TaggedValue::GradientTable(_) => FrontendGraphDataType::Gradient,
				TaggedValue::String(_) => FrontendGraphDataType::Typography,
				_ => FrontendGraphDataType::General,
			},
			None => FrontendGraphDataType::General,
		}
	}

	pub fn compiled_nested_type(&self) -> Option<&Type> {
		match self {
			TypeSource::Compiled(compiled_type) => Some(compiled_type.nested_type()),
			TypeSource::TaggedValue(value_type) => Some(value_type.nested_type()),
			_ => None,
		}
	}

	/// If Some, the type should be displayed in the imports/exports, if None it should be replaced with "import/export index _".
	pub fn compiled_nested_type_name(&self) -> Option<String> {
		self.compiled_nested_type().map(|ty| ty.to_string())
	}

	/// Used when searching for nodes in the add Node popup.
	pub fn add_node_string(self) -> Option<String> {
		self.compiled_nested_type().map(|ty| format!("type:{}", ty.to_string()))
	}

	/// The type to display in the tooltip.
	pub fn resolved_type_name(&self) -> String {
		match self {
			TypeSource::Compiled(compiled_type) => compiled_type.nested_type().to_string(),
			TypeSource::TaggedValue(value_type) => value_type.nested_type().to_string(),
			TypeSource::DocumentNodeDefinition(_) => "Unknown".to_string(),
			TypeSource::Unknown => "Unknown".to_string(),
			TypeSource::Error(_) => "Error".to_string(),
		}
	}
}

impl NodeNetworkInterface {
	/// Get the [`TypeSource`] for any InputConnector.
	/// If the input is not compiled, then an Unknown or default from the definition is returned.
	pub fn input_type(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> TypeSource {
		let Some(input) = self.input_from_connector(input_connector, network_path) else {
			return TypeSource::Error("Could not get input from connector");
		};

		match input {
			NodeInput::Node { node_id, output_index } => {
				let input_type = self.output_type(&OutputConnector::node(*node_id, *output_index), network_path);
				if input_type == TypeSource::Unknown {
					// If we are trying to get the input type of an unknown node, check if it has a reference to its definition and use that input type
					if let InputConnector::Node { node_id, input_index } = input_connector {
						if let Some(definition) = self.get_node_definition(node_id, network_path) {
							if let Some(value) = definition.node_template.document_node.inputs.get(*input_index).and_then(|input| input.as_value()) {
								return TypeSource::DocumentNodeDefinition(value.ty());
							}
						}
					}
				}
				input_type
			}

			NodeInput::Value { tagged_value, .. } => TypeSource::TaggedValue(tagged_value.ty()),
			NodeInput::Import { import_index, .. } => {
				// Get the input type of the encapsulating node input
				let Some((encapsulating_node, encapsulating_path)) = network_path.split_last() else {
					return TypeSource::Error("Could not get type of import in document network");
				};
				self.input_type(&InputConnector::node(*encapsulating_node, *import_index), encapsulating_path)
			}
			NodeInput::Scope(_) => TypeSource::Compiled(concrete!(())),
			NodeInput::Reflection(document_node_metadata) => TypeSource::Compiled(document_node_metadata.ty()),
			NodeInput::Inline(_) => TypeSource::Compiled(concrete!(InlineRust)),
		}
	}

	// Gets the default tagged value for an input. If its not compiled, then it tries to get a valid type. If there are no valid types, then it picks a random implementation
	pub fn tagged_value_from_input(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> TaggedValue {
		let guaranteed_type = match self.input_type(input_connector, network_path) {
			TypeSource::Compiled(compiled) => compiled,
			TypeSource::TaggedValue(value) => value,
			TypeSource::DocumentNodeDefinition(definition) => definition,
			TypeSource::Unknown => {
				let mut valid_types = self.potential_valid_input_types(input_connector, network_path);

				match valid_types.pop() {
					Some(valid_type) => valid_type,
					None => {
						match self.random_type_for_connector(input_connector, network_path) {
							Some(random_type) => random_type,
							// If there are no connected protonodes then we give up and return the empty type
							None => concrete!(()),
						}
					}
				}
			}
			TypeSource::Error(e) => {
				log::error!("Error getting tagged_value_from_input for {input_connector:?} {e}");
				concrete!(())
			}
		};
		TaggedValue::from_type_or_none(&guaranteed_type)
	}

	/// A list of all valid input types for this specific node.
	pub fn potential_valid_input_types(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Vec<Type> {
		let InputConnector::Node { node_id, input_index } = input_connector else {
			// An export can have any type connected to it
			return vec![graph_craft::generic!(T)];
		};
		let Some(implementation) = self.implementation(node_id, network_path) else {
			log::error!("Could not get node implementation in valid_input_types");
			return Vec::new();
		};
		match implementation {
			DocumentNodeImplementation::Network(_) => {
				let nested_path = [network_path, &[*node_id]].concat();
				let Some(outward_wires) = self.outward_wires(&nested_path) else {
					log::error!("Could not get outward wires in valid_input_types");
					return Vec::new();
				};
				let Some(inputs_from_import) = outward_wires.get(&OutputConnector::Import(*input_index)) else {
					log::error!("Could not get inputs from import in valid_input_types");
					return Vec::new();
				};

				let intersection: HashSet<Type> = inputs_from_import
					.clone()
					.iter()
					.map(|input_connector| self.potential_valid_input_types(input_connector, &nested_path).into_iter().collect::<HashSet<_>>())
					.fold(None, |acc: Option<HashSet<Type>>, set| match acc {
						Some(acc_set) => Some(acc_set.intersection(&set).cloned().collect()),
						None => Some(set),
					})
					.unwrap_or_default();

				intersection.into_iter().collect::<Vec<_>>()
			}
			DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
				let Some(implementations) = NODE_REGISTRY.get(proto_node_identifier) else {
					log::error!("Protonode {proto_node_identifier:?} not found in registry");
					return Vec::new();
				};
				let number_of_inputs = self.number_of_inputs(node_id, network_path);
				implementations
					.iter()
					.filter_map(|(node_io, _)| {
						let valid_implementation = (0..number_of_inputs).filter(|iterator_index| iterator_index != input_index).all(|iterator_index| {
							let input_type = self.input_type(&InputConnector::node(*node_id, iterator_index), network_path);
							// TODO: Fix type checking for different call arguments
							// For example a node input of (Footprint) -> Vector would not be compatible with a node that is called with () and returns Vector
							node_io.inputs.get(iterator_index).map(|ty| ty.nested_type()) == input_type.compiled_nested_type()
						});
						if valid_implementation { node_io.inputs.get(*input_index).cloned() } else { None }
					})
					.collect::<Vec<_>>()
			}
			DocumentNodeImplementation::Extract => {
				log::error!("Input types for extract node not supported");
				Vec::new()
			}
		}
	}

	/// Performs a downstream traversal to ensure input type will work in the full context of the graph.
	pub fn complete_valid_input_types(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Result<Vec<Type>, String> {
		match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(implementation) = self.implementation(node_id, network_path) else {
					return Err(format!("Could not get node implementation for {:?} {} in valid_input_types", network_path, *node_id));
				};
				match implementation {
					DocumentNodeImplementation::Network(_) => self.valid_output_types(&OutputConnector::Import(input_connector.input_index()), &[network_path, &[*node_id]].concat()),
					DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
						let Some(implementations) = NODE_REGISTRY.get(proto_node_identifier) else {
							return Err(format!("Protonode {proto_node_identifier:?} not found in registry"));
						};
						let valid_output_types = match self.valid_output_types(&OutputConnector::node(*node_id, 0), network_path) {
							Ok(valid_types) => valid_types,
							Err(e) => return Err(e),
						};

						let valid_types = implementations
							.iter()
							.filter_map(|(node_io, _)| {
								if !valid_output_types.iter().any(|output_type| output_type.nested_type() == node_io.return_value.nested_type()) {
									return None;
								}

								let valid_inputs = (0..node_io.inputs.len()).filter(|iterator_index| iterator_index != input_index).all(|iterator_index| {
									let input_type = self.input_type(&InputConnector::node(*node_id, iterator_index), network_path);
									match input_type.compiled_nested_type() {
										Some(input_type) => node_io.inputs.get(iterator_index).is_some_and(|node_io_input_type| node_io_input_type.nested_type() == input_type),
										None => true,
									}
								});
								if valid_inputs { node_io.inputs.get(*input_index).cloned() } else { None }
							})
							.collect::<Vec<_>>();
						Ok(valid_types)
					}
					DocumentNodeImplementation::Extract => {
						log::error!("Input types for extract node not supported");
						Ok(Vec::new())
					}
				}
			}
			InputConnector::Export(export_index) => {
				match network_path.split_last() {
					Some((encapsulating_node, encapsulating_path)) => self.valid_output_types(&OutputConnector::node(*encapsulating_node, *export_index), encapsulating_path),
					None => {
						// Valid types for the export are all types that can be fed into the render node
						// TODO: Use ::IDENTIFIER
						let render_node = "graphene_std::wasm_application_io::RenderNode";
						let Some(implementations) = NODE_REGISTRY.get(&ProtoNodeIdentifier::new(render_node)) else {
							return Err(format!("Protonode {render_node:?} not found in registry"));
						};
						Ok(implementations.iter().map(|(types, _)| types.inputs[1].clone()).collect())
					}
				}
			}
		}
	}

	pub fn output_type(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> TypeSource {
		match output_connector {
			OutputConnector::Node { node_id, output_index } => {
				// First try iterating upstream to the first protonode and try get its compiled type
				let Some(implementation) = self.implementation(node_id, network_path) else {
					return TypeSource::Error("Could not get implementation");
				};
				match implementation {
					DocumentNodeImplementation::Network(_) => self.input_type(&InputConnector::Export(*output_index), &[network_path, &[*node_id]].concat()),
					DocumentNodeImplementation::ProtoNode(_) => match self.resolved_types.types.get(&[network_path, &[*node_id]].concat()) {
						Some(resolved_type) => TypeSource::Compiled(resolved_type.output.clone()),
						None => TypeSource::Unknown,
					},
					DocumentNodeImplementation::Extract => TypeSource::Compiled(concrete!(())),
				}
			}
			OutputConnector::Import(import_index) => {
				let Some((encapsulating_node, encapsulating_path)) = network_path.split_last() else {
					return TypeSource::Error("Cannot get import type in document network");
				};
				self.input_type(&InputConnector::node(*encapsulating_node, *import_index), encapsulating_path)
			}
		}
	}
	// The valid output types are all types that are valid for each downstream connection
	pub fn valid_output_types(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Result<Vec<Type>, String> {
		let Some(outward_wires) = self.outward_wires(&network_path) else {
			return Err("Could not get outward wires in valid_input_types".to_string());
		};
		let Some(inputs_from_import) = outward_wires.get(output_connector) else {
			return Err("Could not get inputs from import in valid_input_types".to_string());
		};

		let intersection = inputs_from_import
			.clone()
			.iter()
			.map(|input_connector| self.potential_valid_input_types(input_connector, &network_path).into_iter().collect::<HashSet<_>>())
			.fold(None, |acc: Option<HashSet<Type>>, set| match acc {
				Some(acc_set) => Some(acc_set.intersection(&set).cloned().collect()),
				None => Some(set),
			})
			.unwrap_or_default();

		Ok(intersection.into_iter().collect::<Vec<_>>())
	}

	/// Performs a downstream iteration from an input connector to the next protonode and selects a random type from its implementations.
	pub fn random_type_for_connector(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<Type> {
		match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(implementation) = self.implementation(node_id, network_path) else {
					log::error!("Could not get node {node_id} in random_downstream_protonode_from_connector");
					return None;
				};
				match implementation {
					DocumentNodeImplementation::Network(_) => {
						let Some(outward_wires) = self.outward_wires(&network_path) else {
							log::error!("Could not get outward wires in random_downstream_protonode_from_connector");
							return None;
						};
						let Some(inputs_from_import) = outward_wires.get(&OutputConnector::Import(*input_index)) else {
							log::error!("Could not get inputs from import in valid_input_types");
							return None;
						};
						let Some(first_input) = inputs_from_import.first().cloned() else {
							return None;
						};
						self.random_type_for_connector(&first_input, &[network_path, &[*node_id]].concat())
					}
					DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
						let Some(implementations) = NODE_REGISTRY.get(proto_node_identifier) else {
							log::error!("Protonode {proto_node_identifier:?} not found in registry");
							return None;
						};
						implementations.keys().min().and_then(|node_io| node_io.inputs.get(input_connector.input_index())).cloned()
					}
					DocumentNodeImplementation::Extract => None,
				}
			}
			InputConnector::Export(export_index) => network_path.split_last().and_then(|(encapsulating_node, encapsulating_path)| {
				let Some(outward_wires) = self.outward_wires(&encapsulating_path) else {
					log::error!("Could not get outward wires in random_downstream_protonode_from_connector export");
					return None;
				};
				let Some(inputs_from_import) = outward_wires.get(&OutputConnector::node(*encapsulating_node, *export_index)) else {
					log::error!("Could not get inputs from import in valid_input_types");
					return None;
				};
				let Some(first_input) = inputs_from_import.first().cloned() else {
					return None;
				};
				self.random_type_for_connector(&first_input, encapsulating_path)
			}),
		}
	}
}
