//! A port of `graph_craft::proto::TypingContext`.

use super::NodeId;
use super::executor::NodeConstructor;
use super::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};
use super::types::{NodeIOTypes, Type};
use std::borrow::Cow;
use std::collections::HashMap;

pub type Registry = HashMap<Cow<'static, str>, HashMap<NodeIOTypes, NodeConstructor>>;

fn valid_type(from: &Type, to: &Type) -> bool {
	match (from, to) {
		(Type::Concrete(type1), Type::Concrete(type2)) => type1 == type2,
		(Type::Fn(in1, out1), Type::Fn(in2, out2)) => valid_type(out2, out1) && valid_type(in1, in2),
		(Type::Item(element1), Type::Item(element2)) => valid_type(element1, element2),
		(Type::List(element1), Type::List(element2)) => valid_type(element1, element2),
		(Type::Generic(_), _) | (_, Type::Generic(_)) => true,
		_ => false,
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum Promotion {
	ItemToList(Type),
	Bundle(Type),
	Unbundle(Type),
}

type PromotionVariant = fn(Type) -> Promotion;

impl Promotion {
	fn between(from: &Type, to: &Type) -> Option<Self> {
		Self::variant_between(from, to).map(|(variant, element)| variant(element.clone()))
	}

	fn variant_between<'a>(from: &'a Type, to: &'a Type) -> Option<(PromotionVariant, &'a Type)> {
		fn concrete_name(ty: &Type) -> Option<&str> {
			match ty {
				Type::Concrete(descriptor) => Some(&descriptor.name),
				_ => None,
			}
		}

		match (from, to) {
			(Type::Item(from_element), Type::List(element)) if valid_type(from_element, element) => Some((Self::ItemToList, element)),
			(Type::List(element), Type::Item(_)) if to.bundle_element_name().is_some_and(|name| Some(name) == concrete_name(element)) => Some((Self::Bundle, element)),
			(Type::Item(_), Type::List(element)) if from.bundle_element_name().is_some_and(|name| Some(name) == concrete_name(element)) => Some((Self::Unbundle, element)),
			_ => None,
		}
	}

	pub fn adapter_identifier(&self) -> String {
		let (adapter_name, element) = match self {
			Self::ItemToList(element) => ("graphene_core::ops::ItemToListNode", element),
			Self::Bundle(element) => ("graphene_core::ops::BundleNode", element),
			Self::Unbundle(element) => ("graphene_core::ops::UnbundleNode", element),
		};
		format!("{adapter_name}<{}>", element.identifier_name())
	}
}

pub struct TypingContext<'a> {
	lookup: &'a Registry,
	inferred: HashMap<NodeId, NodeIOTypes>,
	constructor: HashMap<NodeId, NodeConstructor>,
	promotions: HashMap<NodeId, Vec<(usize, Promotion)>>,
}

impl<'a> TypingContext<'a> {
	pub fn new(lookup: &'a Registry) -> Self {
		Self {
			lookup,
			inferred: HashMap::new(),
			constructor: HashMap::new(),
			promotions: HashMap::new(),
		}
	}

	pub fn update(&mut self, network: &ProtoNetwork) -> Result<(), String> {
		for (id, node) in network.nodes.iter() {
			self.infer(*id, node)?;
		}
		Ok(())
	}

	pub fn promotions(&self, node_id: NodeId) -> Option<&Vec<(usize, Promotion)>> {
		self.promotions.get(&node_id)
	}

	pub fn adapter_constructor(&self, identifier: &str) -> Option<NodeConstructor> {
		self.lookup.get(identifier).and_then(|implementations| implementations.values().next().copied())
	}

	pub fn constructor(&self, node_id: NodeId) -> Option<NodeConstructor> {
		self.constructor.get(&node_id).copied()
	}

	pub fn type_of(&self, node_id: NodeId) -> Option<&NodeIOTypes> {
		self.inferred.get(&node_id)
	}

	pub fn infer(&mut self, node_id: NodeId, node: &ProtoNode) -> Result<NodeIOTypes, String> {
		if let Some(inferred) = self.inferred.get(&node_id) {
			return Ok(inferred.clone());
		}

		let inputs = match node.construction_args {
			ConstructionArgs::Value(ref value) => {
				let types = NodeIOTypes::new(Type::context(), value.ty(), vec![]);
				self.inferred.insert(node_id, types.clone());
				return Ok(types);
			}
			ConstructionArgs::Nodes(ref nodes) => nodes
				.iter()
				.map(|id| {
					self.inferred
						.get(id)
						.ok_or_else(|| format!("Input node {id:?} is not present in the typing context"))
						.map(|node| node.ty())
				})
				.collect::<Result<Vec<Type>, String>>()?,
			ConstructionArgs::Inline => return Err("Inline nodes are not supported".to_string()),
		};

		let call_argument = &node.call_argument;
		let implementations = self.lookup.get(&node.identifier).ok_or_else(|| format!("No implementations found for {}", node.identifier))?;

		if let Some(index) = inputs.iter().position(|p| matches!(p, Type::Fn(_, b) if matches!(b.as_ref(), Type::Generic(_)))) {
			return Err(format!("Generic inputs should not exist but found at {index}"));
		}

		let valid_output_types = implementations
			.keys()
			.filter(|node_io| valid_type(&node_io.call_argument, call_argument) && inputs.iter().zip(node_io.inputs.iter()).all(|(p1, p2)| valid_type(p1, p2)))
			.collect::<Vec<_>>();

		let substitution_results = valid_output_types
			.iter()
			.map(|node_io| {
				let generics_lookup: Result<HashMap<_, _>, _> = collect_generics(node_io)
					.iter()
					.map(|generic| check_generic(node_io, call_argument, &inputs, generic).map(|x| (generic.to_string(), x)))
					.collect();

				generics_lookup.map(|generics_lookup| {
					let original_node_io = (*node_io).clone();
					let mut new_node_io = original_node_io.clone();
					replace_generics(&mut new_node_io, &generics_lookup);
					(new_node_io, original_node_io)
				})
			})
			.collect::<Vec<_>>();

		let valid_implementations = substitution_results.iter().filter_map(|result| result.as_ref().ok()).collect::<Vec<_>>();

		match valid_implementations.as_slice() {
			[] => {
				fn promotable_adapter(from: &Type, to: &Type) -> Option<Promotion> {
					let (Type::Fn(from_input, from_output), Type::Fn(to_input, to_output)) = (from, to) else {
						return None;
					};
					if !valid_type(from_input, to_input) {
						return None;
					}
					Promotion::between(from_output.nested_type(), to_output.nested_type())
				}

				let mut promotable_matches = implementations
					.keys()
					.filter(|node_io| collect_generics(node_io).is_empty() && valid_type(&node_io.call_argument, call_argument) && inputs.len() == node_io.inputs.len())
					.filter_map(|node_io| {
						let mut required_promotions = Vec::new();
						for (index, (provided, expected)) in inputs.iter().zip(node_io.inputs.iter()).enumerate() {
							if valid_type(provided, expected) {
								continue;
							}
							required_promotions.push((index, promotable_adapter(provided, expected)?));
						}
						Some((node_io, required_promotions))
					})
					.collect::<Vec<_>>();

				promotable_matches.sort_by_key(|(_, required_promotions)| required_promotions.len());
				let minimum_promotions = promotable_matches.first().map(|(_, required_promotions)| required_promotions.len());
				let tied_at_minimum = promotable_matches
					.iter()
					.take_while(|(_, required_promotions)| Some(required_promotions.len()) == minimum_promotions)
					.count();

				if tied_at_minimum == 1 {
					let (node_io, required_promotions) = promotable_matches.remove(0);
					let node_io = node_io.clone();

					self.inferred.insert(node_id, node_io.clone());
					self.constructor.insert(node_id, implementations[&node_io]);
					self.promotions.insert(node_id, required_promotions);
					return Ok(node_io);
				}

				Err(format!("Invalid implementations for {} with inputs {inputs:?}", node.identifier))
			}
			[(node_io, original_node_io)] => {
				let node_io = node_io.clone();
				self.inferred.insert(node_id, node_io.clone());
				self.constructor.insert(node_id, implementations[original_node_io]);
				Ok(node_io)
			}
			[first, second] => {
				if first.0.call_argument != second.0.call_argument {
					for (node_io, original_node_io) in [first, second] {
						if node_io.call_argument != Type::unit() {
							continue;
						}
						self.inferred.insert(node_id, node_io.clone());
						self.constructor.insert(node_id, implementations[original_node_io]);
						return Ok(node_io.clone());
					}
				}
				Err(format!("Multiple implementations found for {}", node.identifier))
			}
			_ => Err(format!("Multiple implementations found for {}", node.identifier)),
		}
	}
}

fn collect_generics(types: &NodeIOTypes) -> Vec<Cow<'static, str>> {
	let inputs = [&types.call_argument].into_iter().chain(types.inputs.iter().map(|x| x.nested_type()));
	let mut generics = inputs
		.filter_map(|t| match t {
			Type::Generic(out) => Some(out.clone()),
			_ => None,
		})
		.collect::<Vec<_>>();
	if let Type::Generic(out) = &types.return_value {
		generics.push(out.clone());
	}
	generics.dedup();
	generics
}

fn check_generic(types: &NodeIOTypes, input: &Type, parameters: &[Type], generic: &str) -> Result<Type, String> {
	let inputs = [(Some(&types.call_argument), Some(input))]
		.into_iter()
		.chain(types.inputs.iter().map(|x| x.fn_input()).zip(parameters.iter().map(|x| x.fn_input())))
		.chain(types.inputs.iter().map(|x| x.fn_output()).zip(parameters.iter().map(|x| x.fn_output())));
	let concrete_inputs = inputs.filter(|(node_input, _)| matches!(node_input, Some(Type::Generic(input)) if generic == input));
	let mut outputs = concrete_inputs.flat_map(|(_, out)| out);
	let out_ty = outputs
		.next()
		.ok_or_else(|| format!("Generic output type {generic} is not dependent on input {input:?} or parameters {parameters:?}"))?;
	if outputs.any(|ty| ty != out_ty) {
		return Err(format!("Generic output type {generic} is dependent on multiple inputs or parameters"));
	}
	Ok(out_ty.clone())
}

fn replace_generics(types: &mut NodeIOTypes, lookup: &HashMap<String, Type>) {
	let replace = |ty: &Type| {
		let Type::Generic(ident) = ty else { return None };
		lookup.get(ident.as_ref()).cloned()
	};
	types.call_argument.replace_nested(replace);
	types.return_value.replace_nested(replace);
	for input in &mut types.inputs {
		input.replace_nested(replace);
	}
}
