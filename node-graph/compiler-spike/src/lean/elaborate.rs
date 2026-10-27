//! The elaboration pass: walks definition instances post-order, resolving imports, scopes, reflection, passthroughs,
//! hidden nodes, and substitutions, and hands each resulting node to the node-local passes.
//!
//! With instance memoization, each instance's exports are cached under its definition's deep hash, its arguments, its
//! path, and its metadata override, together with the outer scope bindings it read, so unchanged instances are skipped.

use super::checks::Checks;
use super::context::{Builder, ContextAnalysis};
use super::document::{Definition, DefinitionId, IdentifierId, Implementation, Input, Node, Substitution};
use super::ir::{Ir, IrValue, NewNode};
use super::typing::Typing;
use super::{Builtins, Document};
use crate::workload::{CompileMetadata, Reflection};
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::{Hash, Hasher};

const UNLOWERED: u32 = u32::MAX;
const ROOT_PATH: u32 = 0;
/// Marks a slot pointing into its instance's slot cache's export lists rather than this compile's export stack.
const PERSISTED: u32 = 1 << 31;

/// A document instance's slots from its previous lowering, reused when its outer scope bindings are unchanged: the edited
/// nodes, the nodes reading changed arguments, and their dependents are recomputed, and the rest keep their results.
struct SlotCache {
	arguments: Box<[u32]>,
	scope_reads: Box<[(u32, u32)]>,
	slots: Vec<u32>,
	exports: Vec<u32>,
	/// For each node, the nodes of the same definition reading it through an input or a scope binding.
	dependents: Vec<Vec<u32>>,
	/// For each import, the nodes reading it directly.
	import_readers: Vec<Vec<u32>>,
}

impl SlotCache {
	fn new(definition: &Definition) -> Self {
		let mut dependents = vec![Vec::new(); definition.nodes.len()];
		let mut import_readers: Vec<Vec<u32>> = Vec::new();
		for (local, node) in definition.nodes.iter().enumerate() {
			for input in definition.node_inputs(node) {
				let dependency = match *input {
					Input::Node { local, .. } => Some(local),
					Input::Scope(key) => definition.scopes.iter().find(|(scope_key, _)| *scope_key == key).map(|(_, provider)| *provider),
					Input::Import(index) => {
						if import_readers.len() <= index as usize {
							import_readers.resize(index as usize + 1, Vec::new());
						}
						import_readers[index as usize].push(local as u32);
						None
					}
					_ => None,
				};
				if let Some(dependency) = dependency {
					dependents[dependency as usize].push(local as u32);
				}
			}
		}
		Self {
			arguments: Box::default(),
			scope_reads: Box::default(),
			slots: Vec::new(),
			exports: Vec::new(),
			dependents,
			import_readers,
		}
	}
}

/// The node-local passes, run in order on every node elaboration produces.
pub struct Passes<'a> {
	pub ir: &'a mut Ir,
	pub typing: &'a mut Typing,
	pub checks: &'a mut Checks,
	pub context: &'a mut ContextAnalysis,
	pub value_node: IdentifierId,
	pub context_type: super::types::TypeId,
}

impl Passes<'_> {
	fn builder(&mut self) -> (Builder<'_>, &mut ContextAnalysis) {
		let Passes { ir, typing, checks, context, .. } = self;
		(Builder { ir, typing, checks }, context)
	}

	/// Builds a node through the passes, inserting the conversion nodes typing asks for on its inputs first.
	fn build(&mut self, node: NewNode, inputs: &mut [u32]) -> Result<u32, String> {
		let (mut builder, context) = self.builder();
		let conversions = match context.build(&mut builder, node, inputs)? {
			Ok(id) => return Ok(id),
			Err(conversions) => conversions,
		};

		for conversion in conversions.iter() {
			let mut conversion_inputs = vec![inputs[conversion.input as usize]];
			if let Some(converter) = conversion.node.converter {
				// Stands in for the scope-provided converter, like the editor's `WgpuExecutor`
				let converter_wire = self.typing.types.intern(super::types::TypeData::Item(converter));
				let converter_value = NewNode {
					identifier: self.value_node,
					call_argument: self.context_type,
					value: Some(super::ir::IrValue {
						ty: converter_wire,
						hash: converter.0 as u64,
						source: None,
					}),
					skip_deduplication: None,
					compile_metadata: CompileMetadata::default(),
					origin: node.origin,
				};
				conversion_inputs.push(self.build_value(converter_value)?);
			}
			let conversion_node = NewNode {
				identifier: conversion.node.identifier,
				call_argument: self.context_type,
				value: None,
				skip_deduplication: None,
				compile_metadata: conversion.node.compile_metadata(),
				origin: node.origin,
			};
			inputs[conversion.input as usize] = self.build(conversion_node, &mut conversion_inputs)?;
		}

		let (mut builder, context) = self.builder();
		context.build(&mut builder, node, inputs)?.map_err(|_| "Converted inputs still need conversions".to_string())
	}

	fn build_value(&mut self, node: NewNode) -> Result<u32, String> {
		let (mut builder, context) = self.builder();
		context.build_value(&mut builder, node)
	}
}

/// The registry-level tables elaboration reads.
#[derive(Clone, Copy)]
pub struct Tables<'a> {
	pub templates: &'a [Definition],
	pub substitutions: &'a [Option<Substitution>],
	pub compile_metadata: &'a [Option<CompileMetadata>],
	pub builtins: Builtins,
}

struct Frame<'a> {
	definition: DefinitionId,
	slots: usize,
	arguments: (usize, usize),
	path: u32,
	depth: u32,
	parent: Option<&'a Frame<'a>>,
	/// Compile metadata the preprocessor moves onto a substitution's wrapped node.
	wrapped_metadata: Option<(u32, CompileMetadata)>,
	slot_cache: Option<u32>,
}

enum Lowered {
	Single(u32),
	Exports(u32),
}

struct InstanceEntry {
	deep_hash: u64,
	path_hash: u64,
	wrapped_metadata: Option<(u32, CompileMetadata)>,
	arguments: Box<[u32]>,
	exports: Box<[u32]>,
	/// Scope bindings resolved outside the instance while lowering it, as (key, IR node).
	scope_reads: Box<[(u32, u32)]>,
}

/// An instance being lowered, collecting the outer scope bindings it reads.
struct ActiveInstance {
	depth: u32,
	scope_reads: Vec<(u32, u32)>,
}

/// Elaboration's stacks, kept between compiles to reuse their capacity, and the instance cache.
#[derive(Default)]
pub struct Elaboration {
	slots: Vec<u32>,
	exports: Vec<u32>,
	arguments: Vec<u32>,
	/// Parent and hash of each path.
	paths: Vec<(u32, u64)>,
	scratch: Vec<u32>,
	pub memoize_instances: bool,
	instances: FxHashMap<u64, Vec<InstanceEntry>>,
	active: Vec<ActiveInstance>,
	slot_caches: Vec<SlotCache>,
	/// Slot caches by (document definition, path hash), valid for `slot_cache_document`.
	slot_cache_index: FxHashMap<(u32, u64), u32>,
	slot_cache_document: u64,
	/// This compile's edited nodes by definition.
	dirty: FxHashMap<u32, Vec<u32>>,
	pub statistics: Statistics,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Statistics {
	pub instance_hits: usize,
	pub instance_misses: usize,
	/// Nodes of reused slot caches recomputed because an edit reached them.
	pub nodes_invalidated: usize,
	pub slot_caches_reused: usize,
}

struct Walk<'a, 'p> {
	tables: Tables<'a>,
	document: &'a Document,
	state: &'a mut Elaboration,
	passes: Passes<'p>,
}

impl Elaboration {
	pub fn clear_instances(&mut self) {
		self.instances.clear();
		self.slot_caches.clear();
		self.slot_cache_index.clear();
	}

	/// Elaborates the document's root export and returns the IR node it lowers to.
	/// When memoizing, instances containing `dirty` edits recompute those nodes with their dependents and reuse the rest.
	pub fn run(&mut self, tables: Tables, document: &Document, dirty: &[(u32, u32)], passes: Passes) -> Result<u32, String> {
		self.slots.clear();
		self.exports.clear();
		self.arguments.clear();
		self.paths.clear();
		self.paths.push((ROOT_PATH, 0));
		self.scratch.clear();
		self.active.clear();
		self.statistics = Statistics::default();

		if self.memoize_instances {
			if self.slot_cache_document != document.id {
				self.slot_caches.clear();
				self.slot_cache_index.clear();
				self.slot_cache_document = document.id;
			}
			self.dirty.clear();
			for &(definition, local) in dirty {
				self.dirty.entry(definition).or_default().push(local);
			}
		}

		let mut walk = Walk {
			tables,
			document,
			state: self,
			passes,
		};
		let definition = walk.definition(document.root);
		let slot_cache = walk.prepare_slots(document.root, ROOT_PATH, (0, 0), None)?;
		let root = Frame {
			definition: document.root,
			slots: 0,
			arguments: (0, 0),
			path: ROOT_PATH,
			depth: 0,
			parent: None,
			wrapped_metadata: None,
			slot_cache,
		};

		let [export] = definition.exports.as_slice() else {
			return Err("Only networks with a single export are supported".to_string());
		};
		let output = walk.lower_input(&root, export, ROOT_PATH)?;
		walk.persist_slots(&root, Box::default());
		Ok(output)
	}
}

impl<'a> Walk<'a, '_> {
	fn definition(&self, id: DefinitionId) -> &'a Definition {
		match id {
			DefinitionId::Template(index) => &self.tables.templates[index as usize],
			DefinitionId::Document(index) => &self.document.definitions[index as usize],
		}
	}

	fn substitution(&self, identifier: IdentifierId) -> Option<&'a Substitution> {
		self.tables.substitutions.get(identifier.0 as usize).and_then(Option::as_ref)
	}

	fn registered_metadata(&self, identifier: IdentifierId) -> Option<CompileMetadata> {
		self.tables.compile_metadata.get(identifier.0 as usize).copied().flatten()
	}

	fn path(&mut self, parent: u32, id: u64) -> u32 {
		let mut hasher = FxHasher::default();
		self.state.paths[parent as usize].1.hash(&mut hasher);
		id.hash(&mut hasher);
		self.state.paths.push((parent, hasher.finish()));
		(self.state.paths.len() - 1) as u32
	}

	fn path_hash(&self, path: u32) -> u64 {
		self.state.paths[path as usize].1
	}

	fn origin(&self, path: u32, id: u64) -> (u64, u64) {
		(self.path_hash(path), id)
	}

	fn node_path(&mut self, frame: &Frame, id: u64) -> u32 {
		match self.definition(frame.definition).generated {
			true => frame.path,
			false => self.path(frame.path, id),
		}
	}

	fn is_substituted(&self, definition: &Definition, node: &Node) -> bool {
		node.visible && definition.in_document && matches!(node.implementation, Implementation::Proto(identifier) if self.substitution(identifier).is_some())
	}

	/// How many exports a node lowering to an instance has, or `None` for a node lowering to a single IR node.
	/// Document definitions carry these counts precomputed; templates contain no instances.
	fn export_count(&self, definition: &Definition, local: usize, node: &Node) -> Option<usize> {
		if let Some(&count) = definition.export_counts.get(local) {
			return (count != super::document::SINGLE_OUTPUT).then_some(count as usize);
		}
		match node.implementation {
			Implementation::Network(network) if node.visible => Some(self.definition(network).exports.len()),
			Implementation::Proto(identifier) if self.is_substituted(definition, node) => {
				let substitution = self.substitution(identifier).expect("checked by `is_substituted`");
				Some(self.definition(substitution.definition).exports.len())
			}
			_ => None,
		}
	}

	/// Allocates an instance's slots on the slot stack, starting from its slot cache when memoizing a document instance whose
	/// arguments and outer scope bindings are unchanged, with its edited nodes and their dependents cleared.
	fn prepare_slots(&mut self, definition_id: DefinitionId, path: u32, arguments: (usize, usize), parent: Option<&Frame>) -> Result<Option<u32>, String> {
		let definition = self.definition(definition_id);
		let base = self.state.slots.len();
		self.state.slots.resize(base + definition.nodes.len(), UNLOWERED);

		let DefinitionId::Document(index) = definition_id else { return Ok(None) };
		if !self.state.memoize_instances {
			return Ok(None);
		}

		let key = (index, self.path_hash(path));
		let cache_index = match self.state.slot_cache_index.get(&key) {
			Some(&cache_index) => cache_index,
			None => {
				self.state.slot_caches.push(SlotCache::new(definition));
				let cache_index = (self.state.slot_caches.len() - 1) as u32;
				self.state.slot_cache_index.insert(key, cache_index);
				cache_index
			}
		};

		let cache = &self.state.slot_caches[cache_index as usize];
		let same_argument_count = cache.arguments.len() == arguments.1 - arguments.0;
		let mut reusable = same_argument_count && cache.slots.len() == definition.nodes.len();
		if reusable && let Some(parent) = parent {
			for (scope_key, expected) in cache.scope_reads.clone() {
				if !matches!(self.resolve_scope(parent, scope_key), Some(Ok((_, id))) if id == expected) {
					reusable = false;
					break;
				}
			}
		}

		let cache = &mut self.state.slot_caches[cache_index as usize];
		if reusable {
			self.state.slots[base..].copy_from_slice(&cache.slots);
			let mut stack = self.state.dirty.get(&index).cloned().unwrap_or_default();
			let current_arguments = &self.state.arguments[arguments.0..arguments.1];
			for (import, (previous, current)) in cache.arguments.iter().zip(current_arguments).enumerate() {
				if previous != current
					&& let Some(readers) = cache.import_readers.get(import)
				{
					stack.extend_from_slice(readers);
				}
			}
			cache.arguments = current_arguments.into();
			while let Some(local) = stack.pop() {
				if std::mem::replace(&mut self.state.slots[base + local as usize], UNLOWERED) != UNLOWERED {
					self.state.statistics.nodes_invalidated += 1;
					stack.extend_from_slice(&cache.dependents[local as usize]);
				}
			}
			self.state.statistics.slot_caches_reused += 1;
		} else {
			cache.arguments = self.state.arguments[arguments.0..arguments.1].into();
		}
		Ok(Some(cache_index))
	}

	/// Stores an instance's slots in its slot cache, moving export lists lowered this compile out of the export stack.
	fn persist_slots(&mut self, frame: &Frame, scope_reads: Box<[(u32, u32)]>) {
		let Some(cache_index) = frame.slot_cache else { return };
		let definition = self.definition(frame.definition);
		let mut slots = Vec::with_capacity(definition.nodes.len());
		for (local, node) in definition.nodes.iter().enumerate() {
			let mut slot = self.state.slots[frame.slots + local];
			if slot != UNLOWERED
				&& slot & PERSISTED == 0
				&& let Some(count) = self.export_count(definition, local, node)
			{
				let cache = &mut self.state.slot_caches[cache_index as usize];
				let start = cache.exports.len() as u32;
				cache.exports.extend_from_slice(&self.state.exports[slot as usize..slot as usize + count]);
				slot = PERSISTED | start;
			}
			slots.push(slot);
		}
		let cache = &mut self.state.slot_caches[cache_index as usize];
		cache.slots = slots;
		cache.scope_reads = scope_reads;
	}

	fn value_node(&mut self, ty: super::types::TypeId, hash: u64, source: Option<u64>, call_argument: super::types::TypeId, origin: (u64, u64)) -> Result<u32, String> {
		self.passes.build_value(NewNode {
			identifier: self.tables.builtins.value_node,
			call_argument,
			value: Some(IrValue { ty, hash, source }),
			skip_deduplication: None,
			compile_metadata: CompileMetadata::default(),
			origin,
		})
	}

	/// Resolves a scope key from `frame` outward, returning the frame depth providing it and the node it lowers to.
	fn resolve_scope(&mut self, frame: &Frame, key: u32) -> Option<Result<(u32, u32), String>> {
		let mut scope = Some(frame);
		while let Some(current) = scope {
			if let Some(&(_, local)) = self.definition(current.definition).scopes.iter().find(|(scope_key, _)| *scope_key == key) {
				return Some(self.lower_output(current, local, 0).map(|id| (current.depth, id)));
			}
			scope = current.parent;
		}
		None
	}

	fn lower_input(&mut self, frame: &Frame, input: &Input, owner_path: u32) -> Result<u32, String> {
		let builtins = self.tables.builtins;
		match *input {
			Input::Node { local, output } => self.lower_output(frame, local, output),
			Input::Value(value) => {
				let origin = self.origin(frame.path, 0);
				self.value_node(value.ty, value.hash, None, builtins.context, origin)
			}
			Input::Import(index) => Ok(self.state.arguments[frame.arguments.0 + index as usize]),
			Input::Scope(key) => {
				let (provider_depth, id) = self.resolve_scope(frame, key).ok_or_else(|| format!("Scope key {key} not found"))??;
				// Instances below the providing frame depend on this binding
				for active in self.state.active.iter_mut().rev().take_while(|active| active.depth > provider_depth) {
					active.scope_reads.push((key, id));
				}
				Ok(id)
			}
			Input::Reflection(reflection) => {
				let path_hash = self.path_hash(owner_path);
				let (ty, source) = match reflection {
					Reflection::DocumentNodePath => (builtins.node_id_path, None),
					Reflection::SourceId => (builtins.source_id, Some(path_hash)),
				};
				let hash = path_hash ^ (reflection as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
				self.value_node(ty, hash, source, builtins.context, (path_hash, 0))
			}
			Input::Inline => Err("Inline inputs are not supported".to_string()),
		}
	}

	fn lower_output(&mut self, frame: &Frame, local: u32, output: u32) -> Result<u32, String> {
		let slot = self.state.slots[frame.slots + local as usize];
		let definition = self.definition(frame.definition);
		let node = &definition.nodes[local as usize];
		let multiple_outputs = self.export_count(definition, local as usize, node).is_some();

		if slot != UNLOWERED {
			return Ok(match (multiple_outputs, slot & PERSISTED != 0) {
				(false, _) => slot,
				(true, true) => {
					let cache = frame.slot_cache.expect("persisted slots come from the frame's slot cache");
					self.state.slot_caches[cache as usize].exports[((slot & !PERSISTED) + output) as usize]
				}
				(true, false) => self.state.exports[(slot + output) as usize],
			});
		}

		let (slot, result) = match self.lower_node(frame, local)? {
			Lowered::Single(id) => (id, id),
			Lowered::Exports(start) => (start, self.state.exports[(start + output) as usize]),
		};
		self.state.slots[frame.slots + local as usize] = slot;
		Ok(result)
	}

	/// Lowers a network node's inputs as its instance's arguments. A substitution takes exactly its template's inputs,
	/// filling in the ones the document node lacks and dropping extra ones.
	fn lower_arguments(&mut self, frame: &Frame, inputs: &[Input], template_defaults: Option<&[Input]>, path: u32) -> Result<(usize, usize), String> {
		let start = self.state.arguments.len();
		let count = template_defaults.map_or(inputs.len(), <[Input]>::len);
		for index in 0..count {
			let input = inputs
				.get(index)
				.or_else(|| template_defaults.map(|defaults| &defaults[index]))
				.expect("the index is within the inputs or the defaults");
			let argument = self.lower_input(frame, input, path)?;
			self.state.arguments.push(argument);
		}
		Ok((start, self.state.arguments.len()))
	}

	fn lower_node(&mut self, frame: &Frame, local: u32) -> Result<Lowered, String> {
		let definition = self.definition(frame.definition);
		let node = &definition.nodes[local as usize];
		let inputs = definition.node_inputs(node);
		let builtins = self.tables.builtins;

		// A hidden node passes its primary input through, or the unit value when it has none
		if !node.visible {
			let path = self.node_path(frame, node.id);
			return Ok(Lowered::Single(match inputs.first() {
				Some(input) => self.lower_input(frame, input, path)?,
				None => {
					let origin = self.origin(path, node.id);
					self.value_node(builtins.unit, builtins.none_hash, None, builtins.context, origin)?
				}
			}));
		}

		match node.implementation {
			Implementation::Proto(identifier) if identifier == builtins.passthrough => {
				let path = self.node_path(frame, node.id);
				Ok(Lowered::Single(self.lower_input(frame, &inputs[0], path)?))
			}
			Implementation::Proto(identifier) if self.is_substituted(definition, node) => {
				let substitution = self.substitution(identifier).expect("checked by `is_substituted`");
				let metadata = node.compile_metadata.or_else(|| self.registered_metadata(identifier)).unwrap_or_default();
				let path = self.node_path(frame, node.id);
				let arguments = self.lower_arguments(frame, inputs, Some(&substitution.defaults), path)?;
				let exports = self.instantiate(frame, substitution.definition, arguments, path, Some((substitution.wrapped_local, metadata)))?;
				self.state.arguments.truncate(arguments.0);
				Ok(Lowered::Exports(exports))
			}
			Implementation::Proto(identifier)
				if identifier == builtins.value_node
					&& let [Input::Value(value)] = inputs =>
			{
				let origin = self.origin(frame.path, node.id);
				Ok(Lowered::Single(self.value_node(value.ty, value.hash, None, node.call_argument, origin)?))
			}
			Implementation::Proto(identifier) => {
				let path = self.node_path(frame, node.id);
				let start = self.state.scratch.len();
				for input in inputs {
					let lowered = self.lower_input(frame, input, path)?;
					self.state.scratch.push(lowered);
				}

				let compile_metadata = match frame.wrapped_metadata {
					Some((wrapped_local, metadata)) if wrapped_local == local => metadata,
					_ if definition.in_document => node.compile_metadata.or_else(|| self.registered_metadata(identifier)).unwrap_or_default(),
					_ => node.compile_metadata.unwrap_or_default(),
				};
				let new_node = NewNode {
					identifier,
					call_argument: node.call_argument,
					value: None,
					skip_deduplication: node.skip_deduplication.then(|| self.path_hash(path)),
					compile_metadata,
					origin: self.origin(frame.path, node.id),
				};
				let id = self.passes.build(new_node, &mut self.state.scratch[start..]);
				self.state.scratch.truncate(start);
				Ok(Lowered::Single(id?))
			}
			Implementation::Network(network) => {
				let path = self.node_path(frame, node.id);
				let arguments = self.lower_arguments(frame, inputs, None, path)?;
				let exports = self.instantiate(frame, network, arguments, path, None)?;
				self.state.arguments.truncate(arguments.0);
				Ok(Lowered::Exports(exports))
			}
			Implementation::Extract => Err("Extract nodes are not supported".to_string()),
		}
	}

	fn instance_key(&self, deep_hash: u64, path_hash: u64, wrapped_metadata: Option<(u32, CompileMetadata)>, arguments: &[u32]) -> u64 {
		let mut hasher = FxHasher::default();
		(deep_hash, path_hash, wrapped_metadata, arguments).hash(&mut hasher);
		hasher.finish()
	}

	/// Finds a cached instance with the same key whose outer scope bindings still resolve to the same nodes.
	fn cached_instance(
		&mut self,
		parent: &Frame,
		key: u64,
		deep_hash: u64,
		path_hash: u64,
		wrapped_metadata: Option<(u32, CompileMetadata)>,
		arguments: (usize, usize),
	) -> Result<Option<u32>, String> {
		let candidates = self.state.instances.get(&key).map_or(0, Vec::len);
		for candidate in 0..candidates {
			let entry = &self.state.instances[&key][candidate];
			let same_key =
				entry.deep_hash == deep_hash && entry.path_hash == path_hash && entry.wrapped_metadata == wrapped_metadata && *entry.arguments == self.state.arguments[arguments.0..arguments.1];
			if !same_key {
				continue;
			}

			let scope_reads = entry.scope_reads.clone();
			let mut resolved_reads = Vec::with_capacity(scope_reads.len());
			for &(scope_key, expected) in &scope_reads {
				match self.resolve_scope(parent, scope_key) {
					Some(resolved) => {
						let (provider_depth, id) = resolved?;
						if id != expected {
							break;
						}
						resolved_reads.push((scope_key, provider_depth, id));
					}
					None => break,
				}
			}
			if resolved_reads.len() == scope_reads.len() {
				// The enclosing instances being recorded depend on the reused instance's outer bindings too
				for (scope_key, provider_depth, id) in resolved_reads {
					for active in self.state.active.iter_mut().rev().take_while(|active| active.depth > provider_depth) {
						active.scope_reads.push((scope_key, id));
					}
				}
				let start = self.state.exports.len();
				let entry = &self.state.instances[&key][candidate];
				self.state.exports.extend_from_slice(&entry.exports);
				return Ok(Some(start as u32));
			}
		}
		Ok(None)
	}

	fn instantiate(&mut self, parent: &Frame, definition_id: DefinitionId, arguments: (usize, usize), path: u32, wrapped_metadata: Option<(u32, CompileMetadata)>) -> Result<u32, String> {
		let definition = self.definition(definition_id);
		let path_hash = self.path_hash(path);
		let key = self.instance_key(definition.deep_hash, path_hash, wrapped_metadata, &self.state.arguments[arguments.0..arguments.1]);
		// Substitution templates are a handful of nodes, cheaper to elaborate again than to look up
		let memoize = self.state.memoize_instances && matches!(definition_id, DefinitionId::Document(_));
		if memoize {
			if let Some(start) = self.cached_instance(parent, key, definition.deep_hash, path_hash, wrapped_metadata, arguments)? {
				self.state.statistics.instance_hits += 1;
				return Ok(start);
			}
			self.state.statistics.instance_misses += 1;
		}

		let depth = parent.depth + 1;
		let slots = self.state.slots.len();
		let slot_cache = match memoize {
			true => self.prepare_slots(definition_id, path, arguments, Some(parent))?,
			false => {
				self.state.slots.resize(slots + definition.nodes.len(), UNLOWERED);
				None
			}
		};
		let frame = Frame {
			definition: definition_id,
			slots,
			arguments,
			path,
			depth,
			parent: Some(parent),
			wrapped_metadata,
			slot_cache,
		};
		if memoize {
			self.state.active.push(ActiveInstance { depth, scope_reads: Vec::new() });
		}

		// Reserving the range first keeps it contiguous while nested instances push their own exports after it
		let start = self.state.exports.len();
		self.state.exports.resize(start + definition.exports.len(), UNLOWERED);
		for (index, export) in definition.exports.iter().enumerate() {
			if matches!(export, Input::Import(_)) {
				return Err("Network exports of imports are not supported by flattening".to_string());
			}
			self.state.exports[start + index] = self.lower_input(&frame, export, path)?;
		}

		if memoize {
			let active = self.state.active.pop().expect("this instance's record was pushed above");
			self.persist_slots(&frame, active.scope_reads.clone().into());
			let entry = InstanceEntry {
				deep_hash: definition.deep_hash,
				path_hash,
				wrapped_metadata,
				arguments: self.state.arguments[arguments.0..arguments.1].into(),
				exports: self.state.exports[start..start + definition.exports.len()].into(),
				scope_reads: active.scope_reads.into(),
			};
			self.state.instances.entry(key).or_default().push(entry);
		}
		self.state.slots.truncate(slots);
		Ok(start as u32)
	}
}
