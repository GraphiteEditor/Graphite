//! Implementation selection over interned types, memoized per identifier and input types.

use super::coercion::{Coercions, ConversionKind, ConversionNode, Rank};
use super::document::{IdentifierId, Strings};
use super::types::{TypeData, TypeId, TypeInterner};
use rustc_hash::FxHashMap;

pub struct Signature {
	pub call_argument: TypeId,
	pub inputs: Vec<TypeId>,
	pub output: TypeId,
	/// The generic names, collected like `collect_generics`.
	pub generics: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromotionKind {
	ItemToList,
	Bundle,
	Unbundle,
}

impl PromotionKind {
	const ADAPTER_PREFIXES: [(&str, PromotionKind); 3] = [
		("graphene_core::ops::ItemToListNode<", PromotionKind::ItemToList),
		("graphene_core::ops::BundleNode<", PromotionKind::Bundle),
		("graphene_core::ops::UnbundleNode<", PromotionKind::Unbundle),
	];
}

/// An adapter node to insert on an input after context analysis nullified it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Promotion {
	pub input: u32,
	pub adapter: IdentifierId,
}

/// A conversion node to insert on an input before the node can be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conversion {
	pub input: u32,
	pub node: ConversionNode,
}

#[derive(Clone, Debug)]
pub struct Selection {
	pub implementation: u32,
	pub return_type: TypeId,
	pub fn_type: TypeId,
	pub promotions: Vec<Promotion>,
	/// When not empty, the node is only buildable after inserting these conversions, which changes its input types and so
	/// its selection.
	pub conversions: Vec<Conversion>,
}

#[derive(Default)]
pub struct Registry {
	pub implementations: FxHashMap<IdentifierId, Vec<Signature>>,
	pub coercions: Coercions,
	/// Keyed by the element of the adapter's `List` side.
	promotion_adapters: FxHashMap<(PromotionKind, TypeId), IdentifierId>,
	selection_indices: FxHashMap<Box<[u32]>, u32>,
	selections: Vec<Result<Selection, String>>,
	key: Vec<u32>,
}

fn collect_generics(types: &TypeInterner, call_argument: TypeId, inputs: &[TypeId], output: TypeId) -> Vec<u32> {
	let mut generics = Vec::new();
	for ty in [call_argument].into_iter().chain(inputs.iter().copied()).chain([output]) {
		types.collect_generics(ty, &mut generics);
	}
	generics
}

/// Binds the generics in `expected` to the matching parts of `provided`, failing when one generic would bind two types.
/// Concrete parts are not compared, since `TypeInterner::valid` checks them.
fn bind_generics(types: &TypeInterner, expected: TypeId, provided: TypeId, bindings: &mut Vec<(u32, TypeId)>) -> bool {
	match (types.get(expected), types.get(provided)) {
		(TypeData::Generic(name), _) => match bindings.iter().find(|(generic, _)| *generic == name) {
			Some(&(_, bound)) => bound == provided,
			None => {
				bindings.push((name, provided));
				true
			}
		},
		(TypeData::Fn(expected_input, expected_output), TypeData::Fn(provided_input, provided_output)) => {
			bind_generics(types, expected_input, provided_input, bindings) && bind_generics(types, expected_output, provided_output, bindings)
		}
		(TypeData::Item(expected), TypeData::Item(provided)) | (TypeData::List(expected), TypeData::List(provided)) => bind_generics(types, expected, provided, bindings),
		_ => true,
	}
}

impl Registry {
	pub fn add(&mut self, types: &TypeInterner, identifier: IdentifierId, call_argument: TypeId, inputs: Vec<TypeId>, output: TypeId) {
		let generics = collect_generics(types, call_argument, &inputs, output);
		self.implementations.entry(identifier).or_default().push(Signature {
			call_argument,
			inputs,
			output,
			generics,
		});
	}

	pub fn index_promotion_adapters(&mut self, types: &TypeInterner, strings: &Strings) {
		for (&identifier, signatures) in &self.implementations {
			let name = strings.name(identifier.0);
			let Some(&(_, kind)) = PromotionKind::ADAPTER_PREFIXES.iter().find(|(prefix, _)| name.starts_with(prefix)) else {
				continue;
			};
			let [signature] = signatures.as_slice() else { continue };
			let [input] = signature.inputs.as_slice() else { continue };

			let list_side = if kind == PromotionKind::Bundle { types.nested(*input) } else { signature.output };
			if let TypeData::List(element) = types.get(list_side) {
				self.promotion_adapters.insert((kind, element), identifier);
			}
		}
	}

	pub fn clear_selections(&mut self) {
		self.selection_indices.clear();
		self.selections.clear();
	}

	pub fn select(&mut self, types: &mut TypeInterner, identifier: IdentifierId, call_argument: TypeId, inputs: &[TypeId]) -> Result<&Selection, String> {
		let index = self.select_index(types, identifier, call_argument, inputs);
		self.selection(index)
	}

	pub fn selection(&self, index: u32) -> Result<&Selection, String> {
		self.selections[index as usize].as_ref().map_err(Clone::clone)
	}

	/// Selects the implementation and returns the memoized result's index, which stays valid until selections are cleared.
	pub fn select_index(&mut self, types: &mut TypeInterner, identifier: IdentifierId, call_argument: TypeId, inputs: &[TypeId]) -> u32 {
		self.key.clear();
		self.key.push(identifier.0);
		self.key.push(call_argument.0);
		self.key.extend(inputs.iter().map(|input| input.0));

		match self.selection_indices.get(self.key.as_slice()) {
			Some(&index) => index,
			None => {
				let selection = self.resolve(types, identifier, call_argument, inputs);
				let index = self.selections.len() as u32;
				self.selections.push(selection);
				self.selection_indices.insert(self.key.clone().into_boxed_slice(), index);
				index
			}
		}
	}

	fn resolve(&self, types: &mut TypeInterner, identifier: IdentifierId, call_argument: TypeId, inputs: &[TypeId]) -> Result<Selection, String> {
		let implementations = self.implementations.get(&identifier).ok_or("No implementations found")?;

		if inputs.iter().any(|&input| matches!(types.get(input), TypeData::Fn(_, output) if types.generic_name(output).is_some())) {
			return Err("Generic inputs should not exist".to_string());
		}

		let mut valid = Vec::new();
		for (index, signature) in implementations.iter().enumerate() {
			let matches = types.valid(signature.call_argument, call_argument) && inputs.iter().zip(&signature.inputs).all(|(&provided, &expected)| types.valid(provided, expected));
			if !matches {
				continue;
			}
			if let Some((substituted_call, substituted_output)) = substitute_generics(types, signature, call_argument, inputs) {
				valid.push((index, substituted_call, substituted_output));
			}
		}

		let selected = match valid.as_slice() {
			[] => return self.resolve_adapted(types, implementations, call_argument, inputs),
			[single] => *single,
			[first, second] => {
				let unit = types.concrete(crate::baseline::types::UNIT_NAME);
				match [first, second].into_iter().find(|candidate| first.1 != second.1 && candidate.1 == unit) {
					Some(candidate) => *candidate,
					None => return Err("Multiple implementations found".to_string()),
				}
			}
			_ => return Err("Multiple implementations found".to_string()),
		};

		let (implementation, call, output) = selected;
		Ok(Selection {
			implementation: implementation as u32,
			return_type: output,
			fn_type: types.function(call, output),
			promotions: Vec::new(),
			conversions: Vec::new(),
		})
	}

	/// Plans how one input reaches its connector: unchanged, promoted, converted, or converted at its own rank and then
	/// promoted. Conversions need a concrete connector element, since the target must be known to pick one.
	fn plan_edge(&self, types: &mut TypeInterner, provided: TypeId, expected: TypeId) -> Option<EdgePlan> {
		if types.valid(provided, expected) {
			return Some(EdgePlan::unchanged(provided));
		}
		let call_argument = types.fn_input(provided)?;
		if !types.valid(call_argument, types.fn_input(expected)?) {
			return None;
		}
		let concrete_connector = !contains_generic(types, expected);

		if let Some((kind, element)) = promotion_between(types, provided, expected) {
			let element = match (kind, types.get(types.nested(provided))) {
				(PromotionKind::ItemToList, TypeData::Item(provided_element)) if !concrete_connector => provided_element,
				_ if concrete_connector => element,
				_ => return None,
			};
			let adapter = *self.promotion_adapters.get(&(kind, element))?;
			let adapted = match concrete_connector {
				true => expected,
				false => {
					let list = types.intern(TypeData::List(element));
					types.function(call_argument, list)
				}
			};
			return Some(EdgePlan {
				adapted,
				conversion: None,
				promotion: Some(adapter),
				target: None,
			});
		}
		if !concrete_connector {
			return None;
		}

		let (provided_wire, expected_wire) = (types.nested(provided), types.nested(expected));
		if let Some(conversion) = self.coercions.wire_conversion(provided_wire, expected_wire) {
			return Some(EdgePlan {
				adapted: expected,
				conversion: Some(conversion),
				promotion: None,
				target: Some(expected_wire),
			});
		}

		let (from_rank, from) = Coercions::ranked_element(types, provided)?;
		let (to_rank, to) = Coercions::ranked_element(types, expected)?;
		let conversion = self.coercions.automatic_conversion(from, to, from_rank)?;
		let promotion = match (from_rank, to_rank) {
			(from_rank, to_rank) if from_rank == to_rank => None,
			(Rank::Item, Rank::List) => Some(*self.promotion_adapters.get(&(PromotionKind::ItemToList, to))?),
			_ => return None,
		};
		Some(EdgePlan {
			adapted: expected,
			conversion: Some(conversion),
			promotion,
			target: Some(to),
		})
	}

	/// Selects among all implementations reachable by adapting inputs, each input taking at most one conversion followed by at
	/// most one promotion. Candidates rank by fewest lossy conversions, then purity-losing ones, then representation changes,
	/// then promotions. Remaining ties go to the candidate whose converted inputs receive the purest data, or are ambiguous.
	fn resolve_adapted(&self, types: &mut TypeInterner, implementations: &[Signature], call_argument: TypeId, inputs: &[TypeId]) -> Result<Selection, String> {
		let mut candidates: Vec<Candidate> = Vec::new();
		'implementations: for (index, signature) in implementations.iter().enumerate() {
			if !types.valid(signature.call_argument, call_argument) || inputs.len() != signature.inputs.len() {
				continue;
			}

			let mut candidate = Candidate {
				implementation: index,
				..Default::default()
			};
			for (input, (&provided, &expected)) in inputs.iter().zip(&signature.inputs).enumerate() {
				let Some(plan) = self.plan_edge(types, provided, expected) else { continue 'implementations };
				candidate.adapted.push(plan.adapted);
				candidate.targets.push(plan.target);
				if let Some(node) = plan.conversion {
					candidate.conversions.push(Conversion { input: input as u32, node });
				}
				if let Some(adapter) = plan.promotion {
					candidate.promotions.push(Promotion { input: input as u32, adapter });
				}
			}
			let Some((call, output)) = substitute_generics(types, signature, call_argument, &candidate.adapted) else {
				continue;
			};
			candidate.substituted = (call, output);
			candidates.push(candidate);
		}

		let Some(cheapest) = candidates.iter().map(Candidate::cost).min() else {
			let provided: Vec<_> = inputs.iter().map(|&input| types.to_workload(types.nested(input))).collect();
			return Err(format!("No implementation is reachable through automatic adaptation from {provided:?}"));
		};
		candidates.retain(|candidate| candidate.cost() == cheapest);

		let at_least_as_pure = |a: &Candidate, b: &Candidate| {
			a.targets.iter().zip(&b.targets).all(|pair| match pair {
				(Some(a), Some(b)) => a == b || self.coercions.is_purer(*a, *b),
				_ => true,
			})
		};
		let best: Vec<_> = candidates.iter().filter(|candidate| candidates.iter().all(|other| at_least_as_pure(candidate, other))).collect();
		let [best] = best.as_slice() else {
			return Err(if cheapest.0 + cheapest.1 + cheapest.2 > 0 {
				"Ambiguous automatic conversions"
			} else {
				"Invalid implementations"
			}
			.to_string());
		};

		let (call, output) = best.substituted;
		Ok(Selection {
			implementation: best.implementation as u32,
			return_type: output,
			fn_type: types.function(call, output),
			promotions: best.promotions.clone(),
			conversions: best.conversions.clone(),
		})
	}
}

struct EdgePlan {
	adapted: TypeId,
	conversion: Option<ConversionNode>,
	promotion: Option<IdentifierId>,
	/// The element a conversion produces, which ranks candidates by purity.
	target: Option<TypeId>,
}

impl EdgePlan {
	fn unchanged(provided: TypeId) -> Self {
		Self {
			adapted: provided,
			conversion: None,
			promotion: None,
			target: None,
		}
	}
}

#[derive(Default)]
struct Candidate {
	implementation: usize,
	adapted: Vec<TypeId>,
	targets: Vec<Option<TypeId>>,
	conversions: Vec<Conversion>,
	promotions: Vec<Promotion>,
	substituted: (TypeId, TypeId),
}

impl Candidate {
	fn cost(&self) -> (usize, usize, usize, usize) {
		let count = |kind| self.conversions.iter().filter(|conversion| conversion.node.kind == kind).count();
		(
			count(ConversionKind::Lossy),
			count(ConversionKind::PurityLosing),
			count(ConversionKind::Representation),
			self.promotions.len(),
		)
	}
}

fn contains_generic(types: &TypeInterner, ty: TypeId) -> bool {
	match types.get(ty) {
		TypeData::Generic(_) => true,
		TypeData::Concrete(_) => false,
		TypeData::Item(element) | TypeData::List(element) => contains_generic(types, element),
		TypeData::Fn(input, output) => contains_generic(types, input) || contains_generic(types, output),
	}
}

/// Resolves the signature's generics against the provided types like `check_generic` and `replace_generics`, returning the substituted call argument and output.
fn substitute_generics(types: &mut TypeInterner, signature: &Signature, call_argument: TypeId, inputs: &[TypeId]) -> Option<(TypeId, TypeId)> {
	if signature.generics.is_empty() {
		return Some((signature.call_argument, signature.output));
	}

	let mut bindings: Vec<(u32, TypeId)> = Vec::with_capacity(signature.generics.len());
	let pairs = [(signature.call_argument, call_argument)]
		.into_iter()
		.chain(signature.inputs.iter().copied().zip(inputs.iter().copied()));
	for (expected, provided) in pairs {
		if !bind_generics(types, expected, provided, &mut bindings) {
			return None;
		}
	}
	if signature.generics.iter().any(|generic| !bindings.iter().any(|(bound, _)| bound == generic)) {
		return None;
	}

	let replace = |name: u32| bindings.iter().find(|(generic, _)| *generic == name).map(|(_, ty)| *ty);
	Some((types.substitute(signature.call_argument, &replace), types.substitute(signature.output, &replace)))
}

fn promotion_between(types: &TypeInterner, provided: TypeId, expected: TypeId) -> Option<(PromotionKind, TypeId)> {
	let (TypeData::Fn(provided_input, provided_output), TypeData::Fn(expected_input, expected_output)) = (types.get(provided), types.get(expected)) else {
		return None;
	};
	if !types.valid(provided_input, expected_input) {
		return None;
	}

	let (from, to) = (types.nested(provided_output), types.nested(expected_output));
	match (types.get(from), types.get(to)) {
		(TypeData::Item(from_element), TypeData::List(element)) if types.valid(from_element, element) => Some((PromotionKind::ItemToList, element)),
		(TypeData::List(element), TypeData::Item(_)) if types.bundle_element_name(to).is_some() && types.bundle_element_name(to) == types.concrete_name(element) => {
			Some((PromotionKind::Bundle, element))
		}
		(TypeData::Item(_), TypeData::List(element)) if types.bundle_element_name(from).is_some() && types.bundle_element_name(from) == types.concrete_name(element) => {
			Some((PromotionKind::Unbundle, element))
		}
		_ => None,
	}
}
