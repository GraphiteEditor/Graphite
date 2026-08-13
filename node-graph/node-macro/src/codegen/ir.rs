//! The intent IR: a node built from its signature, from which lowering derives.
#![allow(dead_code)]

use crate::codegen::classify::{Dialect, context_param, dialect};
use crate::parsing::ParsedNodeFn;
use syn::{GenericParam, Ident, Type, TypeParamBound};

pub(crate) fn build(parsed: &ParsedNodeFn) -> Node {
	Node {
		kernel: Kernel { fn_name: parsed.fn_name.clone() },
		generics: generics(parsed),
		monomorphizations: Vec::new(),
		inputs: Vec::new(),
		output: Output {
			shape: ItemShape {
				element: Element::Concrete(parsed.output_type.clone()),
				depth: 0,
				attrs: Vec::new(),
			},
			removes: Vec::new(),
		},
		effect: effect(parsed),
	}
}

fn generics(parsed: &ParsedNodeFn) -> Vec<Generic> {
	let ctx = context_param(parsed).map(|param| param.ident.clone());
	parsed
		.fn_generics
		.iter()
		.filter_map(|param| match param {
			GenericParam::Type(param) if Some(&param.ident) != ctx.as_ref() => Some(Generic {
				ident: param.ident.clone(),
				bounds: param.bounds.iter().cloned().collect(),
			}),
			_ => None,
		})
		.collect()
}

fn effect(parsed: &ParsedNodeFn) -> Effect {
	match dialect(parsed) {
		Dialect::Sync => Effect::Pure,
		Dialect::Interrupt => Effect::Fallible,
		Dialect::Poll => Effect::Progressive,
		Dialect::AsyncFn | Dialect::Future | Dialect::FutureInterrupt => Effect::AsyncSource,
	}
}

pub(crate) struct Node {
	pub(crate) kernel: Kernel,
	pub(crate) generics: Vec<Generic>,
	/// Correlated rows (zipped `#[implementations]`, not crossed); empty = erased.
	pub(crate) monomorphizations: Vec<ImplRow>,
	pub(crate) inputs: Vec<Input>,
	pub(crate) output: Output,
	pub(crate) effect: Effect,
}

/// The kernel fn the node wraps.
pub(crate) struct Kernel {
	pub(crate) fn_name: Ident,
}

pub(crate) struct Generic {
	pub(crate) ident: Ident,
	pub(crate) bounds: Vec<TypeParamBound>,
}

/// One monomorphization: a concrete type per monomorphized generic.
pub(crate) struct ImplRow {
	pub(crate) assignments: Vec<(Ident, Type)>,
}

pub(crate) struct Input {
	pub(crate) ident: Ident,
	pub(crate) evaluation: Evaluation,
	pub(crate) content: Content,
}

/// `Lazy` = `impl Node<..>`, the kernel drives it.
pub(crate) enum Evaluation {
	Eager,
	Lazy,
}

pub(crate) enum Content {
	/// Configuration; not an item.
	Value(Type),
	/// `subject` = its layout (attrs + rank) flows to the output.
	Item { subject: bool, shape: ItemShape },
}

pub(crate) struct Output {
	pub(crate) shape: ItemShape,
	pub(crate) removes: Vec<LevelAttr>,
}

/// An item's ranked layout; `attrs` are reads on an input, writes on the output.
pub(crate) struct ItemShape {
	pub(crate) element: Element,
	pub(crate) depth: u8,
	pub(crate) attrs: Vec<LevelAttr>,
}

pub(crate) enum Element {
	Concrete(Type),
	/// Indexes [`Node::generics`].
	Generic(Ident),
}

/// An attribute at a nesting level; `0` = innermost (the element's level).
pub(crate) struct LevelAttr {
	pub(crate) marker: Type,
	pub(crate) level: u8,
}

pub(crate) enum Effect {
	Pure,
	Fallible,
	Progressive,
	AsyncSource,
}
