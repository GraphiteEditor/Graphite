//! The intent IR: a node built from its signature, from which lowering derives.
#![allow(dead_code)]

use crate::codegen::classify::{
	Dialect, RoutingIo, bare_ident, context_param, dialect, flip_carrier, generic_assignment, generic_extractable, is_record_value, record_shape, routing_io, slot_value_type,
};
use crate::codegen::entries::implementation_rows;
use crate::parsing::{AttributeRead, NodeParsedField, ParsedField, ParsedFieldType, ParsedNodeFn, RecordWrites, RegularParsedField, record_writes};
use syn::{GenericArgument, GenericParam, Ident, PathArguments, Type, TypeParamBound};

pub(crate) fn build(parsed: &ParsedNodeFn) -> Node {
	let generics = generics(parsed);
	let generic_idents: Vec<Ident> = generics.iter().map(|generic| generic.ident.clone()).collect();
	let fields: Vec<&ParsedField> = parsed.fields.iter().filter(|field| !field.is_data_field).collect();
	Node {
		kernel: Kernel { fn_name: parsed.fn_name.clone() },
		monomorphizations: monomorphizations(parsed, &fields, &generic_idents),
		inputs: inputs(parsed, &fields, &generic_idents),
		output: output(parsed, &generic_idents),
		generics,
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

fn inputs(parsed: &ParsedNodeFn, fields: &[&ParsedField], generics: &[Ident]) -> Vec<Input> {
	let routing = routing_io(parsed);
	let carrier_subject = flip_carrier(parsed) || record_shape(parsed).map_or(false, |shape| !shape.skips_carrier());
	fields
		.iter()
		.enumerate()
		.map(|(index, &field)| {
			let evaluation = match &field.ty {
				ParsedFieldType::Node(_) => Evaluation::Lazy,
				ParsedFieldType::Regular(_) => Evaluation::Eager,
			};
			Input {
				ident: field.pat_ident.ident.clone(),
				evaluation,
				shape: item_shape(field_element_type(field), &field.attribute_reads, generics),
				subject: subject(index, field, carrier_subject, routing.as_ref()),
			}
		})
		.collect()
}

fn subject(index: usize, field: &ParsedField, carrier_subject: bool, routing: Option<&RoutingIo>) -> bool {
	match &field.ty {
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => is_record_value(output_type) || routing.is_some_and(|routing| bare_ident(output_type) == Some(&routing.generic)),
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => routing.is_some_and(|routing| bare_ident(ty) == Some(&routing.generic)) || (index == 0 && carrier_subject),
	}
}

fn output(parsed: &ParsedNodeFn, generics: &[Ident]) -> Output {
	let value = slot_value_type(&parsed.output_type);
	let (element, writes, removes) = match record_writes(&value) {
		Some(RecordWrites { element, markers, removes }) => (element, markers, removes),
		None => (value, Vec::new(), Vec::new()),
	};
	let (element, depth) = strip_ilist(&element);
	Output {
		shape: ItemShape {
			element: element_of(&element, generics),
			depth,
			attrs: writes.into_iter().map(|marker| LevelAttr { marker, level: 0 }).collect(),
		},
		removes: removes.into_iter().map(|marker| LevelAttr { marker, level: 0 }).collect(),
	}
}

fn monomorphizations(parsed: &ParsedNodeFn, fields: &[&ParsedField], generics: &[Ident]) -> Vec<ImplRow> {
	if generics.is_empty() {
		return Vec::new();
	}
	let Some(rows) = implementation_rows(parsed, fields) else {
		return Vec::new();
	};
	let positions: Option<Vec<(Ident, usize)>> = generics
		.iter()
		.map(|generic| fields.iter().position(|&field| generic_extractable(field_element_type(field), generic)).map(|index| (generic.clone(), index)))
		.collect();
	let Some(positions) = positions else {
		return Vec::new();
	};
	rows.iter()
		.filter_map(|row| {
			let assignments = positions
				.iter()
				.map(|(generic, index)| generic_assignment(field_element_type(fields[*index]), &row[*index], generic).map(|ty| (generic.clone(), ty)))
				.collect::<Option<Vec<_>>>()?;
			Some(ImplRow { assignments })
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

fn field_element_type(field: &ParsedField) -> &Type {
	match &field.ty {
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
	}
}

fn item_shape(element: &Type, reads: &[AttributeRead], generics: &[Ident]) -> ItemShape {
	let (element, depth) = strip_ilist(element);
	ItemShape {
		element: element_of(&element, generics),
		depth,
		attrs: reads.iter().map(|read| LevelAttr { marker: read.marker.clone(), level: 0 }).collect(),
	}
}

fn element_of(ty: &Type, generics: &[Ident]) -> Element {
	if is_record_value(ty) {
		return Element::Opaque;
	}
	match bare_ident(ty) {
		Some(ident) if generics.contains(ident) => Element::Generic(ident.clone()),
		_ => Element::Concrete(ty.clone()),
	}
}

fn strip_ilist(ty: &Type) -> (Type, u8) {
	let mut element = ty.clone();
	let mut depth = 0;
	while let Some(inner) = ilist_inner(&element) {
		element = inner;
		depth += 1;
	}
	(element, depth)
}

fn ilist_inner(ty: &Type) -> Option<Type> {
	let Type::Path(path) = ty else { return None };
	let segment = path.path.segments.last()?;
	if segment.ident != "IList" {
		return None;
	}
	let PathArguments::AngleBracketed(args) = &segment.arguments else { return None };
	args.args.iter().find_map(|arg| match arg {
		GenericArgument::Type(inner) => Some(inner.clone()),
		_ => None,
	})
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
	pub(crate) shape: ItemShape,
	/// This input's layout folds into the output.
	pub(crate) subject: bool,
}

/// `Lazy` = `impl Node<..>`, the kernel drives it.
pub(crate) enum Evaluation {
	Eager,
	Lazy,
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
	/// A whole erased record; the element type is unknown.
	Opaque,
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
