use super::*;
use proc_macro_error2::emit_error;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
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

/// The registry rows of a flipped plain node: every input is a record input,
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
	let output = substitute_lifetimes(&slot_value_type(&parsed.output_type), "'static");

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
	let alias_param_idents: Vec<Ident> = alias_params.iter().map(param_ident).collect();
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
		let row: Vec<Type> = row.iter().map(|ty| substitute_lifetimes(ty, "'static")).collect();
		let assignments: Vec<(Ident, Type)> = assignments.into_iter().map(|(generic, ty)| (generic, substitute_lifetimes(&ty, "'static"))).collect();
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
		let element_spec = quote!(gcore::record::ElementSpec::Concrete({ use gcore::record::{ElementWritePickHashed as _, ElementWritePickPlain as _}; (&gcore::record::ElementWritePick::<#row_output>(::core::marker::PhantomData)).element_write() }));
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

/// Which record an input claims and how its value is recovered. Base slots
/// are the record inputs whose layouts form the output; value slots are record
/// inputs read for their layout.
enum SlotKind {
	/// A generic record input whose element is only known at runtime; the runtime
	/// type is captured for the output wrap or the union.
	BaseGeneric(String),
	/// A concrete record carrier read for its layout.
	BaseConcrete(Type),
	/// A concrete record input read for its layout only.
	Value(Type),
	/// A record input whose element extracts to the node's plain value input.
	Extracted(Type),
	/// A ranked record input consumed whole; no layout rides to the constructor.
	Ranked(Type),
}

impl SlotKind {
	fn is_base(&self) -> bool {
		matches!(self, SlotKind::BaseGeneric(_) | SlotKind::BaseConcrete(_))
	}
}

/// The single registry row shared by record-io, routing, and opaque nodes: one
/// instance covers the input, each input's type and downcast follow its
/// slot, and the output layout folds from the base slots.
fn single_row_entries(parsed: &ParsedNodeFn, struct_name: &Ident, regular_fields: &[&ParsedField]) -> TokenStream2 {
	use crate::codegen::ir;
	let fn_name = &parsed.fn_name;
	let node = ir::build(parsed);
	let core_types = quote!(gcore);
	let lend = |field: &ParsedField| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }));

	// A subject is its record input (concrete carrier or erased generic); a
	// non-subject value rides a record input when it reads its layout.
	let slots: Option<Vec<SlotKind>> = regular_fields
		.iter()
		.enumerate()
		.map(|(index, field)| {
			let input = &node.inputs[index];
			if input.subject {
				return Some(match &input.shape.element {
					ir::Element::Concrete(ty) => SlotKind::BaseConcrete(ty.clone()),
					ir::Element::Generic(ident) => SlotKind::BaseGeneric(ident.to_string()),
					ir::Element::Opaque => SlotKind::BaseGeneric("T".to_string()),
				});
			}
			match &field.ty {
				// An element-consuming lazy secondary of a record node rides a
				// record input with a layout slot, like a reading secondary.
				ParsedFieldType::Node(NodeParsedField { output_type, .. })
					if matches!(ir::node_kind(&node), ir::NodeKind::RecordIo) && matches!(ir::lazy_binding(&node, index), ir::LazyBinding::Element) =>
				{
					Some(SlotKind::Value(output_type.clone()))
				}
				ParsedFieldType::Node(_) => {
					emit_error!(field.pat_ident.span(), "plain (non-record) io is unsupported: this lazy input needs a record edge");
					None
				}
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => match ir::value_binding(&node, index) {
					ir::ValueBinding::Materialized => Some(SlotKind::Ranked(ty.clone())),
					ir::ValueBinding::ReadingSecondary | ir::ValueBinding::RecordElement => Some(SlotKind::Value(ty.clone())),
					// One input kind: a record node's plain value still rides a
					// record input, extracted to its element at construction.
					_ if matches!(ir::node_kind(&node), ir::NodeKind::RecordIo) => Some(SlotKind::Extracted(ty.clone())),
					_ => {
						emit_error!(field.pat_ident.span(), "plain (non-record) io is unsupported: this value input needs a record edge");
						None
					}
				},
			}
		})
		.collect();
	let Some(slots) = slots else {
		return quote!();
	};

	// A ranked input's element generic monomorphizes the kernel, so its
	// implementations expand to one registry row each; every other slot
	// (erased routing generics included) is row-invariant. The carried list
	// mirrors the struct's carried generic parameters in declaration order.
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	let ranked_generic_idents: Vec<Ident> = parsed
		.fn_generics
		.iter()
		.filter_map(|param| match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref() => Some(type_param.ident.clone()),
			_ => None,
		})
		.filter(|ident| {
			regular_fields.iter().any(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, list_levels, .. }) => *list_levels > 0 && crate::codegen::type_contains_ident(ty, ident),
				_ => false,
			})
		})
		.collect();
	let ranked_source = |generic: &Ident| {
		regular_fields.iter().position(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, list_levels, implementations, .. }) => *list_levels > 0 && !implementations.is_empty() && generic_extractable(ty, generic),
			_ => false,
		})
	};
	let carried: Option<Vec<(Ident, usize)>> = ranked_generic_idents.iter().map(|ident| ranked_source(ident).map(|index| (ident.clone(), index))).collect();
	let Some(carried) = carried else {
		return quote!();
	};
	let impls_of = |index: usize| match &regular_fields[index].ty {
		ParsedFieldType::Regular(RegularParsedField { implementations, .. }) => implementations.iter().cloned().collect::<Vec<Type>>(),
		_ => Vec::new(),
	};
	let row_count = carried.iter().map(|(_, index)| impls_of(*index).len()).max().unwrap_or(1).max(1);
	let row_assignments: Vec<Vec<(Ident, Type)>> = (0..row_count)
		.map(|row| {
			carried
				.iter()
				.filter_map(|(generic, index)| {
					let impls = impls_of(*index);
					let row_ty = ir::strip_ilist(&impls[row.min(impls.len() - 1)]).0;
					let field_ty = match &regular_fields[*index].ty {
						ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty.clone(),
						_ => unreachable!("ranked sources are regular fields"),
					};
					generic_assignment(&field_ty, &row_ty, generic).map(|ty| (generic.clone(), ty))
				})
				.collect()
		})
		.collect();

	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();

	let entries: Vec<TokenStream2> = row_assignments
		.iter()
		.filter_map(|assignments| {
			// A row whose assignments did not all solve cannot instantiate the struct.
			if assignments.len() != carried.len() {
				return None;
			}
			let slots: Vec<SlotKind> = slots
				.iter()
				.map(|slot| match slot {
					SlotKind::BaseGeneric(name) => SlotKind::BaseGeneric(name.clone()),
					SlotKind::BaseConcrete(ty) => SlotKind::BaseConcrete(substitute_lifetimes(&substitute_ident_types(ty, assignments), "'static")),
					SlotKind::Value(ty) => SlotKind::Value(substitute_lifetimes(&substitute_ident_types(ty, assignments), "'static")),
					SlotKind::Extracted(ty) => SlotKind::Extracted(substitute_lifetimes(&substitute_ident_types(ty, assignments), "'static")),
					SlotKind::Ranked(ty) => SlotKind::Ranked(substitute_lifetimes(&substitute_ident_types(ty, assignments), "'static")),
				})
				.collect();

			// Every non-base value input must be concrete.
			let values_concrete = regular_fields.iter().zip(&slots).all(|(field, slot)| match slot {
				SlotKind::BaseGeneric(_) | SlotKind::BaseConcrete(_) => true,
				SlotKind::Value(ty) | SlotKind::Extracted(ty) | SlotKind::Ranked(ty) => !contains_open_generic(parsed, ty) && (lend(field) || !type_disqualifies(ty)),
			});
			if !values_concrete {
				return None;
			}

			let input_types = slots.iter().map(|slot| match slot {
				SlotKind::BaseGeneric(name) => quote!(gcore::registry::generic_record_edge_type(#name)),
				SlotKind::BaseConcrete(ty) | SlotKind::Value(ty) | SlotKind::Extracted(ty) | SlotKind::Ranked(ty) => quote!(gcore::registry::record_edge_type::<#ty>()),
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
					// The node reads the element off the input's own layout, so
					// neither slot rides a layout to the constructor.
					SlotKind::Extracted(value_ty) | SlotKind::Ranked(value_ty) => quote! {
						let #name = inputs.next().unwrap().downcast_record::<#value_ty>()?;
					},
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

			// The output type and node wrap follow the output element: a concrete (or
			// row-assigned) element is a typed record; a generic or opaque element is
			// an erased record carrying the first base slot's runtime type.
			let output_element = match &node.output.shape.element {
				ir::Element::Concrete(element) => Some(substitute_ident_types(element, assignments)),
				ir::Element::Generic(ident) => assignments.iter().find(|(generic, _)| generic == ident).map(|(_, ty)| ty.clone()),
				ir::Element::Opaque => None,
			};
			let output_element = output_element.map(|element| substitute_lifetimes(&element, "'static"));
			let (io_output, wrap) = match &output_element {
				Some(element) => (
					quote!(gcore::registry::record_type::<#element>()),
					quote!(Ok(gcore::registry::EdgeHandle::new_record::<#element>(::std::sync::Arc::new(__node)))),
				),
				None => {
					let name = match &node.output.shape.element {
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
					let carrier_arg = (node.inputs.first().is_some_and(|input| input.subject) && ir::materialized_levels(&node, 0) == 0).then(|| quote!(&__layout_0,));
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

			// A carried generic instantiates through the struct's trailing phantom
			// parameters, so the constructor names the row's types after one inferred
			// slot per input field.
			let turbofish = (!carried.is_empty()).then(|| {
				let underscores = (0..arity).map(|_| quote!(_));
				let carried_types = carried
					.iter()
					.filter_map(|(generic, _)| assignments.iter().find(|(ident, _)| ident == generic).map(|(_, ty)| quote!(#ty)));
				quote!(::<#(#underscores,)* #(#carried_types,)*>)
			});

			Some(quote! {
				gcore::registry::RegistryEntry {
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
						let __node = #struct_name #turbofish::new(#(#names,)* #new_layout_args);
						#wrap
					},
				}
			})
		})
		.collect();

	if entries.is_empty() {
		return quote!();
	}
	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			vec![#(#entries),*]
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
