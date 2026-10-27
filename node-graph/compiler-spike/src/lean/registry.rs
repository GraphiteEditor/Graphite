//! Implementation selection over interned types, memoized per identifier and input types.

use super::coercion::{Coercions, ConversionKind, ConversionNode};
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
	/// When not empty, the node is only buildable after inserting these conversions, and the other fields are unset.
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
	let mut generics: Vec<u32> = [call_argument]
		.into_iter()
		.chain(inputs.iter().map(|&input| types.nested(input)))
		.filter_map(|ty| types.generic_name(ty))
		.collect();
	generics.extend(types.generic_name(output));
	generics.dedup();
	generics
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
			[] => return self.resolve_with_promotions(types, implementations, call_argument, inputs),
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

	/// Finds the implementation reachable by converting inputs, at most one automatic conversion per input, preferring the
	/// fewest purity-losing conversions, then the fewest representation changes, then the purest converted data.
	/// Incomparable best candidates are ambiguous.
	fn resolve_with_coercions(&self, types: &TypeInterner, implementations: &[Signature], call_argument: TypeId, inputs: &[TypeId]) -> Result<Selection, String> {
		// Each candidate is an implementation index with its conversions and the element type each converted input receives
		let mut candidates: Vec<(usize, Vec<Conversion>, Vec<TypeId>)> = Vec::new();
		'implementations: for (index, signature) in implementations.iter().enumerate() {
			if !signature.generics.is_empty() || !types.valid(signature.call_argument, call_argument) || inputs.len() != signature.inputs.len() {
				continue;
			}
			let (mut conversions, mut targets) = (Vec::new(), Vec::new());
			for (input, (&provided, &expected)) in inputs.iter().zip(&signature.inputs).enumerate() {
				if types.valid(provided, expected) {
					continue;
				}
				let (Some((from_rank, from)), Some((to_rank, to))) = (Coercions::ranked_element(types, provided), Coercions::ranked_element(types, expected)) else {
					continue 'implementations;
				};
				let conversion = (from_rank == to_rank).then(|| self.coercions.automatic_conversion(from, to, from_rank)).flatten();
				let Some(node) = conversion else { continue 'implementations };
				conversions.push(Conversion { input: input as u32, node });
				targets.push(to);
			}
			candidates.push((index, conversions, targets));
		}

		let cost = |conversions: &[Conversion]| {
			let count = |kind| conversions.iter().filter(|conversion| conversion.node.kind == kind).count();
			(count(ConversionKind::PurityLosing), count(ConversionKind::Representation))
		};
		let cheapest = candidates
			.iter()
			.map(|(_, conversions, _)| cost(conversions))
			.min()
			.ok_or("No implementation is reachable through automatic conversions")?;
		candidates.retain(|(_, conversions, _)| cost(conversions) == cheapest);

		// A candidate is better when every converted input receives data at least as pure, and some purer
		let at_least_as_pure = |a: &[TypeId], b: &[TypeId]| a.iter().zip(b).all(|(&a, &b)| a == b || self.coercions.is_purer(a, b));
		let best: Vec<_> = candidates
			.iter()
			.filter(|(_, _, targets)| candidates.iter().all(|(_, _, other)| at_least_as_pure(targets, other)))
			.collect();
		let [(_, conversions, _)] = best.as_slice() else {
			return Err("Ambiguous automatic conversions".to_string());
		};

		Ok(Selection {
			implementation: 0,
			return_type: TypeId::default(),
			fn_type: TypeId::default(),
			promotions: Vec::new(),
			conversions: conversions.clone(),
		})
	}

	fn resolve_with_promotions(&self, types: &mut TypeInterner, implementations: &[Signature], call_argument: TypeId, inputs: &[TypeId]) -> Result<Selection, String> {
		let mut candidates: Vec<(usize, Vec<Promotion>)> = Vec::new();
		'implementations: for (index, signature) in implementations.iter().enumerate() {
			if !signature.generics.is_empty() || !types.valid(signature.call_argument, call_argument) || inputs.len() != signature.inputs.len() {
				continue;
			}
			let mut promotions = Vec::new();
			for (input, (&provided, &expected)) in inputs.iter().zip(&signature.inputs).enumerate() {
				if types.valid(provided, expected) {
					continue;
				}
				let Some((kind, element)) = promotion_between(types, provided, expected) else {
					continue 'implementations;
				};
				let adapter = self.promotion_adapters.get(&(kind, element)).copied().ok_or("No promotion adapter node")?;
				promotions.push(Promotion { input: input as u32, adapter });
			}
			candidates.push((index, promotions));
		}

		candidates.sort_by_key(|(_, promotions)| promotions.len());
		let minimum = candidates.first().map(|(_, promotions)| promotions.len());
		let tied = candidates.iter().take_while(|(_, promotions)| Some(promotions.len()) == minimum).count();
		match tied {
			0 => return self.resolve_with_coercions(types, implementations, call_argument, inputs),
			1 => {}
			_ => return Err("Invalid implementations".to_string()),
		}

		let (index, promotions) = candidates.swap_remove(0);
		let signature = &implementations[index];
		Ok(Selection {
			implementation: index as u32,
			return_type: signature.output,
			fn_type: types.function(signature.call_argument, signature.output),
			promotions,
			conversions: Vec::new(),
		})
	}
}

/// Resolves the signature's generics against the provided types like `check_generic` and `replace_generics`, returning the substituted call argument and output.
fn substitute_generics(types: &mut TypeInterner, signature: &Signature, call_argument: TypeId, inputs: &[TypeId]) -> Option<(TypeId, TypeId)> {
	let mut lookup: Vec<(u32, TypeId)> = Vec::with_capacity(signature.generics.len());
	for &generic in &signature.generics {
		let pairs = [(Some(signature.call_argument), Some(call_argument))]
			.into_iter()
			.chain(signature.inputs.iter().map(|&input| types.fn_input(input)).zip(inputs.iter().map(|&input| types.fn_input(input))))
			.chain(signature.inputs.iter().map(|&input| types.fn_output(input)).zip(inputs.iter().map(|&input| types.fn_output(input))));
		let mut outputs = pairs
			.filter(|(expected, _)| expected.and_then(|ty| types.generic_name(ty)) == Some(generic))
			.filter_map(|(_, provided)| provided);
		let first = outputs.next()?;
		if outputs.any(|ty| ty != first) {
			return None;
		}
		lookup.push((generic, first));
	}

	let replace = |name: u32| lookup.iter().find(|(generic, _)| *generic == name).map(|(_, ty)| *ty);
	Some((types.replace_nested(signature.call_argument, &replace), types.replace_nested(signature.output, &replace)))
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
