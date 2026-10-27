//! A faithful port of Graphite's current compilation pipeline: preprocessing, `Compiler::compile`, and `DynamicExecutor::new`.

pub mod document;
pub mod executor;
pub mod preprocess;
pub mod proto;
pub mod types;
pub mod typing;

use crate::workload;
use document::{DocumentNode, DocumentNodeImplementation, MemoValue, NodeInput, NodeNetwork, OriginalLocation, ValueData, ValueKind};
use executor::DynamicExecutor;
use preprocess::{Preprocessor, Substitution};
use proto::ProtoNetwork;
use std::borrow::Cow;
use std::collections::BTreeSet;
use types::{NodeIOTypes, Type, TypeNames};
use typing::Registry;

pub const PASSTHROUGH: &str = "graphene_core::ops::PassthroughNode";
pub const VALUE_NODE: &str = "core_types::value::ClonedNode";
pub const MEMOIZE: &str = "graphene_core::memo::MemoizeNode";
pub const CONTEXT_MODIFICATION: &str = "graphene_core::context_modification::ContextModificationNode";
pub const FOOTPRINT: u32 = 1 << 0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u64);

pub type CompileMetadata = workload::CompileMetadata;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ContextModification {
	pub features: u32,
	pub sources: BTreeSet<u64>,
}

/// Types of the values the compiler synthesizes itself.
pub struct Builtins {
	pub node_id_path: Type,
	pub source_id: Type,
	pub context_modification: Type,
}

pub struct Baseline {
	pub registry: Registry,
	pub preprocessor: Preprocessor,
	pub builtins: Builtins,
	/// Analyzes each node's context dependencies once rather than once per path reaching it. Off reproduces Graphite.
	pub memoize_context_analysis: bool,
	names: TypeNames,
}

impl Baseline {
	pub fn load() -> Self {
		let mut names = TypeNames::default();
		let mut item = |name: &str| Type::Item(Box::new(names.convert(&workload::Type::Concrete(name.to_string()))));
		let builtins = Builtins {
			node_id_path: item("core_types::list::NodeIdPath"),
			source_id: item("core_types::runtime::SourceId"),
			context_modification: item("core_types::context::ContextModification"),
		};

		let registry = workload::load_registry()
			.into_iter()
			.map(|entry| {
				let implementations = entry
					.implementations
					.iter()
					.map(|signature| {
						let node_io = NodeIOTypes::new(
							names.convert(&signature.call_argument),
							names.convert(&signature.output),
							signature.inputs.iter().map(|input| names.convert(input)).collect(),
						);
						(node_io, executor::construct as executor::NodeConstructor)
					})
					.collect();
				(Cow::Borrowed(names.leak(&entry.identifier)), implementations)
			})
			.collect();

		let tables = workload::load_preprocessor_tables();
		let mut converter = Converter { names: &mut names, from_code: true };
		let substitutions = tables
			.substitutions
			.iter()
			.map(|substitution| {
				let node = converter.node(&substitution.node);
				(
					converter.identifier(&substitution.identifier),
					Substitution {
						node,
						wrapped_node_id: NodeId(substitution.wrapped_node_id),
					},
				)
			})
			.collect();
		let inject_scopes = tables
			.inject_scopes
			.iter()
			.map(|scope| (converter.identifier(&scope.identifier), (converter.node(&scope.template), converter.ty(&scope.ty))))
			.collect();
		let compile_metadata = tables.compile_metadata.iter().map(|entry| (converter.identifier(&entry.identifier), entry.metadata)).collect();

		Self {
			registry,
			preprocessor: Preprocessor {
				substitutions,
				inject_scopes,
				compile_metadata,
			},
			builtins,
			memoize_context_analysis: false,
			names,
		}
	}

	/// The demo's scoped network as Graphite deserializes it from the document.
	pub fn load_network(&mut self, name: &str) -> NodeNetwork {
		self.convert_network(&workload::load_network(name))
	}

	pub fn convert_network(&mut self, network: &workload::Network) -> NodeNetwork {
		Converter {
			names: &mut self.names,
			from_code: false,
		}
		.network(network)
	}

	pub fn preprocess(&self, mut network: NodeNetwork) -> NodeNetwork {
		self.preprocessor.preprocess(&mut network);
		network
	}

	pub fn compile(&self, mut network: NodeNetwork) -> Result<ProtoNetwork, String> {
		network.resolve_scope_inputs();
		network.generate_node_paths(&[]);
		let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();
		network.populate_dependants();
		for id in node_ids {
			network.flatten(id, &self.builtins);
		}
		network.remove_all_passthrough_nodes();

		let mut proto_network = network.into_proto_network();
		proto_network.insert_context_nullification_nodes(&self.builtins, self.memoize_context_analysis)?;
		if proto_network.non_constant_inputs() != 0 {
			return Err("An input requiring a constant is not fed by a value".to_string());
		}
		std::hint::black_box(proto_network.footprint_invariance_violations());
		proto_network.generate_stable_node_ids();
		Ok(proto_network)
	}

	pub fn build_executor(&self, proto_network: ProtoNetwork) -> Result<DynamicExecutor<'_>, String> {
		DynamicExecutor::new(proto_network, &self.registry)
	}

	pub fn run(&self, network: NodeNetwork) -> Result<DynamicExecutor<'_>, String> {
		self.build_executor(self.compile(self.preprocess(network))?)
	}
}

/// Converts the exported workload schema into the baseline's runtime structures.
struct Converter<'a> {
	names: &'a mut TypeNames,
	/// Whether the structures were built in Graphite's code (borrowed names, `TypeId`s) rather than deserialized from a document.
	from_code: bool,
}

impl Converter<'_> {
	fn identifier(&mut self, identifier: &str) -> Cow<'static, str> {
		match self.from_code {
			true => Cow::Borrowed(self.names.leak(identifier)),
			false => Cow::Owned(identifier.to_string()),
		}
	}

	fn ty(&mut self, ty: &workload::Type) -> Type {
		match self.from_code {
			true => self.names.convert(ty),
			false => TypeNames::convert_deserialized(ty),
		}
	}

	fn input(&mut self, input: &workload::Input) -> NodeInput {
		match input {
			workload::Input::Node(reference) => NodeInput::node(NodeId(reference.node), reference.output),
			workload::Input::Value(value) => {
				let data = ValueData {
					ty: self.names.convert(&value.ty),
					kind: ValueKind::Document { size: value.size },
				};
				NodeInput::Value {
					value: MemoValue::with_hash(data, value.hash),
					exposed: value.exposed,
				}
			}
			workload::Input::Import(import) => NodeInput::Import {
				import_type: self.ty(&import.ty),
				import_index: import.index,
			},
			workload::Input::Scope(key) => NodeInput::Scope(self.identifier(key)),
			workload::Input::Reflection(reflection) => NodeInput::Reflection(*reflection),
			workload::Input::Inline => NodeInput::Inline,
		}
	}

	fn node(&mut self, node: &workload::Node) -> DocumentNode {
		let implementation = match &node.implementation {
			workload::Implementation::Network(network) => DocumentNodeImplementation::Network(self.network(network)),
			workload::Implementation::Proto(identifier) => DocumentNodeImplementation::ProtoNode(self.identifier(identifier)),
			workload::Implementation::Extract => DocumentNodeImplementation::Extract,
		};
		DocumentNode {
			inputs: node.inputs.iter().map(|input| self.input(input)).collect(),
			call_argument: self.ty(&node.call_argument),
			implementation,
			visible: node.visible,
			skip_deduplication: node.skip_deduplication,
			compile_metadata: node.compile_metadata,
			original_location: OriginalLocation::default(),
		}
	}

	fn network(&mut self, network: &workload::Network) -> NodeNetwork {
		NodeNetwork {
			exports: network.exports.iter().map(|input| self.input(input)).collect(),
			nodes: network.nodes.iter().map(|(id, node)| (NodeId(*id), self.node(node))).collect(),
			scope_injections: network.scope_injections.iter().map(|(key, id, ty)| (key.clone(), (NodeId(*id), self.ty(ty)))).collect(),
			generated: network.generated,
		}
	}
}
