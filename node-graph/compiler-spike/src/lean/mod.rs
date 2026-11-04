//! An exploratory from-scratch compiler aiming at the performance target, producing the same graph as the memoized baseline.
//!
//! Load time builds the shared tables: interned types and identifiers, the registry, and the preprocessor's substitutions as
//! shared template definitions. A document is a table of definitions referencing nodes by index.
//!
//! Compilation is a set of passes with their own state, fused per node by elaboration's post-order walk:
//! - `elaborate`: instances, imports, scopes, reflection, passthroughs, hidden nodes, substitutions.
//! - `context`: context feature analysis and nullification insertion.
//! - `typing`: implementation selection.
//! - `checks`: constant inputs and footprint invariance.
//!
//! Every pass stores its per-node results in a table indexed by the IR node ID, and the IR store hash-conses nodes as they are built.

pub mod checks;
pub mod codegen;
pub mod coercion;
pub mod context;
pub mod document;
pub mod elaborate;
pub mod executor;
pub mod ir;
pub mod registry;
pub mod types;
pub mod typing;
pub mod varargs;

use crate::baseline::{CONTEXT_MODIFICATION, MEMOIZE, PASSTHROUGH, VALUE_NODE};
use crate::workload::{self, CompileMetadata};
use checks::Checks;
use context::ContextAnalysis;
use document::{Converter, Definition, DefinitionId, IdentifierId, Strings, Substitution};
use elaborate::{Elaboration, Passes, Tables};
use ir::Ir;
use registry::Registry;
use rustc_hash::FxHashMap;
use types::{TypeId, TypeInterner};
use typing::Typing;

#[derive(Clone, Copy)]
pub struct Builtins {
	pub context: TypeId,
	pub unit: TypeId,
	pub node_id_path: TypeId,
	pub source_id: TypeId,
	pub context_modification: TypeId,
	pub none_hash: u64,
	pub passthrough: IdentifierId,
	pub value_node: IdentifierId,
	pub memoize: IdentifierId,
	pub context_modification_node: IdentifierId,
	pub inject_vararg: IdentifierId,
	pub read_vararg: IdentifierId,
}

pub struct Document {
	pub definitions: Vec<Definition>,
	pub root: DefinitionId,
	/// Identifies this document's structure, so incremental compiles know when the root's cached slots still apply.
	pub id: u64,
	/// Nodes edited since the last compile as (definition, node), including the node implementing each edited definition up
	/// to the root, whose results and dependents must be recomputed.
	pub dirty_nodes: Vec<(u32, u32)>,
}

impl Document {
	/// Replaces a value input, as dragging a slider does, and rehashes the edited definition and its ancestors.
	pub fn set_value(&mut self, definition: u32, local: u32, input: usize, hash: u64) {
		let edited = &mut self.definitions[definition as usize];
		let index = edited.nodes[local as usize].inputs.start as usize + input;
		let document::Input::Value(value) = &mut edited.inputs[index] else {
			panic!("The edited input should be a value")
		};
		value.hash = hash;
		self.rehash(definition, local);
	}

	fn rehash(&mut self, definition: u32, local: u32) {
		let mut dirty = rustc_hash::FxHashSet::default();
		let mut pending = vec![(definition, local)];
		while let Some(node) = pending.pop() {
			if dirty.insert(node) {
				pending.extend(self.definitions[node.0 as usize].owners.iter().copied());
			}
		}

		// Definitions follow the ones they name, so ascending order rehashes every nested definition before its owners
		let mut affected: Vec<u32> = dirty.iter().map(|(definition, _)| *definition).collect();
		affected.sort_unstable();
		affected.dedup();
		for definition in affected {
			let definitions = &self.definitions;
			let deep_hash = definitions[definition as usize].compute_deep_hash(|nested| definitions[nested.index()].deep_hash);
			self.definitions[definition as usize].deep_hash = deep_hash;
		}
		self.dirty_nodes.extend(dirty);
	}
}

/// The result of a compile: the IR store with its pass results, and the node computing the document's output.
pub struct Compiled<'a> {
	pub ir: &'a Ir,
	pub typing: &'a Typing,
	pub context: &'a ContextAnalysis,
	pub checks: &'a Checks,
	pub output: u32,
}

pub struct Lean {
	pub strings: Strings,
	pub templates: Vec<Definition>,
	/// Indexed by identifier ID, which is dense since identifiers are interned.
	pub substitutions: Vec<Option<Substitution>>,
	pub compile_metadata: Vec<Option<CompileMetadata>>,
	/// The preprocessor's inject scope templates, added to each document's root.
	inject_scopes: Vec<(String, workload::Node)>,
	pub builtins: Builtins,

	pub ir: Ir,
	pub typing: Typing,
	pub checks: Checks,
	pub context: ContextAnalysis,
	elaboration: Elaboration,
}

/// Where inputs get converted to the types their connectors accept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAdapters {
	/// The preprocessor's `input_adapter<T>` wrapper on every input of a substituted node, as Graphite does today.
	Preprocessed,
	/// Typing inserts conversion nodes only on the inputs that need them, selecting among all of the node's implementations.
	Resolved,
}

fn strip_input_adapters(network: &mut workload::Network) {
	for (_, node) in &mut network.nodes {
		match &mut node.implementation {
			workload::Implementation::Proto(identifier) if identifier.starts_with("input_adapter<") => *identifier = PASSTHROUGH.to_string(),
			workload::Implementation::Network(nested) => strip_input_adapters(nested),
			_ => {}
		}
	}
}

impl Lean {
	pub fn load() -> Self {
		Self::load_with(InputAdapters::Resolved)
	}

	pub fn load_with(input_adapters: InputAdapters) -> Self {
		let mut types = TypeInterner::default();
		let mut strings = Strings::default();

		let item = |types: &mut TypeInterner, name: &str| {
			let element = types.concrete(name);
			types.intern(types::TypeData::Item(element))
		};
		let builtins = Builtins {
			context: types.concrete(crate::baseline::types::CONTEXT_NAME),
			unit: types.concrete(crate::baseline::types::UNIT_NAME),
			node_id_path: item(&mut types, "core_types::list::NodeIdPath"),
			source_id: item(&mut types, "core_types::runtime::SourceId"),
			context_modification: item(&mut types, "core_types::context::ContextModification"),
			none_hash: 0x4E4F_4E45,
			passthrough: IdentifierId(strings.intern(PASSTHROUGH)),
			value_node: IdentifierId(strings.intern(VALUE_NODE)),
			memoize: IdentifierId(strings.intern(MEMOIZE)),
			context_modification_node: IdentifierId(strings.intern(CONTEXT_MODIFICATION)),
			inject_vararg: IdentifierId(strings.intern(varargs::INJECT_VARARG)),
			read_vararg: IdentifierId(strings.intern(varargs::READ_VARARG)),
		};

		let mut registry = Registry::default();
		for entry in workload::load_registry() {
			let identifier = IdentifierId(strings.intern(&entry.identifier));
			for signature in &entry.implementations {
				let call_argument = types.convert(&signature.call_argument);
				let inputs = signature.inputs.iter().map(|input| types.convert(input)).collect();
				let output = types.convert(&signature.output);
				registry.add(&types, identifier, call_argument, inputs, output);
			}
		}

		registry.index_promotion_adapters(&types, &strings);
		coercion::load(&mut types, &mut strings, &mut registry, builtins.context);
		varargs::load(&mut types, &mut registry, builtins.inject_vararg, builtins.context);

		let tables = workload::load_preprocessor_tables();
		let mut templates = Vec::new();
		let mut substitutions = Vec::new();
		for substitution in &tables.substitutions {
			let workload::Implementation::Network(network) = &substitution.node.implementation else {
				continue;
			};
			let stripped;
			let network = match input_adapters {
				InputAdapters::Preprocessed => network,
				InputAdapters::Resolved => {
					let mut copy = network.clone();
					strip_input_adapters(&mut copy);
					stripped = copy;
					&stripped
				}
			};
			let mut converter = Converter {
				types: &mut types,
				strings: &mut strings,
				definitions: &mut templates,
				template: true,
				in_document: false,
				entries: Default::default(),
			};
			let definition = converter.network(network);
			let empty = FxHashMap::default();
			let defaults = substitution.node.inputs.iter().map(|input| converter.input(input, &empty)).collect();

			let wrapped_local = network.nodes.iter().position(|(id, _)| *id == substitution.wrapped_node_id).expect("the wrapped node should exist") as u32;
			let identifier = strings.intern(&substitution.identifier) as usize;
			if substitutions.len() <= identifier {
				substitutions.resize_with(identifier + 1, || None);
			}
			substitutions[identifier] = Some(Substitution { definition, wrapped_local, defaults });
		}

		let mut compile_metadata = Vec::new();
		for entry in &tables.compile_metadata {
			let identifier = strings.intern(&entry.identifier) as usize;
			if compile_metadata.len() <= identifier {
				compile_metadata.resize(identifier + 1, None);
			}
			compile_metadata[identifier] = Some(entry.metadata);
		}
		let inject_scopes = tables.inject_scopes.into_iter().map(|scope| (scope.identifier, scope.template)).collect();

		Self {
			strings,
			templates,
			substitutions,
			compile_metadata,
			inject_scopes,
			builtins,
			ir: Ir::default(),
			typing: Typing::new(types, registry, builtins.context),
			checks: Checks::default(),
			context: ContextAnalysis::new(builtins),
			elaboration: Elaboration::default(),
		}
	}

	/// Converts a scoped document, adding the inject scope templates to its root like `Preprocessor::insert_inject_scopes`.
	pub fn convert_document(&mut self, network: &workload::Network) -> Document {
		use std::hash::{Hash, Hasher};
		let producer_id = |identifier: &str| {
			let mut hasher = std::collections::hash_map::DefaultHasher::new();
			identifier.hash(&mut hasher);
			hasher.finish()
		};
		let extra_nodes: Vec<(u64, workload::Node)> = self.inject_scopes.iter().map(|(identifier, template)| (producer_id(identifier), template.clone())).collect();
		let extra_scopes: Vec<(String, u64)> = self.inject_scopes.iter().map(|(identifier, _)| (identifier.clone(), producer_id(identifier))).collect();

		let mut definitions = Vec::new();
		let mut converter = Converter {
			types: &mut self.typing.types,
			strings: &mut self.strings,
			definitions: &mut definitions,
			template: false,
			in_document: true,
			entries: Default::default(),
		};
		let root = converter.network_with(network, &extra_nodes, &extra_scopes);
		self.annotate_export_counts(&mut definitions);
		static NEXT_DOCUMENT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
		Document {
			definitions,
			root,
			id: NEXT_DOCUMENT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
			dirty_nodes: Vec::new(),
		}
	}

	/// Records how many exports each node's instance has: networks and substituted nodes lower to instances.
	fn annotate_export_counts(&self, definitions: &mut [Definition]) {
		for index in 0..definitions.len() {
			let definition = &definitions[index];
			let counts = definition
				.nodes
				.iter()
				.map(|node| match node.implementation {
					document::Implementation::Network(network) if node.visible => definitions[network.index()].exports.len() as u32,
					document::Implementation::Proto(identifier) if node.visible && definition.in_document => match self.substitutions.get(identifier.0 as usize) {
						Some(Some(substitution)) => self.templates[substitution.definition.index()].exports.len() as u32,
						_ => document::SINGLE_OUTPUT,
					},
					_ => document::SINGLE_OUTPUT,
				})
				.collect();
			definitions[index].export_counts = counts;
		}
	}

	/// Compiles the document from scratch, keeping only the pass buffers' capacity from earlier compiles.
	pub fn compile(&mut self, document: &Document) -> Result<Compiled<'_>, String> {
		self.ir.clear();
		self.typing.clear();
		self.checks.clear();
		self.context.clear();
		self.elaboration.clear_instances();
		self.elaboration.memoize_instances = false;
		self.run(document, &[])
	}

	/// Compiles the document reusing everything earlier incremental compiles built: hash-consed nodes with their pass
	/// results, the root nodes untouched by edits since the last compile, and cached instances whose definition content,
	/// arguments, path, and outer scope bindings are unchanged. Consumes the document's record of edits.
	/// Nodes no longer reachable stay in the store; `Compiled::output` reaches only the current graph.
	pub fn compile_incremental(&mut self, document: &mut Document) -> Result<Compiled<'_>, String> {
		self.elaboration.memoize_instances = true;
		let dirty = std::mem::take(&mut document.dirty_nodes);
		self.run(document, &dirty)
	}

	pub fn instance_statistics(&self) -> elaborate::Statistics {
		self.elaboration.statistics
	}

	fn run(&mut self, document: &Document, dirty: &[(u32, u32)]) -> Result<Compiled<'_>, String> {
		let tables = Tables {
			templates: &self.templates,
			substitutions: &self.substitutions,
			compile_metadata: &self.compile_metadata,
			builtins: self.builtins,
		};
		let passes = Passes {
			ir: &mut self.ir,
			typing: &mut self.typing,
			checks: &mut self.checks,
			context: &mut self.context,
			strings: &mut self.strings,
		};
		let output = self.elaboration.run(tables, document, dirty, passes)?;

		Ok(Compiled {
			ir: &self.ir,
			typing: &self.typing,
			context: &self.context,
			checks: &self.checks,
			output,
		})
	}

	pub fn identifier_name(&self, identifier: IdentifierId) -> &str {
		self.strings.name(identifier.0)
	}
}
