//! The intent IR: a node built from its signature, from which lowering derives.
#![allow(dead_code)]

use crate::codegen::classify::{
	Dialect, RoutingIo, bare_ident, context_param, dialect, flip_carrier, generic_assignment, generic_extractable, is_record_value, record_shape, routing_io, slot_value_type,
};
use crate::codegen::entries::implementation_rows;
use crate::parsing::{AttributeRead, NodeParsedField, ParsedField, ParsedFieldType, ParsedNodeFn, RecordWrites, RegularParsedField, record_writes};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
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
		derives: derives(parsed),
	}
}

fn derives(parsed: &ParsedNodeFn) -> bool {
	context_param(parsed).is_some_and(|ctx| {
		ctx.bounds
			.iter()
			.any(|bound| matches!(bound, TypeParamBound::Trait(trait_bound) if trait_bound.path.segments.last().is_some_and(|segment| segment.ident == "DeriveCtx")))
	})
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
				lend: matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. })),
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

/// Emits the `LayoutMeta` literal from the IR. `element_spec` is supplied by the
/// caller since it is the one row-dependent facet; the rest folds from the node.
pub(crate) fn layout_meta_tokens(node: &Node, element_spec: TokenStream2, core_types: &TokenStream2) -> TokenStream2 {
	let sources = node.inputs.iter().enumerate().filter(|(_, input)| input.subject).map(|(index, _)| index as u8);
	let reads = node.inputs.iter().enumerate().filter_map(|(index, input)| {
		(matches!(input.evaluation, Evaluation::Eager) && !input.shape.attrs.is_empty()).then(|| {
			let descs = field_writes(&input.shape.attrs, core_types);
			let index = index as u8;
			quote!(#core_types::record::InputReads { input: #index, reads: ::std::vec![#(#descs),*] })
		})
	});
	let writes = field_writes(&node.output.shape.attrs, core_types);
	let removes = node.output.removes.iter().map(|attr| {
		let marker = &attr.marker;
		let level = attr.level;
		quote!((<#marker as #core_types::attribute::Attribute>::NAME, #level))
	});
	let level_delta = level_delta(node);
	quote! {
		#core_types::record::LayoutMeta {
			sources: ::std::vec![#(#sources),*],
			reads: ::std::vec![#(#reads),*],
			element: #element_spec,
			writes: ::std::vec![#(#writes),*],
			removes: ::std::vec![#(#removes),*],
			level_delta: #level_delta,
		}
	}
}

fn field_writes(attrs: &[LevelAttr], core_types: &TokenStream2) -> Vec<TokenStream2> {
	attrs
		.iter()
		.map(|attr| {
			let marker = &attr.marker;
			let level = attr.level;
			quote!(#core_types::record::FieldWrite::of::<#marker>(#level))
		})
		.collect()
}

fn level_delta(node: &Node) -> i8 {
	let subject_depth = node.inputs.iter().find(|input| input.subject).map_or(0, |input| input.shape.depth as i8);
	node.output.shape.depth as i8 - subject_depth
}

/// How an eager value input binds in eval.
pub(crate) enum ValueBinding {
	Carrier,
	Lend,
	ReadingSecondary,
	RecordElement,
	Plain,
}

/// How a lazy (`impl Node`) input binds in eval. The `Poll` effect further
/// selects the borrowed vs `__cell`-driven form within `Element`/`Plain`.
pub(crate) enum LazyBinding {
	Element,
	Plain,
	DeriveRouting,
	OpaqueRecord,
}

impl ValueBinding {
	/// Copies an element out of a record edge, so the frame is reclaimed after.
	pub(crate) fn reads_out(&self) -> bool {
		matches!(self, ValueBinding::ReadingSecondary | ValueBinding::RecordElement)
	}
}

enum NodeKind {
	Flip,
	RecordIo,
	Routing,
	Opaque,
}

fn node_kind(node: &Node) -> NodeKind {
	if matches!(node.output.shape.element, Element::Opaque) {
		NodeKind::Opaque
	} else if has_attr_io(node) {
		NodeKind::RecordIo
	} else if is_routing(node) {
		NodeKind::Routing
	} else {
		NodeKind::Flip
	}
}

/// Routing forwards an unbounded generic from a source whole; a bounded generic
/// or one transformed into a different output type works on the element and flips.
fn is_routing(node: &Node) -> bool {
	let Element::Generic(output) = &node.output.shape.element else { return false };
	node.monomorphizations.is_empty()
		&& node.generics.iter().any(|generic| &generic.ident == output && generic.bounds.is_empty())
		&& node.inputs.iter().any(|input| input.subject && matches!(&input.shape.element, Element::Generic(generic) if generic == output))
}

fn has_attr_io(node: &Node) -> bool {
	// Reads on lazy inputs ride the flip; only eager reads make a record-io node.
	node.inputs.iter().any(|input| matches!(input.evaluation, Evaluation::Eager) && !input.shape.attrs.is_empty()) || !node.output.shape.attrs.is_empty() || !node.output.removes.is_empty()
}

pub(crate) fn value_binding(node: &Node, index: usize) -> ValueBinding {
	let input = &node.inputs[index];
	let kind = node_kind(node);
	if matches!(kind, NodeKind::RecordIo | NodeKind::Flip) && index == 0 && input.subject {
		ValueBinding::Carrier
	} else if matches!(kind, NodeKind::Flip) && input.lend {
		ValueBinding::Lend
	} else if matches!(kind, NodeKind::RecordIo) && !input.shape.attrs.is_empty() {
		ValueBinding::ReadingSecondary
	} else if matches!(kind, NodeKind::Flip) || (matches!(kind, NodeKind::Routing) && !input.subject) {
		ValueBinding::RecordElement
	} else {
		ValueBinding::Plain
	}
}

pub(crate) fn lazy_binding(node: &Node, index: usize) -> LazyBinding {
	let input = &node.inputs[index];
	let kind = node_kind(node);
	if node.derives && matches!(kind, NodeKind::Routing) && input.subject {
		LazyBinding::DeriveRouting
	} else if matches!(kind, NodeKind::Flip) {
		LazyBinding::Element
	} else if matches!(input.shape.element, Element::Opaque) {
		LazyBinding::OpaqueRecord
	} else {
		LazyBinding::Plain
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
	/// The context is derived (a `DeriveCtx` bound), so routing sources rebind it.
	pub(crate) derives: bool,
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
	/// Written `&T`; the kernel borrows the evaluated element.
	pub(crate) lend: bool,
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::codegen::classify::{Class, Dialect, analyze, context_param, dialect};
	use crate::parsing::parse_node_fn;
	use proc_macro2::TokenStream as TokenStream2;
	use quote::{ToTokens, quote};

	/// The layout facts every emitter expresses, derived from either the intent
	/// IR or the resolved class, so the two paths can be checked equal.
	#[derive(Debug, PartialEq)]
	struct Facts {
		sources: Vec<usize>,
		carried: bool,
		writes: Vec<String>,
		removes: Vec<String>,
		delta: i8,
	}

	fn markers<'a>(types: impl IntoIterator<Item = &'a Type>) -> Vec<String> {
		types.into_iter().map(|ty| ty.to_token_stream().to_string()).collect()
	}

	fn facts_from_ir(node: &Node) -> Facts {
		let carried = match &node.output.shape.element {
			Element::Opaque => true,
			Element::Generic(_) => node.monomorphizations.is_empty(),
			Element::Concrete(_) => false,
		};
		let subject_depth = node.inputs.iter().find(|input| input.subject).map_or(0, |input| input.shape.depth as i8);
		Facts {
			sources: node.inputs.iter().enumerate().filter(|(_, input)| input.subject).map(|(index, _)| index).collect(),
			carried,
			writes: markers(node.output.shape.attrs.iter().map(|attr| &attr.marker)),
			removes: markers(node.output.removes.iter().map(|attr| &attr.marker)),
			delta: node.output.shape.depth as i8 - subject_depth,
		}
	}

	fn facts_from_class(class: &Class, fields: &[&ParsedField]) -> Facts {
		let source_ty = |field: &ParsedField| match &field.ty {
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type.clone(),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty.clone(),
		};
		match class {
			Class::Flip { carrier } => Facts {
				sources: if *carrier { vec![0] } else { vec![] },
				carried: false,
				writes: vec![],
				removes: vec![],
				delta: 0,
			},
			Class::Opaque => {
				let record = fields.iter().position(|field| matches!(&field.ty, ParsedFieldType::Node(NodeParsedField { output_type, .. }) if is_record_value(output_type)));
				Facts {
					sources: record.into_iter().collect(),
					carried: true,
					writes: vec![],
					removes: vec![],
					delta: 0,
				}
			}
			Class::Routing(routing) => Facts {
				sources: fields.iter().enumerate().filter(|(_, field)| bare_ident(&source_ty(field)) == Some(&routing.generic)).map(|(index, _)| index).collect(),
				carried: true,
				writes: vec![],
				removes: vec![],
				delta: 0,
			},
			Class::RecordIo(shape) => Facts {
				sources: if shape.skips_carrier() { vec![] } else { vec![0] },
				carried: shape.carries_element(),
				writes: markers(&shape.write_markers),
				removes: markers(&shape.removes),
				delta: 0,
			},
		}
	}

	fn assert_bridge(attr: TokenStream2, item: TokenStream2) -> Node {
		let mut parsed = parse_node_fn(attr, item).unwrap();
		parsed.replace_impl_trait_in_input();
		let model = analyze(&parsed).expect("representative resolves to a class");
		let fields: Vec<&ParsedField> = parsed.fields.iter().filter(|field| !field.is_data_field).collect();
		let node = build(&parsed);
		assert_eq!(facts_from_ir(&node), facts_from_class(&model.class, &fields));
		node
	}

	#[test]
	fn bridge_flip_concrete() {
		assert_bridge(quote!(category("")), quote!(fn negate(_: impl Ctx, x: f64) -> f64 { -x }));
	}

	#[test]
	fn bridge_flip_generic() {
		assert_bridge(
			quote!(category("")),
			quote! {
				fn add<A: core::ops::Add<B>, B>(_: impl Ctx, #[implementations(f64, u32)] augend: A, #[implementations(f64, u32)] addend: B) -> <A as core::ops::Add<B>>::Output { augend + addend }
			},
		);
	}

	#[test]
	fn bridge_record_write() {
		assert_bridge(quote!(category("")), quote!(fn set_opacity(_: impl Ctx, val: f64) -> (f64, Attr<Opacity>) { (val, Attr(1.)) }));
	}

	#[test]
	fn bridge_record_remove() {
		assert_bridge(quote!(category("")), quote!(fn strip(_: impl Ctx, val: f64) -> (f64, RemoveAttr<Opacity>) { (val, RemoveAttr) }));
	}

	#[test]
	fn bridge_record_fresh() {
		assert_bridge(quote!(category("")), quote!(fn make(_: impl Ctx, _: (), fill: f64) -> (f64, Attr<Opacity>) { (fill, Attr(1.)) }));
	}

	#[test]
	fn bridge_routing() {
		assert_bridge(
			quote!(category("")),
			quote! {
				fn switch<T>(_: impl Ctx, condition: bool, off: impl Node<(), Output = T>, on: impl Node<(), Output = T>) -> T { if condition { on.eval(()) } else { off.eval(()) } }
			},
		);
	}

	#[test]
	fn bridge_opaque() {
		assert_bridge(
			quote!(category("")),
			quote! {
				fn memo<'e>(_: impl Ctx, #[data] cache: Store, content: impl Node<Context<'_>, Output = RecordValue<'e>>) -> GPoll<RecordValue<'e>> { content.eval(()) }
			},
		);
	}

	fn ctx_derives(parsed: &ParsedNodeFn) -> bool {
		context_param(parsed).is_some_and(|ctx| {
			ctx.bounds
				.iter()
				.any(|bound| matches!(bound, TypeParamBound::Trait(trait_bound) if trait_bound.path.segments.last().is_some_and(|segment| segment.ident == "DeriveCtx")))
		})
	}

	/// The frozen `field_role` classification the IR bindings must reproduce.
	fn reference_label(parsed: &ParsedNodeFn, class: &Class, raw: bool, index: usize, field: &ParsedField) -> &'static str {
		let record = matches!(class, Class::RecordIo(_));
		let skips_carrier = matches!(class, Class::RecordIo(shape) if shape.skips_carrier());
		let carrier_flip = matches!(class, Class::Flip { carrier: true });
		let flip = matches!(class, Class::Flip { .. });
		let opaque = matches!(class, Class::Opaque);
		let routing = matches!(class, Class::Routing(_));
		let derives = ctx_derives(parsed);
		let routing_source = |ty: &Type| matches!(class, Class::Routing(routing) if bare_ident(ty) == Some(&routing.generic));
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend, .. }) => {
				if record && !skips_carrier && index == 0 {
					"carrier"
				} else if carrier_flip && index == 0 {
					"carrier"
				} else if flip && lend.is_some() {
					"lend"
				} else if record && !field.attribute_reads.is_empty() {
					"reading"
				} else if flip || (routing && !routing_source(ty)) {
					"record"
				} else {
					"plain"
				}
			}
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
				if derives && routing && routing_source(output_type) {
					"derive-routing"
				} else if flip && raw {
					"flip-raw"
				} else if flip {
					"flip-lazy"
				} else if opaque && raw && is_record_value(output_type) {
					"opaque-record"
				} else if raw {
					"raw-lazy"
				} else {
					"lazy"
				}
			}
		}
	}

	fn ir_label(node: &Node, index: usize, field: &ParsedField, raw: bool) -> &'static str {
		match &field.ty {
			ParsedFieldType::Regular(_) => match value_binding(node, index) {
				ValueBinding::Carrier => "carrier",
				ValueBinding::Lend => "lend",
				ValueBinding::ReadingSecondary => "reading",
				ValueBinding::RecordElement => "record",
				ValueBinding::Plain => "plain",
			},
			ParsedFieldType::Node(_) => match (lazy_binding(node, index), raw) {
				(LazyBinding::DeriveRouting, _) => "derive-routing",
				(LazyBinding::OpaqueRecord, _) => "opaque-record",
				(LazyBinding::Element, true) => "flip-raw",
				(LazyBinding::Element, false) => "flip-lazy",
				(LazyBinding::Plain, true) => "raw-lazy",
				(LazyBinding::Plain, false) => "lazy",
			},
		}
	}

	fn assert_bindings(attr: TokenStream2, item: TokenStream2) {
		let mut parsed = parse_node_fn(attr, item).unwrap();
		parsed.replace_impl_trait_in_input();
		let model = analyze(&parsed).expect("representative resolves to a class");
		let raw = matches!(dialect(&parsed), Dialect::Poll);
		let node = build(&parsed);
		let fields: Vec<&ParsedField> = parsed.fields.iter().filter(|field| !field.is_data_field).collect();
		for (index, field) in fields.iter().enumerate() {
			assert_eq!(
				ir_label(&node, index, field, raw),
				reference_label(&parsed, &model.class, raw, index, field),
				"field {index} of {}",
				parsed.fn_name
			);
		}
	}

	#[test]
	fn bindings_flip() {
		assert_bindings(quote!(category("")), quote!(fn negate(_: impl Ctx, x: f64) -> f64 { -x }));
		assert_bindings(quote!(category("")), quote!(fn add2(_: impl Ctx, a: f64, b: f64) -> f64 { a + b }));
	}

	#[test]
	fn bindings_lend() {
		assert_bindings(quote!(category("")), quote!(fn borrow(_: impl Ctx, prim: f64, other: &f64) -> f64 { prim + *other }));
	}

	#[test]
	fn bindings_reading_secondary() {
		assert_bindings(quote!(category("")), quote!(fn read_op(_: impl Ctx, carrier: f64, (other, op): (f64, Attr<Opacity>)) -> f64 { carrier + other }));
	}

	#[test]
	fn bindings_flip_lazy() {
		assert_bindings(quote!(category("")), quote!(fn apply(_: impl Ctx, inner: impl Node<(), Output = f64>) -> f64 { inner.eval(()) }));
	}

	#[test]
	fn bindings_flip_lazy_reads() {
		assert_bindings(
			quote!(category("")),
			quote!(fn apply_reads(_: impl Ctx, carrier: f64, inner: impl Node<(), Output = (f64, Attr<Opacity>)>) -> f64 { carrier + inner.eval(()).0 }),
		);
	}

	#[test]
	fn bindings_flip_raw() {
		assert_bindings(quote!(category("")), quote!(fn poll_apply(_: impl Ctx, inner: impl Node<(), Output = f64>) -> GPoll<f64> { inner.eval(()) }));
	}

	#[test]
	fn bindings_skip_impl_generic() {
		// A bounded generic forwarded whole (passthrough) flips, not routes.
		assert_bindings(quote!(category(""), skip_impl), quote!(fn passthrough<T: Send>(_: impl Ctx, content: T) -> T { content }));
		// A generic transformed into a different output type flips.
		assert_bindings(
			quote!(category(""), skip_impl),
			quote!(fn into_ty<T: Send + Into<O>, O: Send>(_: impl Ctx, value: T, #[data] _out: PhantomData<O>) -> O { value.into() }),
		);
	}

	#[test]
	fn bindings_routing() {
		assert_bindings(
			quote!(category("")),
			quote!(fn switch<T>(_: impl Ctx, condition: bool, off: impl Node<(), Output = T>, on: impl Node<(), Output = T>) -> T { if condition { on.eval(()) } else { off.eval(()) } }),
		);
	}

	#[test]
	fn bindings_derive_routing() {
		assert_bindings(quote!(category("")), quote!(fn ctx_mod<T>(_: impl Ctx + DeriveCtx, inner: impl Node<(), Output = T>) -> T { inner.eval(()) }));
	}

	#[test]
	fn bindings_opaque() {
		assert_bindings(
			quote!(category("")),
			quote!(fn memo<'e>(_: impl Ctx, #[data] cache: Store, content: impl Node<Context<'_>, Output = RecordValue<'e>>) -> GPoll<RecordValue<'e>> { content.eval(()) }),
		);
	}

	#[test]
	fn monomorphizations_key_by_generic() {
		let node = assert_bridge(
			quote!(category("")),
			quote! {
				fn add<A: core::ops::Add<B>, B>(_: impl Ctx, #[implementations(f64, u32)] augend: A, #[implementations(f64, u32)] addend: B) -> <A as core::ops::Add<B>>::Output { augend + addend }
			},
		);
		let rows: Vec<Vec<(String, String)>> = node
			.monomorphizations
			.iter()
			.map(|row| row.assignments.iter().map(|(generic, ty)| (generic.to_string(), ty.to_token_stream().to_string())).collect())
			.collect();
		assert_eq!(
			rows,
			vec![
				vec![("A".to_string(), "f64".to_string()), ("B".to_string(), "f64".to_string())],
				vec![("A".to_string(), "u32".to_string()), ("B".to_string(), "u32".to_string())],
			]
		);
	}
}
