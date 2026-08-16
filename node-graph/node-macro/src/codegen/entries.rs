use super::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{GenericParam, Ident, Type};

pub(crate) fn entries_tokens(parsed: &ParsedNodeFn, struct_name: &Ident, data_field_generic_idents: &[Ident], regular_fields: &[&ParsedField]) -> TokenStream2 {
	if !data_field_generic_idents.is_empty() {
		return quote!();
	}
	match crate::codegen::ir::node_kind(&crate::codegen::ir::build(parsed)) {
		crate::codegen::ir::NodeKind::Flip => flip_entries_tokens(parsed, struct_name, regular_fields),
		_ => single_row_entries(parsed, struct_name, regular_fields),
	}
}

/// The registry rows of a flipped plain node: every wire is a record wire,
/// inputs resolve their layouts off the claimed handles, and the output is an
/// element-only record of the kernel's return type.
fn flip_entries_tokens(parsed: &ParsedNodeFn, struct_name: &Ident, regular_fields: &[&ParsedField]) -> TokenStream2 {
	let Some(rows) = implementation_rows(parsed, regular_fields) else {
		return quote!();
	};
	let rows: Vec<&Vec<Type>> = rows.iter().filter(|row| row.iter().all(|ty| !type_disqualifies(ty))).collect();
	if rows.is_empty() {
		return quote!();
	}
	let output = slot_value_type(&parsed.output_type);

	let field_type = |field: &ParsedField| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty.clone(),
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type.clone(),
	};
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	let generic_positions: Option<Vec<(Ident, usize)>> = parsed
		.fn_generics
		.iter()
		.filter_map(|param| match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref() => Some(&type_param.ident),
			_ => None,
		})
		.map(|generic| {
			regular_fields
				.iter()
				.position(|field| generic_extractable(&field_type(field), generic))
				.map(|index| (generic.clone(), index))
		})
		.collect();
	let Some(generic_positions) = generic_positions else {
		return quote!();
	};

	let fn_name = &parsed.fn_name;
	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();
	let node_underscores: Vec<TokenStream2> = regular_fields.iter().map(|_| quote!(_)).collect();
	let node = crate::codegen::ir::build(parsed);
	let core_types = quote!(gcore);

	// Shorthand associated types in the output only resolve against the
	// generics' bounds, so rows name the output through a bounded alias. Only
	// output-reaching generics (directly or through a kept bound) may appear:
	// an unused alias parameter is an error.
	let candidate_params: Vec<&GenericParam> = parsed
		.fn_generics
		.iter()
		.filter(|param| matches!(param, GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref()))
		.collect();
	let param_ident = |param: &&GenericParam| match param {
		GenericParam::Type(type_param) => type_param.ident.clone(),
		_ => unreachable!("candidates are type parameters"),
	};
	let mut kept: Vec<bool> = candidate_params.iter().map(|param| type_contains_ident(&output, &param_ident(param))).collect();
	loop {
		let mut grew = false;
		for index in 0..candidate_params.len() {
			if kept[index] {
				continue;
			}
			let ident = param_ident(&candidate_params[index]);
			let mentioned = candidate_params.iter().zip(&kept).any(|(param, kept)| {
				*kept
					&& match param {
						GenericParam::Type(type_param) => type_param.bounds.iter().any(|bound| {
							let bound: Type = syn::parse_quote!(dyn #bound);
							type_contains_ident(&bound, &ident)
						}),
						_ => false,
					}
			});
			if mentioned {
				kept[index] = true;
				grew = true;
			}
		}
		if !grew {
			break;
		}
	}
	let alias_params: Vec<&GenericParam> = candidate_params.iter().zip(&kept).filter(|(_, kept)| **kept).map(|(param, _)| *param).collect();
	let alias_param_idents: Vec<Ident> = alias_params.iter().map(|param| param_ident(param)).collect();
	let alias_param_tokens: Vec<TokenStream2> = alias_params.iter().map(|param| quote!(#param)).collect();
	let output_alias = format_ident!("__{}_output", fn_name);
	let alias_def = match alias_param_tokens.is_empty() {
		true => quote!(#[allow(non_camel_case_types)] type #output_alias = #output;),
		false => quote!(#[allow(non_camel_case_types, type_alias_bounds)] type #output_alias<#(#alias_param_tokens,)*> = #output;),
	};

	let entries = rows.iter().filter_map(|row| {
		let assignments: Vec<(Ident, Type)> = generic_positions
			.iter()
			.map(|(generic, index)| generic_assignment(&field_type(regular_fields[*index]), &row[*index], generic).map(|assigned| (generic.clone(), assigned)))
			.collect::<Option<_>>()?;
		if type_disqualifies(&substitute_ident_types(&output, &assignments)) {
			return None;
		}
		let assignment_types: Vec<TokenStream2> = assignments.iter().map(|(_, ty)| quote!(#ty)).collect();
		let alias_arguments: Vec<TokenStream2> = assignments
			.iter()
			.filter(|(generic, _)| alias_param_idents.contains(generic))
			.map(|(_, ty)| quote!(#ty))
			.collect();
		let row_output = match alias_arguments.is_empty() {
			true => quote!(#output_alias),
			false => quote!(#output_alias<#(#alias_arguments),*>),
		};
		let assignment_types = assignment_types.iter();
		let turbofish = quote!(::<#(#node_underscores,)* #(#assignment_types,)*>);
		let input_types = row.iter().map(|ty| quote!(gcore::registry::record_edge_type::<#ty>()));
		let downcasts = names.iter().zip(row.iter()).enumerate().map(|(index, (name, ty))| {
			let handle = format_ident!("__handle_{index}");
			let layout = format_ident!("__layout_{index}");
			quote! {
				let #handle = inputs.next().unwrap();
				let #layout = #handle.layout().clone();
				let #name = #handle.downcast_record::<#ty>()?;
			}
		});
		let layout_args = (0..arity).map(|index| {
			let layout = format_ident!("__layout_{index}");
			quote!(&#layout,)
		});
		let element_spec = quote!(gcore::record::ElementSpec::Concrete(gcore::record::element_write::<#row_output>()));
		let layout_meta = crate::codegen::ir::layout_meta_tokens(&node, element_spec, &core_types);
		Some(quote! {
			gcore::registry::RegistryEntry {
				layout_meta: Some(#layout_meta),
				io: gcore::registry::NodeIOTypes::new(
					gcore::concrete!(gcore::context::ContextImpl<'static>),
					gcore::registry::record_type::<#row_output>(),
					vec![#(#input_types),*],
				),
				constructor: |inputs| {
					if inputs.len() != #arity {
						return Err(gcore::registry::ConstructionError::Arity { expected: #arity, got: inputs.len() });
					}
					let mut inputs = inputs.into_iter();
					#(#downcasts)*
					let __node = #struct_name #turbofish::new(#(#names,)* #(#layout_args)*);
					Ok(gcore::registry::EdgeHandle::new_record::<#row_output>(::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>))
				},
			}
		})
	});
	let entries: Vec<TokenStream2> = entries.collect();
	if entries.is_empty() {
		return quote!();
	}

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			#alias_def
			vec![#(#entries),*]
		}
	}
}

/// Which record wire an input claims and how its value is recovered. Base slots
/// are the record edges whose layouts form the output; value slots are record
/// edges read for their layout; plain and lazy slots are ordinary edges.
enum SlotKind {
	/// A generic record edge whose element is only known at runtime; the runtime
	/// type is captured for the output wrap or the union.
	BaseGeneric(String),
	/// A concrete record carrier read for its layout.
	BaseConcrete(Type),
	/// A concrete record edge read for its layout only.
	Value(Type),
	/// A record edge whose element extracts to the node's plain value input.
	Extracted(Type),
	/// A plain value edge.
	Plain(Type),
	/// A lazy node edge.
	Lazy(Type),
}

impl SlotKind {
	fn is_base(&self) -> bool {
		matches!(self, SlotKind::BaseGeneric(_) | SlotKind::BaseConcrete(_))
	}
}

/// The single registry row shared by record-io, routing, and opaque nodes: one
/// instance covers the wire, each input's edge type and downcast follow its
/// slot, and the output layout folds from the base slots.
fn single_row_entries(parsed: &ParsedNodeFn, struct_name: &Ident, regular_fields: &[&ParsedField]) -> TokenStream2 {
	use crate::codegen::ir;
	let fn_name = &parsed.fn_name;
	let node = ir::build(parsed);
	let core_types = quote!(gcore);
	let lend = |field: &ParsedField| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }));

	// A subject is its record edge (concrete carrier or erased generic); a
	// non-subject value rides a record edge when it reads its layout, else plain.
	let slots: Vec<SlotKind> = regular_fields
		.iter()
		.enumerate()
		.map(|(index, field)| {
			let input = &node.inputs[index];
			if input.subject {
				return match &input.shape.element {
					ir::Element::Concrete(ty) => SlotKind::BaseConcrete(ty.clone()),
					ir::Element::Generic(ident) => SlotKind::BaseGeneric(ident.to_string()),
					ir::Element::Opaque => SlotKind::BaseGeneric("T".to_string()),
				};
			}
			match &field.ty {
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) => SlotKind::Lazy(output_type.clone()),
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => match ir::value_binding(&node, index) {
					ir::ValueBinding::ReadingSecondary | ir::ValueBinding::RecordElement => SlotKind::Value(ty.clone()),
					// One wire kind: a record node's plain value still rides a
					// record edge, extracted to its element at construction.
					_ if matches!(ir::node_kind(&node), ir::NodeKind::RecordIo) => SlotKind::Extracted(ty.clone()),
					_ => SlotKind::Plain(ty.clone()),
				},
			}
		})
		.collect();

	// Every non-base value/plain/lazy input must be concrete.
	let values_concrete = regular_fields.iter().zip(&slots).all(|(field, slot)| match slot {
		SlotKind::BaseGeneric(_) | SlotKind::BaseConcrete(_) => true,
		SlotKind::Value(ty) | SlotKind::Extracted(ty) | SlotKind::Plain(ty) | SlotKind::Lazy(ty) => !contains_open_generic(parsed, ty) && (lend(field) || !type_disqualifies(ty)),
	});
	if !values_concrete {
		return quote!();
	}

	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();

	let input_types = slots.iter().map(|slot| match slot {
		SlotKind::BaseGeneric(name) => quote!(gcore::registry::generic_record_edge_type(#name)),
		SlotKind::BaseConcrete(ty) | SlotKind::Value(ty) | SlotKind::Extracted(ty) => quote!(gcore::registry::record_edge_type::<#ty>()),
		SlotKind::Plain(ty) | SlotKind::Lazy(ty) => quote!(gcore::registry::edge_type::<#ty>()),
	});

	let downcasts = names.iter().zip(&slots).enumerate().map(|(index, (name, slot))| {
		let handle = format_ident!("__handle_{index}");
		let layout = format_ident!("__layout_{index}");
		let ty = format_ident!("__ty_{index}");
		match slot {
			SlotKind::BaseGeneric(_) | SlotKind::BaseConcrete(_) => quote! {
				let #handle = inputs.next().unwrap();
				let #ty = #handle.ty().clone();
				let #layout = #handle.layout().clone();
				let #name = #handle.downcast_erased::<gcore::registry::ErasedRecordNode>(#ty.clone())?;
			},
			SlotKind::Value(value_ty) => quote! {
				let #handle = inputs.next().unwrap();
				let #layout = #handle.layout().clone();
				let #name = #handle.downcast_record::<#value_ty>()?;
			},
			SlotKind::Extracted(value_ty) => quote! {
				let #handle = inputs.next().unwrap();
				let #layout = #handle.layout().clone();
				let #name = gcore::record::RecordExtract::<#value_ty, _>::new(#handle.downcast_record::<#value_ty>()?, &#layout);
			},
			SlotKind::Plain(value_ty) | SlotKind::Lazy(value_ty) => quote!(let #name = inputs.next().unwrap().downcast::<#value_ty>()?;),
		}
	});

	let base_indices: Vec<usize> = slots.iter().enumerate().filter(|(_, slot)| slot.is_base()).map(|(index, _)| index).collect();
	let value_indices: Vec<usize> = slots.iter().enumerate().filter(|(_, slot)| matches!(slot, SlotKind::Value(_))).map(|(index, _)| index).collect();
	let value_layout_args: Vec<TokenStream2> = value_indices
		.iter()
		.map(|index| {
			let layout = format_ident!("__layout_{index}");
			quote!(&#layout,)
		})
		.collect();

	let carried_meta = || {
		let meta = ir::layout_meta_tokens(&node, quote!(gcore::record::ElementSpec::Carried), &core_types);
		quote!(Some(#meta))
	};

	// The output wire and node wrap follow the output element: a concrete element
	// is a typed record; a generic or opaque element is an erased record carrying
	// the first base slot's runtime type.
	let (io_output, wrap) = match &node.output.shape.element {
		ir::Element::Concrete(element) => (
			quote!(gcore::registry::record_type::<#element>()),
			quote!(Ok(gcore::registry::EdgeHandle::new_record::<#element>(::std::sync::Arc::new(__node)))),
		),
		element => {
			let name = match element {
				ir::Element::Generic(ident) => ident.to_string(),
				_ => "T".to_string(),
			};
			let base_ty = format_ident!("__ty_{}", base_indices[0]);
			(
				quote!(gcore::Type::Record(Box::new(gcore::Type::Generic(::std::borrow::Cow::Borrowed(#name))))),
				quote!(Ok(gcore::registry::EdgeHandle::new_erased(::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>, #base_ty))),
			)
		}
	};

	let (prelude, new_layout_args, layout_meta) = match ir::node_kind(&node) {
		ir::NodeKind::RecordIo => {
			let carrier_arg = node.inputs.first().is_some_and(|input| input.subject).then(|| quote!(&__layout_0,));
			let layout_meta_fn = format_ident!("{}_layout_meta", fn_name);
			(quote!(), quote!(#carrier_arg #(#value_layout_args)*), quote!(Some(self::#layout_meta_fn())))
		}
		ir::NodeKind::Routing => {
			let source_layouts = base_indices.iter().map(|index| format_ident!("__layout_{index}"));
			let source_wraps = base_indices.iter().map(|index| {
				let name = names[*index];
				let layout = format_ident!("__layout_{index}");
				quote!(let #name = gcore::record::RecordSource::new(#name, &#layout, &__union);)
			});
			let prelude = quote! {
				let __union = gcore::record::Layout::union(&[#(&#source_layouts),*]);
				#(#source_wraps)*
			};
			(prelude, quote!(&__union, #(#value_layout_args)*), carried_meta())
		}
		ir::NodeKind::Opaque => {
			let record_layout = format_ident!("__layout_{}", base_indices[0]);
			(quote!(), quote!(&#record_layout), carried_meta())
		}
		ir::NodeKind::Flip => unreachable!("flip has its own multi-row emitter"),
	};

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			vec![gcore::registry::RegistryEntry {
				layout_meta: #layout_meta,
				io: gcore::registry::NodeIOTypes::new(
					gcore::concrete!(gcore::context::ContextImpl<'static>),
					#io_output,
					vec![#(#input_types),*],
				),
				constructor: |inputs| {
					if inputs.len() != #arity {
						return Err(gcore::registry::ConstructionError::Arity { expected: #arity, got: inputs.len() });
					}
					let mut inputs = inputs.into_iter();
					#(#downcasts)*
					#prelude
					let __node = #struct_name::new(#(#names,)* #new_layout_args);
					#wrap
				},
			}]
		}
	}
}

pub(crate) fn implementation_rows(parsed: &ParsedNodeFn, regular_fields: &[&ParsedField]) -> Option<Vec<Vec<Type>>> {
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	let open_generics: Vec<&Ident> = parsed
		.fn_generics
		.iter()
		.filter_map(|param| match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref() => Some(&type_param.ident),
			_ => None,
		})
		.collect();

	let candidates: Vec<Vec<Type>> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. }) => match implementations.is_empty() {
				false => Some(implementations.iter().cloned().collect()),
				true => open_generics.iter().all(|generic| !crate::codegen::type_contains_ident(ty, generic)).then(|| vec![ty.clone()]),
			},
			ParsedFieldType::Node(NodeParsedField { output_type, implementations, .. }) => match implementations.is_empty() {
				false => Some(implementations.iter().map(|implementation| implementation.output.clone()).collect()),
				true => open_generics
					.iter()
					.all(|generic| !crate::codegen::type_contains_ident(output_type, generic))
					.then(|| vec![output_type.clone()]),
			},
		})
		.collect::<Option<_>>()?;

	let row_count = candidates.iter().map(|types| types.len()).max().unwrap_or(1).max(1);
	Some((0..row_count).map(|row| candidates.iter().map(|types| types[row.min(types.len() - 1)].clone()).collect()).collect())
}
