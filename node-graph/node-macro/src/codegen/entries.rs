use super::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{GenericParam, Ident, Type};

pub(crate) fn entries_tokens(parsed: &ParsedNodeFn, class: &Class, struct_name: &Ident, data_field_generic_idents: &[Ident], regular_fields: &[&ParsedField]) -> TokenStream2 {
	if !data_field_generic_idents.is_empty() {
		return quote!();
	}
	match class {
		Class::Flip { .. } => flip_entries_tokens(parsed, struct_name, regular_fields),
		Class::RecordIo(_) | Class::Routing(_) | Class::Opaque => single_row_entries(parsed, class, struct_name, regular_fields),
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
	let carrier_present = flip_carrier(parsed);
	let sources = if carrier_present { quote!(::std::vec![0u8]) } else { quote!(::std::vec![]) };

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
				let Some(#layout) = #handle.layout().cloned() else {
					return Err(gcore::registry::ConstructionError::MissingLayout);
				};
				let #name = #handle.downcast_record::<#ty>()?;
			}
		});
		let layout_args = (0..arity).map(|index| {
			let layout = format_ident!("__layout_{index}");
			quote!(&#layout,)
		});
		Some(quote! {
			gcore::registry::RegistryEntry {
				layout_meta: Some(gcore::record::LayoutMeta {
					sources: #sources,
					reads: ::std::vec::Vec::new(),
					element: gcore::record::ElementSpec::Concrete(gcore::record::element_write::<#row_output>()),
					writes: ::std::vec::Vec::new(),
					removes: ::std::vec::Vec::new(),
					level_delta: 0,
				}),
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
fn single_row_entries(parsed: &ParsedNodeFn, class: &Class, struct_name: &Ident, regular_fields: &[&ParsedField]) -> TokenStream2 {
	let fn_name = &parsed.fn_name;
	let lend = |field: &ParsedField| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }));

	let slots: Vec<SlotKind> = match class {
		Class::RecordIo(shape) => {
			let carrier_in_fields = !shape.skips_carrier();
			regular_fields
				.iter()
				.enumerate()
				.map(|(index, field)| {
					let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
						unreachable!("record nodes take no lazy inputs")
					};
					if carrier_in_fields && index == 0 {
						return match &shape.carrier {
							RecordCarrier::Token(token) => SlotKind::BaseGeneric(token.to_string()),
							RecordCarrier::Read(carrier_ty) => SlotKind::BaseConcrete(carrier_ty.clone()),
							RecordCarrier::None => unreachable!(),
						};
					}
					match field.attribute_reads.is_empty() {
						false => SlotKind::Value(ty.clone()),
						true => SlotKind::Plain(ty.clone()),
					}
				})
				.collect()
		}
		Class::Routing(routing) => {
			let is_source = |field: &ParsedField| {
				let ty = match &field.ty {
					ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
					ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
				};
				matches!(ty, Type::Path(path) if path.path.get_ident() == Some(&routing.generic))
			};
			regular_fields
				.iter()
				.map(|field| {
					if is_source(field) {
						return SlotKind::BaseGeneric(routing.generic.to_string());
					}
					match &field.ty {
						ParsedFieldType::Regular(RegularParsedField { ty, .. }) => SlotKind::Value(ty.clone()),
						ParsedFieldType::Node(NodeParsedField { output_type, .. }) => SlotKind::Lazy(output_type.clone()),
					}
				})
				.collect()
		}
		Class::Opaque => {
			let is_record = |field: &ParsedField| matches!(&field.ty, ParsedFieldType::Node(NodeParsedField { output_type, .. }) if is_record_value(output_type));
			regular_fields
				.iter()
				.map(|field| {
					if is_record(field) {
						return SlotKind::BaseGeneric("T".to_string());
					}
					match &field.ty {
						ParsedFieldType::Regular(RegularParsedField { ty, .. }) => SlotKind::Plain(ty.clone()),
						ParsedFieldType::Node(NodeParsedField { output_type, .. }) => SlotKind::Lazy(output_type.clone()),
					}
				})
				.collect()
		}
		Class::Flip { .. } => unreachable!("flip has its own multi-row emitter"),
	};

	// Every non-base value/plain/lazy input must be concrete.
	let values_concrete = regular_fields.iter().zip(&slots).all(|(field, slot)| match slot {
		SlotKind::BaseGeneric(_) | SlotKind::BaseConcrete(_) => true,
		SlotKind::Value(ty) | SlotKind::Plain(ty) | SlotKind::Lazy(ty) => !contains_open_generic(parsed, ty) && (lend(field) || !type_disqualifies(ty)),
	});
	if !values_concrete {
		return quote!();
	}

	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();

	let input_types = slots.iter().map(|slot| match slot {
		SlotKind::BaseGeneric(name) => quote!(gcore::registry::generic_record_edge_type(#name)),
		SlotKind::BaseConcrete(ty) | SlotKind::Value(ty) => quote!(gcore::registry::record_edge_type::<#ty>()),
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
				let Some(#layout) = #handle.layout().cloned() else {
					return Err(gcore::registry::ConstructionError::MissingLayout);
				};
				let #name = #handle.downcast_erased::<gcore::registry::ErasedRecordNode>(#ty.clone())?;
			},
			SlotKind::Value(value_ty) => quote! {
				let #handle = inputs.next().unwrap();
				let Some(#layout) = #handle.layout().cloned() else {
					return Err(gcore::registry::ConstructionError::MissingLayout);
				};
				let #name = #handle.downcast_record::<#value_ty>()?;
			},
			SlotKind::Plain(value_ty) | SlotKind::Lazy(value_ty) => quote!(let #name = inputs.next().unwrap().downcast::<#value_ty>()?;),
		}
	});

	let base_indices: Vec<usize> = slots.iter().enumerate().filter(|(_, slot)| slot.is_base()).map(|(index, _)| index).collect();
	let value_indices: Vec<usize> = slots.iter().enumerate().filter(|(_, slot)| matches!(slot, SlotKind::Value(_))).map(|(index, _)| index).collect();
	let value_layout_args = value_indices.iter().map(|index| {
		let layout = format_ident!("__layout_{index}");
		quote!(&#layout,)
	});

	let carried_meta = |sources: &[usize]| {
		let sources = sources.iter().map(|index| *index as u8);
		quote! {
			Some(gcore::record::LayoutMeta {
				sources: ::std::vec![#(#sources),*],
				reads: ::std::vec::Vec::new(),
				element: gcore::record::ElementSpec::Carried,
				writes: ::std::vec::Vec::new(),
				removes: ::std::vec::Vec::new(),
				level_delta: 0,
			})
		}
	};

	let (io_output, wrap, prelude, new_layout_args, layout_meta) = match class {
		Class::RecordIo(shape) => {
			let carrier_arg = (!shape.skips_carrier()).then(|| quote!(&__layout_0,));
			let (io_output, wrap) = match (&shape.carrier, &shape.element_write) {
				(RecordCarrier::Token(token), _) => {
					let token_name = token.to_string();
					(
						quote!(gcore::Type::Record(Box::new(gcore::Type::Generic(::std::borrow::Cow::Borrowed(#token_name))))),
						quote!(Ok(gcore::registry::EdgeHandle::new_erased(::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>, __ty_0))),
					)
				}
				(_, Some(element)) => (
					quote!(gcore::registry::record_type::<#element>()),
					quote!(Ok(gcore::registry::EdgeHandle::new_record::<#element>(::std::sync::Arc::new(__node)))),
				),
				(_, None) => unreachable!("non-token record nodes write an element"),
			};
			let layout_meta_fn = format_ident!("{}_layout_meta", fn_name);
			(io_output, wrap, quote!(), quote!(#carrier_arg #(#value_layout_args)*), quote!(Some(self::#layout_meta_fn())))
		}
		Class::Routing(routing) => {
			let token_name = routing.generic.to_string();
			let source_layouts = base_indices.iter().map(|index| format_ident!("__layout_{index}"));
			let source_wraps = base_indices.iter().map(|index| {
				let name = names[*index];
				let layout = format_ident!("__layout_{index}");
				quote!(let #name = gcore::record::RecordSource::new(#name, &#layout, &__union);)
			});
			let first_source_ty = format_ident!("__ty_{}", base_indices[0]);
			let prelude = quote! {
				let __union = gcore::record::Layout::union(&[#(&#source_layouts),*]);
				#(#source_wraps)*
			};
			(
				quote!(gcore::Type::Record(Box::new(gcore::Type::Generic(::std::borrow::Cow::Borrowed(#token_name))))),
				quote!(Ok(gcore::registry::EdgeHandle::new_erased(::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>, #first_source_ty))),
				prelude,
				quote!(&__union, #(#value_layout_args)*),
				carried_meta(&base_indices),
			)
		}
		Class::Opaque => {
			let first_record = base_indices[0];
			let record_layout = format_ident!("__layout_{first_record}");
			let record_ty = format_ident!("__ty_{first_record}");
			(
				quote!(gcore::Type::Record(Box::new(gcore::Type::Generic(::std::borrow::Cow::Borrowed("T"))))),
				quote!(Ok(gcore::registry::EdgeHandle::new_erased(::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>, #record_ty))),
				quote!(),
				quote!(&#record_layout),
				carried_meta(&[first_record]),
			)
		}
		Class::Flip { .. } => unreachable!("flip has its own multi-row emitter"),
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

fn implementation_rows(parsed: &ParsedNodeFn, regular_fields: &[&ParsedField]) -> Option<Vec<Vec<Type>>> {
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
