use super::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{GenericParam, Ident, Type};

pub(crate) fn entries_tokens(parsed: &ParsedNodeFn, class: &Class, struct_name: &Ident, data_field_generic_idents: &[Ident], regular_fields: &[&ParsedField]) -> TokenStream2 {
	if !data_field_generic_idents.is_empty() {
		return quote!();
	}
	match class {
		Class::RecordIo(_) => record_entries_tokens(parsed, struct_name, regular_fields),
		Class::Routing(_) => routing_entries_tokens(parsed, struct_name, regular_fields),
		Class::Flip { .. } => flip_entries_tokens(parsed, struct_name, regular_fields),
		Class::Opaque => record_opaque_entries_tokens(parsed, struct_name, regular_fields),
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

/// The registry row of a routing node: one instance covers every element,
/// sources claim generic record edges, and the constructor wraps each source
/// in its union translation and stores the union as the node's layout.
fn routing_entries_tokens(parsed: &ParsedNodeFn, struct_name: &Ident, regular_fields: &[&ParsedField]) -> TokenStream2 {
	let Some(routing) = routing_io(parsed) else {
		return quote!();
	};
	let is_source = |field: &ParsedField| {
		let ty = match &field.ty {
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
		};
		matches!(ty, Type::Path(path) if path.path.get_ident() == Some(&routing.generic))
	};
	let values_concrete = regular_fields.iter().filter(|field| !is_source(field)).all(|field| {
		let (ty, lend) = match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend, .. }) => (ty, lend.is_some()),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => (output_type, false),
		};
		!contains_open_generic(parsed, ty) && (lend || !type_disqualifies(ty))
	});
	if !values_concrete {
		return quote!();
	}

	let fn_name = &parsed.fn_name;
	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();
	let token_name = routing.generic.to_string();

	let input_types = regular_fields.iter().map(|field| {
		if is_source(field) {
			return quote!(gcore::registry::generic_record_edge_type(#token_name));
		}
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(gcore::registry::record_edge_type::<#ty>()),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => quote!(gcore::registry::edge_type::<#output_type>()),
		}
	});
	let source_layouts: Vec<Ident> = regular_fields
		.iter()
		.enumerate()
		.filter(|(_, field)| is_source(field))
		.map(|(index, _)| format_ident!("__layout_{index}"))
		.collect();
	let downcasts = regular_fields.iter().enumerate().map(|(index, field)| {
		let name = &field.pat_ident.ident;
		if is_source(field) {
			let layout = format_ident!("__layout_{index}");
			let handle = format_ident!("__handle_{index}");
			let ty = format_ident!("__ty_{index}");
			return quote! {
				let #handle = inputs.next().unwrap();
				let #ty = #handle.ty().clone();
				let Some(#layout) = #handle.layout().cloned() else {
					return Err(gcore::registry::ConstructionError::MissingLayout);
				};
				let #name = #handle.downcast_erased::<gcore::registry::ErasedRecordNode>(#ty.clone())?;
			};
		}
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => {
				let handle = format_ident!("__handle_{index}");
				let layout = format_ident!("__in_layout_{index}");
				quote! {
					let #handle = inputs.next().unwrap();
					let Some(#layout) = #handle.layout().cloned() else {
						return Err(gcore::registry::ConstructionError::MissingLayout);
					};
					let #name = #handle.downcast_record::<#ty>()?;
				}
			}
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => quote!(let #name = inputs.next().unwrap().downcast::<#output_type>()?;),
		}
	});
	let value_layout_args = regular_fields
		.iter()
		.enumerate()
		.filter(|(_, field)| !is_source(field) && matches!(field.ty, ParsedFieldType::Regular(_)))
		.map(|(index, _)| {
			let layout = format_ident!("__in_layout_{index}");
			quote!(&#layout,)
		});
	let source_wraps = regular_fields.iter().enumerate().filter(|(_, field)| is_source(field)).map(|(index, field)| {
		let name = &field.pat_ident.ident;
		let layout = format_ident!("__layout_{index}");
		quote!(let #name = gcore::record::RecordSource::new(#name, &#layout, &__union);)
	});
	let first_source_ty = regular_fields
		.iter()
		.enumerate()
		.find(|(_, field)| is_source(field))
		.map(|(index, _)| format_ident!("__ty_{index}"))
		.expect("routing nodes have a source");

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			vec![gcore::registry::RegistryEntry {
				io: gcore::registry::NodeIOTypes::new(
					gcore::concrete!(gcore::context::ContextImpl<'static>),
					gcore::Type::Record(Box::new(gcore::Type::Generic(::std::borrow::Cow::Borrowed(#token_name)))),
					vec![#(#input_types),*],
				),
				constructor: |inputs| {
					if inputs.len() != #arity {
						return Err(gcore::registry::ConstructionError::Arity { expected: #arity, got: inputs.len() });
					}
					let mut inputs = inputs.into_iter();
					#(#downcasts)*
					let __union = gcore::record::Layout::union(&[#(&#source_layouts),*]);
					#(#source_wraps)*
					let __node = #struct_name::new(#(#names,)* &__union, #(#value_layout_args)*);
					Ok(gcore::registry::EdgeHandle::new_erased(
						::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>,
						#first_source_ty,
					))
				},
			}]
		}
	}
}

fn record_opaque_entries_tokens(parsed: &ParsedNodeFn, struct_name: &Ident, regular_fields: &[&ParsedField]) -> TokenStream2 {
	let is_record = |field: &ParsedField| matches!(&field.ty, ParsedFieldType::Node(NodeParsedField { output_type, .. }) if is_record_value(output_type));
	let values_concrete = regular_fields.iter().filter(|field| !is_record(field)).all(|field| {
		let (ty, lend) = match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend, .. }) => (ty, lend.is_some()),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => (output_type, false),
		};
		!contains_open_generic(parsed, ty) && (lend || !type_disqualifies(ty))
	});
	if !values_concrete {
		return quote!();
	}

	let fn_name = &parsed.fn_name;
	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();

	let input_types = regular_fields.iter().map(|field| {
		if is_record(field) {
			return quote!(gcore::registry::generic_record_edge_type("T"));
		}
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(gcore::registry::edge_type::<#ty>()),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(gcore::registry::edge_type::<#ty>()),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => quote!(gcore::registry::edge_type::<#output_type>()),
		}
	});
	let downcasts = regular_fields.iter().enumerate().map(|(index, field)| {
		let name = &field.pat_ident.ident;
		if is_record(field) {
			let layout = format_ident!("__layout_{index}");
			let handle = format_ident!("__handle_{index}");
			let ty = format_ident!("__ty_{index}");
			return quote! {
				let #handle = inputs.next().unwrap();
				let #ty = #handle.ty().clone();
				let Some(#layout) = #handle.layout().cloned() else {
					return Err(gcore::registry::ConstructionError::MissingLayout);
				};
				let #name = #handle.downcast_erased::<gcore::registry::ErasedRecordNode>(#ty.clone())?;
			};
		}
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(let #name = inputs.next().unwrap().downcast::<#ty>()?;),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(let #name = inputs.next().unwrap().downcast::<#ty>()?;),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => quote!(let #name = inputs.next().unwrap().downcast::<#output_type>()?;),
		}
	});
	let first_record = regular_fields.iter().position(|field| is_record(field)).expect("record-opaque nodes have a record input");
	let record_layout = format_ident!("__layout_{first_record}");
	let record_ty = format_ident!("__ty_{first_record}");

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			vec![gcore::registry::RegistryEntry {
				io: gcore::registry::NodeIOTypes::new(
					gcore::concrete!(gcore::context::ContextImpl<'static>),
					gcore::Type::Record(Box::new(gcore::Type::Generic(::std::borrow::Cow::Borrowed("T")))),
					vec![#(#input_types),*],
				),
				constructor: |inputs| {
					if inputs.len() != #arity {
						return Err(gcore::registry::ConstructionError::Arity { expected: #arity, got: inputs.len() });
					}
					let mut inputs = inputs.into_iter();
					#(#downcasts)*
					let __node = #struct_name::new(#(#names,)* &#record_layout);
					Ok(gcore::registry::EdgeHandle::new_erased(
						::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>,
						#record_ty,
					))
				},
			}]
		}
	}
}

fn record_entries_tokens(parsed: &ParsedNodeFn, struct_name: &Ident, regular_fields: &[&ParsedField]) -> TokenStream2 {
	let Some(shape) = record_shape(parsed) else {
		return quote!();
	};
	let carrier_in_fields = !shape.skips_carrier();
	let values_concrete = regular_fields.iter().skip(carrier_in_fields as usize).all(|field| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { ty, lend, .. }) => !contains_open_generic(parsed, ty) && (lend.is_some() || !type_disqualifies(ty)),
		_ => false,
	});
	if !values_concrete {
		return quote!();
	}

	let fn_name = &parsed.fn_name;
	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();
	let reading_secondaries = reading_secondary_indices(regular_fields, &shape);

	let input_types = regular_fields.iter().enumerate().map(|(index, field)| {
		let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
			unreachable!("record nodes take no lazy inputs")
		};
		if carrier_in_fields && index == 0 {
			return match &shape.carrier {
				RecordCarrier::Token(token) => {
					let name = token.to_string();
					quote!(gcore::registry::generic_record_edge_type(#name))
				}
				RecordCarrier::Read(carrier_ty) => quote!(gcore::registry::record_edge_type::<#carrier_ty>()),
				RecordCarrier::None => unreachable!(),
			};
		}
		match field.attribute_reads.is_empty() {
			true => quote!(gcore::registry::edge_type::<#ty>()),
			false => quote!(gcore::registry::record_edge_type::<#ty>()),
		}
	});
	let downcasts = regular_fields.iter().enumerate().map(|(index, field)| {
		let name = &field.pat_ident.ident;
		let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
			unreachable!("record nodes take no lazy inputs")
		};
		if carrier_in_fields && index == 0 {
			return quote! {
				let __carrier_handle = inputs.next().unwrap();
				let __carrier_ty = __carrier_handle.ty().clone();
				let Some(__carrier_layout) = __carrier_handle.layout().cloned() else {
					return Err(gcore::registry::ConstructionError::MissingLayout);
				};
				let #name = __carrier_handle.downcast_erased::<gcore::registry::ErasedRecordNode>(__carrier_ty.clone())?;
			};
		}
		if !field.attribute_reads.is_empty() {
			let layout_local = format_ident!("__in_layout_{index}");
			return quote! {
				let __in_handle = inputs.next().unwrap();
				let __in_ty = __in_handle.ty().clone();
				let Some(#layout_local) = __in_handle.layout().cloned() else {
					return Err(gcore::registry::ConstructionError::MissingLayout);
				};
				let #name = __in_handle.downcast_erased::<gcore::registry::ErasedRecordNode>(__in_ty)?;
			};
		}
		quote!(let #name = inputs.next().unwrap().downcast::<#ty>()?;)
	});
	let wire_layout_arg = carrier_in_fields.then(|| quote!(&__carrier_layout,)).into_iter();
	let input_layout_args = reading_secondaries.iter().map(|index| {
		let layout_local = format_ident!("__in_layout_{index}");
		quote!(&#layout_local,)
	});
	let (io_output, construct_output) = match (&shape.carrier, &shape.element_write) {
		(RecordCarrier::Token(token), _) => {
			let name = token.to_string();
			(
				quote!(gcore::Type::Record(Box::new(gcore::Type::Generic(::std::borrow::Cow::Borrowed(#name))))),
				quote!(Ok(gcore::registry::EdgeHandle::new_erased(
					::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>,
					__carrier_ty,
				))),
			)
		}
		(_, Some(element)) => (
			quote!(gcore::registry::record_type::<#element>()),
			quote!(Ok(gcore::registry::EdgeHandle::new_record::<#element>(::std::sync::Arc::new(__node)))),
		),
		(_, None) => unreachable!("non-token record nodes write an element"),
	};

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			vec![gcore::registry::RegistryEntry {
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
					let __node = #struct_name::new(#(#names,)* #(#wire_layout_arg)* #(#input_layout_args)*);
					#construct_output
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
