//! Automatic conversions between data types through `Convert` and `ConvertAsync` implementations, chosen by a purity relation
//! instead of hardcoded in the compiler.
//!
//! A conversion from `A` to `B` is applied automatically when `A` is purer than `B` (data loses purity but never regains it by
//! discarding information, which needs an explicit node) or when both are representations of the same data, like CPU and GPU
//! rasters. At most one conversion is applied per input. Among the implementations a node could use after converting, the one
//! needing the fewest purity-losing conversions, then the fewest representation changes, then receiving the purest data wins.

use super::document::{IdentifierId, Strings};
use super::registry::Registry;
use super::types::{TypeData, TypeId, TypeInterner};
use crate::workload::CompileMetadata;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::path::Path;

const FOOTPRINT: u32 = 1 << 0;

/// A `Convert<To, Converter>` (or `ConvertAsync`) implementation for `From`.
#[derive(Deserialize)]
struct ConvertImplementation {
	from: String,
	to: String,
	converter: Option<ConverterDeclaration>,
	#[serde(default)]
	asynchronous: bool,
	#[serde(default)]
	reads_footprint: bool,
}

#[derive(Deserialize)]
struct ConverterDeclaration {
	#[serde(rename = "type")]
	ty: String,
	scope: String,
}

#[derive(Deserialize)]
struct NodeDeclaration {
	identifier: String,
	element_types: Vec<String>,
}

/// The spike's stand-in for purity declarations on Graphite's types, the `Convert` implementations, and test nodes.
#[derive(Deserialize)]
struct CoercionData {
	purity: Vec<(String, String)>,
	equivalent: Vec<(String, String)>,
	conversions: Vec<ConvertImplementation>,
	nodes: Vec<NodeDeclaration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rank {
	Item,
	List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConversionKind {
	/// Between representations of the same data, which is semantically invisible.
	Representation,
	/// From purer data to less pure data.
	PurityLosing,
}

/// The conversion node turning one element type into another at a rank, like `convert_item` over `Convert<To, Converter>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConversionNode {
	pub identifier: IdentifierId,
	/// The scope key providing the `Converter` the implementation needs as a second input, like the `WgpuExecutor`.
	pub converter_scope: Option<u32>,
	pub kind: ConversionKind,
	pub reads_footprint: bool,
}

impl ConversionNode {
	pub fn compile_metadata(&self) -> CompileMetadata {
		CompileMetadata {
			extract: if self.reads_footprint { FOOTPRINT } else { 0 },
			..Default::default()
		}
	}
}

#[derive(Default)]
pub struct Coercions {
	/// Pairs `(purer, less pure)`, transitively closed.
	purer: FxHashSet<(TypeId, TypeId)>,
	equivalent: FxHashSet<(TypeId, TypeId)>,
	conversions: FxHashMap<(TypeId, TypeId, Rank), ConversionNode>,
}

impl Coercions {
	pub fn is_purer(&self, a: TypeId, b: TypeId) -> bool {
		self.purer.contains(&(a, b))
	}

	/// The automatic conversion from one element type to another, if `Convert` implements it and purity allows applying it.
	pub fn automatic_conversion(&self, from: TypeId, to: TypeId, rank: Rank) -> Option<ConversionNode> {
		let conversion = self.conversions.get(&(from, to, rank))?;
		let allowed = match conversion.kind {
			ConversionKind::Representation => true,
			ConversionKind::PurityLosing => self.is_purer(from, to),
		};
		allowed.then_some(*conversion)
	}

	/// The rank and element of a wire type `Fn(_, Item<T>)` or `Fn(_, List<T>)`.
	pub fn ranked_element(types: &TypeInterner, wire: TypeId) -> Option<(Rank, TypeId)> {
		match types.get(types.nested(wire)) {
			TypeData::Item(element) => Some((Rank::Item, element)),
			TypeData::List(element) => Some((Rank::List, element)),
			_ => None,
		}
	}
}

/// Loads the spike's coercion data, registering the conversion nodes' implementations and the test nodes.
pub fn load(types: &mut TypeInterner, strings: &mut Strings, registry: &mut Registry, context: TypeId) {
	let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("spike-data/coercions.json");
	let content = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("Failed to read {}: {error}", path.display()));
	let data: CoercionData = serde_json::from_str(&content).unwrap_or_else(|error| panic!("Failed to parse {}: {error}", path.display()));

	let equivalent = data
		.equivalent
		.iter()
		.flat_map(|(a, b)| {
			let (a, b) = (types.concrete(a), types.concrete(b));
			[(a, b), (b, a)]
		})
		.collect();
	let mut coercions = Coercions { equivalent, ..Default::default() };

	for conversion in &data.conversions {
		let (from, to) = (types.concrete(&conversion.from), types.concrete(&conversion.to));
		let converter = conversion.converter.as_ref().map(|converter| (types.concrete(&converter.ty), strings.intern(&converter.scope)));
		let kind = if coercions.equivalent.contains(&(from, to)) {
			ConversionKind::Representation
		} else {
			ConversionKind::PurityLosing
		};

		for (rank, wrap) in [(Rank::Item, TypeData::Item as fn(TypeId) -> TypeData), (Rank::List, TypeData::List)] {
			// Monomorphized over the target like `convert_item`'s `PhantomData<E>`, so typing the inserted node is unambiguous
			let node = match (rank, conversion.asynchronous) {
				(Rank::Item, false) => "graphene_core::ops::ConvertItemNode",
				(Rank::List, false) => "graphene_core::ops::ConvertListNode",
				(Rank::Item, true) => "graphene_core::ops::ConvertAsyncItemNode",
				(Rank::List, true) => "graphene_core::ops::ConvertAsyncListNode",
			};
			let identifier = IdentifierId(strings.intern(&format!("{node}<{}>", conversion.to)));

			let (from_wire, to_wire) = (types.intern(wrap(from)), types.intern(wrap(to)));
			let mut inputs = vec![types.function(context, from_wire)];
			if let Some((converter, _)) = converter {
				let converter_wire = types.intern(TypeData::Item(converter));
				inputs.push(types.function(context, converter_wire));
			}
			registry.add(types, identifier, context, inputs, to_wire);
			coercions.conversions.insert(
				(from, to, rank),
				ConversionNode {
					identifier,
					converter_scope: converter.map(|(_, scope)| scope),
					kind,
					reads_footprint: conversion.reads_footprint,
				},
			);
		}
	}

	for node in &data.nodes {
		let identifier = IdentifierId(strings.intern(&node.identifier));
		for element in &node.element_types {
			let element = types.concrete(element);
			for wrap in [TypeData::Item as fn(TypeId) -> TypeData, TypeData::List] {
				let wire = types.intern(wrap(element));
				let input = types.function(context, wire);
				registry.add(types, identifier, context, vec![input], wire);
			}
		}
	}

	let mut purer: FxHashSet<(TypeId, TypeId)> = data.purity.iter().map(|(a, b)| (types.concrete(a), types.concrete(b))).collect();
	// Transitive closure, O(n³) over the handful of declared types
	loop {
		let derived: Vec<_> = purer
			.iter()
			.flat_map(|&(a, b)| purer.iter().filter(move |&&(c, _)| c == b).map(move |&(_, d)| (a, d)))
			.filter(|pair| !purer.contains(pair))
			.collect();
		if derived.is_empty() {
			break;
		}
		purer.extend(derived);
	}
	if let Some(&(cycle, _)) = purer.iter().find(|&&(a, b)| a == b) {
		panic!("The purity relation has a cycle through {}", types.concrete_name(cycle).unwrap_or("a type"));
	}
	coercions.purer = purer;
	registry.coercions = coercions;
}
