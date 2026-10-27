//! The context analysis pass: tracks which context features each node depends on and reads inputs through nullification
//! nodes where their dependencies differ, like `find_context_dependencies` with memoization.

use super::Builtins;
use super::checks::Checks;
use super::document::IdentifierId;
use super::ir::{Ir, IrValue, NewNode};
use super::registry::{Conversion, Promotion};
use super::typing::{Inferred, Typing};
use crate::workload::CompileMetadata;
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::{Hash, Hasher};

/// Builds nodes through the store, typing, and checks, which every pass inserting nodes goes through.
pub struct Builder<'a> {
	pub ir: &'a mut Ir,
	pub typing: &'a mut Typing,
	pub checks: &'a mut Checks,
}

/// What building a node produced: the node, or the conversions its inputs need before it can be built.
pub enum Built {
	Node { id: u32, new: bool },
	NeedsConversions(Box<[Conversion]>),
}

impl Builder<'_> {
	/// Returns the existing node with this content, or types, checks, and appends it. `selected` is typing's selection
	/// for inputs of the same types, when already made.
	pub fn build(&mut self, node: NewNode, inputs: &[u32], selected: Option<u32>) -> Result<Built, String> {
		let (hash, existing) = self.ir.find(&node, inputs);
		if let Some(id) = existing {
			return Ok(Built::Node { id, new: false });
		}

		let info = match self.typing.infer(self.ir, &node, inputs, selected)? {
			Inferred::Typed(info) => info,
			Inferred::NeedsConversions(conversions) => return Ok(Built::NeedsConversions(conversions)),
		};
		let resolves_to_value = self.checks.check(&node, inputs)?;
		let id = self.ir.push(hash, node, inputs);
		self.typing.info.push(info);
		self.checks.resolves_to_value.push(resolves_to_value);
		Ok(Built::Node { id, new: true })
	}

	/// Builds a node that never needs conversions, such as values and compiler-inserted nodes over any type.
	fn build_direct(&mut self, node: NewNode, inputs: &[u32]) -> Result<(u32, bool), String> {
		match self.build(node, inputs, None)? {
			Built::Node { id, new } => Ok((id, new)),
			Built::NeedsConversions(_) => Err("A compiler-inserted node should not need conversions".to_string()),
		}
	}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Analysis {
	pub features: u32,
	pub sources: u32,
	/// Whether the node supplies context features its inputs need, so consumers read it through a nullification.
	pub supplies: bool,
}

pub struct ContextAnalysis {
	/// Indexed by IR node ID.
	pub analysis: Vec<Analysis>,
	source_sets: Vec<Vec<u64>>,
	source_set_ids: FxHashMap<Vec<u64>, u32>,
	builtins: Builtins,
}

impl ContextAnalysis {
	pub fn new(builtins: Builtins) -> Self {
		let mut analysis = Self {
			analysis: Vec::new(),
			source_sets: Vec::new(),
			source_set_ids: FxHashMap::default(),
			builtins,
		};
		analysis.clear();
		analysis
	}

	pub fn clear(&mut self) {
		self.analysis.clear();
		self.source_sets.clear();
		self.source_sets.push(Vec::new());
		self.source_set_ids.clear();
		self.source_set_ids.insert(Vec::new(), 0);
	}

	fn source_set(&mut self, sources: Vec<u64>) -> u32 {
		if let Some(&id) = self.source_set_ids.get(&sources) {
			return id;
		}
		let id = self.source_sets.len() as u32;
		self.source_sets.push(sources.clone());
		self.source_set_ids.insert(sources, id);
		id
	}

	fn union_sources(&mut self, a: u32, b: u32) -> u32 {
		if a == b || b == 0 {
			return a;
		}
		if a == 0 {
			return b;
		}
		let mut union: Vec<u64> = self.source_sets[a as usize].iter().chain(&self.source_sets[b as usize]).copied().collect();
		union.sort_unstable();
		union.dedup();
		self.source_set(union)
	}

	fn record(&mut self, id: u32, analysis: Analysis) {
		match (id as usize).cmp(&self.analysis.len()) {
			std::cmp::Ordering::Less => self.analysis[id as usize] = analysis,
			std::cmp::Ordering::Equal => self.analysis.push(analysis),
			std::cmp::Ordering::Greater => unreachable!("nodes are analyzed in the order they are built"),
		}
	}

	/// Builds a value or compiler-inserted node, whose analysis does not depend on its inputs' dependencies.
	fn build_unanalyzed(&mut self, builder: &mut Builder, node: NewNode, inputs: &[u32]) -> Result<u32, String> {
		let (id, new) = builder.build_direct(node, inputs)?;
		if new {
			let sources = match node.value {
				Some(IrValue { source: Some(source), .. }) => self.source_set(vec![source]),
				_ => 0,
			};
			self.record(
				id,
				Analysis {
					features: node.compile_metadata.extract,
					sources,
					supplies: false,
				},
			);
		}
		Ok(id)
	}

	pub fn build_value(&mut self, builder: &mut Builder, node: NewNode) -> Result<u32, String> {
		self.build_unanalyzed(builder, node, &[])
	}

	/// The node a consumer reads in place of `child` when it needs `features` and `sources` stripped from the context.
	fn nullification(&mut self, builder: &mut Builder, child: u32, analysis: Analysis) -> Result<u32, String> {
		let builtins = self.builtins;
		let origin = builder.ir.nodes[child as usize].origin;
		let forwarding = CompileMetadata {
			forwards_input: true,
			..Default::default()
		};
		let compiler_node = |identifier| NewNode {
			identifier,
			call_argument: builtins.context,
			value: None,
			skip_deduplication: None,
			compile_metadata: forwarding,
			origin,
		};

		let memoize = self.build_unanalyzed(builder, compiler_node(builtins.memoize), &[child])?;

		let mut hasher = FxHasher::default();
		analysis.features.hash(&mut hasher);
		self.source_sets[analysis.sources as usize].hash(&mut hasher);
		let modification = NewNode {
			identifier: builtins.value_node,
			call_argument: builtins.context,
			value: Some(IrValue {
				ty: builtins.context_modification,
				hash: hasher.finish(),
				source: None,
			}),
			skip_deduplication: None,
			compile_metadata: CompileMetadata::default(),
			origin,
		};
		let modification = self.build_unanalyzed(builder, modification, &[])?;

		let nullification = self.build_unanalyzed(builder, compiler_node(builtins.context_modification_node), &[memoize, modification])?;
		self.record(nullification, Analysis { supplies: false, ..analysis });
		Ok(nullification)
	}

	/// Adapters only change an input's rank, so they share its analysis.
	fn promotion_adapter(&mut self, builder: &mut Builder, input: u32, adapter: IdentifierId) -> Result<u32, String> {
		let node = NewNode {
			identifier: adapter,
			call_argument: self.builtins.context,
			value: None,
			skip_deduplication: None,
			compile_metadata: CompileMetadata::default(),
			origin: builder.ir.nodes[input as usize].origin,
		};
		let (id, _) = builder.build_direct(node, &[input])?;
		self.record(id, self.analysis[input as usize]);
		Ok(id)
	}

	/// Replaces `inputs` with what the node reads after nullification, builds the node, and records its own analysis.
	/// Returns the conversions instead, with `inputs` untouched, when typing finds the inputs need some first.
	pub fn build(&mut self, builder: &mut Builder, node: NewNode, inputs: &mut [u32]) -> Result<Result<u32, Box<[Conversion]>>, String> {
		// Nullification keeps every input's type, so selecting before it gives the same implementation
		let selected = node.value.is_none().then(|| builder.typing.select(&node, inputs));
		if let Some(selection) = selected {
			let conversions = builder.typing.conversions(selection)?;
			if !conversions.is_empty() {
				return Ok(Err(conversions.into()));
			}
		}

		let (mut combined_features, mut combined_sources) = (0, 0);
		for &input in inputs.iter() {
			let child = self.analysis[input as usize];
			combined_features |= child.features;
			combined_sources = self.union_sources(combined_sources, child.sources);
		}

		let metadata = node.compile_metadata;
		let features = (combined_features & !metadata.inject) | metadata.extract;
		let introduces_new_dependencies = combined_features & features != features;

		for input in inputs.iter_mut() {
			let child = self.analysis[*input as usize];
			let is_value = builder.ir.nodes[*input as usize].value.is_some();
			let differs = introduces_new_dependencies || child.features != combined_features || child.sources != combined_sources;
			if !is_value && (child.supplies || differs) {
				*input = self.nullification(builder, *input, child)?;
			}
		}

		if let Some(selection) = selected {
			let promotions = builder.typing.promotions(selection)?;
			if !promotions.is_empty() {
				let promotions: Box<[Promotion]> = promotions.into();
				for promotion in promotions.iter() {
					let input = &mut inputs[promotion.input as usize];
					*input = self.promotion_adapter(builder, *input, promotion.adapter)?;
				}
			}
		}

		let Built::Node { id, new } = builder.build(node, inputs, selected)? else {
			return Err("Conversions are inserted before context analysis".to_string());
		};
		let net_injections = metadata.inject & !metadata.extract;
		let remaining = combined_features & !net_injections;
		self.record(
			id,
			Analysis {
				features,
				sources: combined_sources,
				supplies: combined_features & !remaining != 0,
			},
		);
		if new {
			builder.checks.check_footprint(id, &node, features);
		}
		Ok(Ok(id))
	}
}
