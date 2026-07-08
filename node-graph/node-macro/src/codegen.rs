use crate::parsing::*;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote, quote_spanned};
use std::sync::atomic::AtomicU64;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Error, Ident, PatIdent, Token, WhereClause, WherePredicate, parse_quote};
static NODE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn generate_node_code(crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<TokenStream2> {
	let ParsedNodeFn {
		vis,
		attributes,
		fn_name,
		struct_name,
		mod_name,
		fn_generics,
		where_clause,
		input,
		output_type,
		is_async,
		fields,
		body,
		description,
		..
	} = parsed;
	let core_types = crate_ident.gcore()?;

	let category = attributes
		.category
		.as_ref()
		.expect("The 'category' attribute is required and should be checked during parsing, but was not found during codegen");
	let mod_name = format_ident!("_{}_mod", mod_name);

	let display_name = match &attributes.display_name.as_ref() {
		Some(lit) => lit.value(),
		None => struct_name.to_string().to_case(Case::Title),
	};
	let struct_name = format_ident!("{}Node", struct_name);

	// Separate data fields from regular fields
	let (data_fields, regular_fields): (Vec<_>, Vec<_>) = fields.iter().partition(|f| f.is_data_field);

	// Extract function generics used by data fields
	let data_field_generics: Vec<_> = fn_generics
		.iter()
		.filter(|generic| {
			let generic_ident = match generic {
				syn::GenericParam::Type(type_param) => &type_param.ident,
				_ => return false,
			};

			// Check if this generic is used in any data field type
			data_fields.iter().any(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => type_contains_ident(ty, generic_ident),
				_ => false,
			})
		})
		.cloned()
		.collect();

	// Node generics for regular fields (Node0, Node1, ...)
	let node_generics: Vec<Ident> = regular_fields.iter().enumerate().map(|(i, _)| format_ident!("Node{}", i)).collect();

	// An `Item<T>` primary input declares the node as an element-wise rank-0 kernel over element type `T`
	let primary_element = primary_item_element(parsed);
	let element_wise = primary_element.is_some();
	let mapped_variant = generates_mapped_variant(parsed);
	let list_content_variant = generates_list_content_variant(parsed);

	// Extract just the idents from data_field_generics for struct type parameters
	let data_field_generic_idents: Vec<Ident> = data_field_generics
		.iter()
		.filter_map(|gp| match gp {
			syn::GenericParam::Type(tp) => Some(tp.ident.clone()),
			_ => None,
		})
		.collect();

	// Combined struct type parameters: data field generic idents (T, U, ...) + node generics (Node0, Node1, ...)
	// For struct type instantiation: MemoizeNode<T, Node0>
	let struct_type_params: Vec<Ident> = data_field_generic_idents.iter().cloned().chain(node_generics.iter().cloned()).collect();

	// Combined struct generic parameters with bounds for struct definition
	// struct MemoizeNode<T: Clone, Node0>
	let struct_generic_params: Vec<TokenStream2> = data_field_generics.iter().map(|gp| quote!(#gp)).chain(node_generics.iter().map(|id| quote!(#id))).collect();
	let input_ident = &input.pat_ident;

	let context_features = &input.context_features;

	// Regular field idents and names (for function parameters)
	let field_idents: Vec<_> = regular_fields.iter().map(|f| &f.pat_ident).collect();
	let field_names: Vec<_> = field_idents.iter().map(|pat_ident| &pat_ident.ident).collect();
	let regular_field_names: Vec<_> = regular_fields.iter().map(|f| &f.pat_ident.ident).collect();
	let data_field_names: Vec<_> = data_fields.iter().map(|f| &f.pat_ident.ident).collect();

	// Only regular fields have input names/descriptions (for UI)
	let input_names: Vec<_> = regular_fields
		.iter()
		.map(|f| &f.name)
		.zip(regular_field_names.iter())
		.map(|zipped| match zipped {
			(Some(name), _) => name.value(),
			(_, name) => name.to_string().to_case(Case::Title),
		})
		.collect();

	let input_hidden = regular_field_names.iter().map(|name| name.to_string().starts_with('_')).collect::<Vec<_>>();

	let input_descriptions: Vec<_> = regular_fields.iter().map(|f| &f.description).collect();

	// Generate struct fields: data fields (concrete types) + regular fields (generic types)
	let data_field_defs = data_fields.iter().map(|field| {
		let name = &field.pat_ident.ident;
		let ty = match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
			_ => unreachable!("Data fields must be Regular types, not Node types"),
		};
		quote! { pub(super) #name: #ty }
	});

	let regular_field_defs = regular_field_names.iter().zip(node_generics.iter()).map(|(name, r#gen)| {
		quote! { pub(super) #name: #r#gen }
	});

	let struct_fields: Vec<_> = data_field_defs.chain(regular_field_defs).collect();

	let mut future_idents = Vec::new();

	// Data fields get passed as references to the underlying function
	let data_field_idents: Vec<_> = data_fields.iter().map(|f| &f.pat_ident).collect();
	let data_field_types: Vec<_> = data_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => {
				let ty = ty.clone();
				quote!(&#ty)
			}
			_ => unreachable!("Data fields must be Regular types, not Node types"),
		})
		.collect();

	// Regular fields have types passed to the function
	let field_types: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty.clone(),
			ParsedFieldType::Node(NodeParsedField { output_type, input_type, .. }) => match parsed.is_async {
				true => parse_quote!(&'n impl #core_types::Node<'n, #input_type, Output = impl core::future::Future<Output=#output_type>>),
				false => parse_quote!(&'n impl #core_types::Node<'n, #input_type, Output = #output_type>),
			},
		})
		.collect();

	// Only regular fields have UI metadata (data fields are internal state)
	let widget_override: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.widget_override {
			ParsedWidgetOverride::None => quote!(RegistryWidgetOverride::None),
			ParsedWidgetOverride::Hidden => quote!(RegistryWidgetOverride::Hidden),
			ParsedWidgetOverride::String(lit_str) => quote!(RegistryWidgetOverride::String(#lit_str)),
			ParsedWidgetOverride::Custom(lit_str) => quote!(RegistryWidgetOverride::Custom(#lit_str)),
		})
		.collect();

	let value_sources: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { value_source, .. }) => match value_source {
				ParsedValueSource::Default(data) => {
					// Check if the data is a string literal by parsing the token stream
					let data_str = data.to_string();
					if data_str.starts_with('"') && data_str.ends_with('"') && data_str.len() >= 2 {
						quote!(RegistryValueSource::Default(#data))
					} else {
						quote!(RegistryValueSource::Default(stringify!(#data)))
					}
				}
				ParsedValueSource::Scope(data) => {
					if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(_), .. }) = data {
						quote!(RegistryValueSource::Scope(#data))
					} else {
						quote!(RegistryValueSource::Scope(#data.as_static_str()))
					}
				}
				_ => quote!(RegistryValueSource::None),
			},
			_ => quote!(RegistryValueSource::None),
		})
		.collect();

	let default_types: Vec<_> = regular_fields
		.iter()
		.enumerate()
		.map(|(index, field)| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField {
				implementations, ty, value_source, ..
			}) => match implementations.first() {
				// A primary's scalar `#[default]` parses as a bare element (unranked, promoted at resolution); without one it defaults to an empty List
				Some(implementation_ty) if index == 0 && element_wise => match value_source {
					ParsedValueSource::Default(_) => quote!(Some(concrete!(#implementation_ty))),
					_ => quote!(Some(concrete!(#core_types::list::List<#implementation_ty>))),
				},
				Some(implementation_ty) => quote!(Some(concrete!(#implementation_ty))),
				// A concrete ranked `Item<T>` param's scalar `#[default]` parses as a bare `T` literal (unranked, promoted at resolution);
				// without one it keeps the declared `Item<T>` wire type, and `node_inputs` peels to `T` if no `Item` type default exists
				None => match peel_item(ty) {
					Some(element_ty)
						if !fn_generics
							.iter()
							.any(|generic| matches!(generic, syn::GenericParam::Type(param) if type_contains_ident(&element_ty, &param.ident))) =>
					{
						match value_source {
							ParsedValueSource::Default(_) => quote!(Some(concrete!(#element_ty))),
							_ => quote!(Some(concrete!(#ty))),
						}
					}
					_ => quote!(None),
				},
			},
			_ => quote!(None),
		})
		.collect();

	let bound_values = |select: fn(&RegularParsedField) -> &Option<NumberBound>| -> Vec<_> {
		regular_fields
			.iter()
			.map(|field| match &field.ty {
				ParsedFieldType::Regular(regular) => select(regular).as_ref().map_or(quote!(None), |bound| quote!(Some(#bound))),
				_ => quote!(None),
			})
			.collect()
	};
	let number_soft_min_values = bound_values(|field| &field.number_soft_min);
	let number_soft_max_values = bound_values(|field| &field.number_soft_max);
	let number_hard_min_values = bound_values(|field| &field.number_hard_min);
	let number_hard_max_values = bound_values(|field| &field.number_hard_max);
	let number_mode_range_values: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { number_mode_range, .. }) => quote!(#number_mode_range),
			_ => quote!(false),
		})
		.collect();
	let number_display_decimal_places: Vec<_> = regular_fields
		.iter()
		.map(|field| field.number_display_decimal_places.as_ref().map_or(quote!(None), |i| quote!(Some(#i))))
		.collect();
	let number_step: Vec<_> = regular_fields.iter().map(|field| field.number_step.as_ref().map_or(quote!(None), |i| quote!(Some(#i)))).collect();

	let unit_suffix: Vec<_> = regular_fields.iter().map(|field| field.unit.as_ref().map_or(quote!(None), |i| quote!(Some(#i)))).collect();

	let exposed: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { exposed, .. }) => quote!(#exposed),
			_ => quote!(true),
		})
		.collect();

	// Only eval regular fields (data fields are accessed directly as self.field_name)
	let eval_args = regular_fields.iter().map(|field| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			ParsedFieldType::Regular { .. } => {
				quote! { let #name = self.#name.eval(__input.clone()).await; }
			}
			ParsedFieldType::Node { .. } => {
				quote! { let #name = &self.#name; }
			}
		}
	});

	// Only regular fields can have min/max constraints
	let min_max_args = regular_fields.iter().map(|field| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { number_hard_min, number_hard_max, .. }) => {
			let name = &field.pat_ident.ident;
			let mut tokens = quote!();
			if let Some(min) = number_hard_min {
				tokens.extend(quote_spanned! {min.span()=>
					let #name = #core_types::misc::Clampable::clamp_hard_min(#name, #min);
				});
			}

			if let Some(max) = number_hard_max {
				tokens.extend(quote_spanned! {max.span()=>
					let #name = #core_types::misc::Clampable::clamp_hard_max(#name, #max);
				});
			}
			tokens
		}
		ParsedFieldType::Node { .. } => quote!(),
	});

	let all_implementation_types = fields.iter().flat_map(|field| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { implementations, .. }) => implementations.iter().cloned().collect::<Vec<_>>(),
		ParsedFieldType::Node(NodeParsedField { implementations, .. }) => implementations
			.iter()
			.flat_map(|implementation| [implementation.input.clone(), implementation.output.clone()])
			.collect(),
	});
	let all_implementation_types = all_implementation_types.chain(input.implementations.iter().cloned());

	let input_type = &parsed.input.ty;

	// Add Clampable bounds for fields with hard bounds, applying to each variant's evaluated wire type
	let build_clampable_clauses = |primary_wire: Option<WireWrapper>| -> Vec<TokenStream2> {
		regular_fields
			.iter()
			.filter_map(|field| {
				let ParsedFieldType::Regular(RegularParsedField {
					ty, number_hard_min, number_hard_max, ..
				}) = &field.ty
				else {
					return None;
				};
				if number_hard_min.is_none() && number_hard_max.is_none() {
					return None;
				}

				let ty = match (peel_item(ty), primary_wire) {
					(Some(element_ty), Some(wrap)) => wrap.apply(core_types, &element_ty),
					_ => ty.clone(),
				};
				Some(quote!(#ty: #core_types::misc::Clampable))
			})
			.collect()
	};
	future_idents.extend((0..regular_fields.len()).map(|id| format_ident!("F{}", id)));

	// Builds every field's where-clause bounds, optionally wrapping the primary field's wire type for the element-wise variants.
	// `list_content` additionally lifts a lazy primary connector's `Item<E>` output to `List<E>` for the list-content variant.
	let build_field_clauses = |primary_wire: Option<WireWrapper>, list_content: bool| -> Vec<TokenStream2> {
		regular_fields
			.iter()
			.zip(node_generics.iter())
			.zip(future_idents.iter())
			.enumerate()
			.map(|(index, ((field, name), fut_ident))| match (&field.ty, *is_async) {
				(ParsedFieldType::Regular(RegularParsedField { ty, .. }), _) => {
					let ty = match (peel_item(ty), primary_wire) {
						// An `Item<T>`-declared connector contributes its element type to the wire wrapping
						(Some(element_ty), Some(wrap)) => wrap.apply(core_types, &element_ty),
						_ => ty.clone(),
					};
					let all_lifetime_ty = substitute_lifetimes(ty.clone(), "all");
					quote!(
						#fut_ident: core::future::Future<Output = #ty> + #core_types::WasmNotSend + 'n,
						for<'all> #all_lifetime_ty: #core_types::WasmNotSend,
						#name: #core_types::Node<'n, #input_type, Output = #fut_ident> + #core_types::WasmNotSync
					)
				}
				(ParsedFieldType::Node(NodeParsedField { input_type, output_type, .. }), true) => {
					let output_type = if list_content && index == 0 {
						let element_ty = peel_item(output_type).unwrap_or_else(|| output_type.clone());
						WireWrapper::List.apply(core_types, &element_ty)
					} else {
						output_type.clone()
					};
					quote!(
						#fut_ident: core::future::Future<Output = #output_type> + #core_types::WasmNotSend + 'n,
						#name: #core_types::Node<'n, #input_type, Output = #fut_ident > + #core_types::WasmNotSync
					)
				}
				(ParsedFieldType::Node { .. }, false) => unreachable!("Found node which takes an impl Node<> input but is not async"),
			})
			.collect()
	};
	let where_clause = where_clause.clone().unwrap_or(WhereClause {
		where_token: Token![where](output_type.span()),
		predicates: Default::default(),
	});

	let make_struct_where_clause = |field_clauses: Vec<TokenStream2>, clampable_clauses: Vec<TokenStream2>, extra_clauses: Vec<TokenStream2>| {
		let mut struct_where_clause = where_clause.clone();
		let extra_where: Punctuated<WherePredicate, Comma> = parse_quote!(
			#(#field_clauses,)*
			#(#clampable_clauses,)*
			#(#extra_clauses,)*
			#output_type: 'n,
		);
		struct_where_clause.predicates.extend(extra_where);
		struct_where_clause
	};
	let primary_wire = element_wise.then_some(WireWrapper::Item);
	let struct_where_clause = make_struct_where_clause(build_field_clauses(primary_wire, false), build_clampable_clauses(primary_wire), Vec::new());

	// The mapped variant clones bare parameters and clones ranked connectors' items per frame slot, so both need Clone
	let param_clone_clauses: Vec<TokenStream2> = regular_fields
		.iter()
		.filter_map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => match peel_item(ty) {
				Some(element_ty) => Some(quote!(#element_ty: Clone)),
				None => Some(quote!(#ty: Clone)),
			},
			ParsedFieldType::Node(_) => None,
		})
		.collect();
	let mapped_struct_where_clause = mapped_variant.then(|| {
		make_struct_where_clause(
			build_field_clauses(Some(WireWrapper::List), false),
			build_clampable_clauses(Some(WireWrapper::List)),
			param_clone_clauses.clone(),
		)
	});

	// The list-content variant clones each content slot and feeds it in behind a shared reference held across the kernel's await,
	// so the primary connector's element type needs `Clone` (to clone the slot) and `Sync` (so `&PrecomputedItemNode` is `Send`)
	let list_content_struct_where_clause = list_content_variant.then(|| {
		let mut extra_clauses = param_clone_clauses.clone();
		if let Some(content_element) = &primary_element {
			extra_clauses.push(quote!(#content_element: Clone + #core_types::WasmNotSync));
		}
		make_struct_where_clause(build_field_clauses(Some(WireWrapper::List), true), build_clampable_clauses(Some(WireWrapper::List)), extra_clauses)
	});

	// Only regular fields are parameters to new()
	let new_args: Vec<_> = node_generics
		.iter()
		.zip(regular_field_names.iter())
		.map(|(r#gen, name)| {
			quote! { #name: #r#gen }
		})
		.collect();

	// Initialize data fields with Default, regular fields with parameters
	let data_inits = data_field_names.iter().map(|name| {
		quote! { #name: Default::default() }
	});
	let regular_inits = regular_field_names.iter().map(|name| {
		quote! { #name }
	});
	let all_field_inits: Vec<_> = data_inits.chain(regular_inits).collect();

	let async_keyword = is_async.then(|| quote!(async));
	let await_keyword = is_async.then(|| quote!(.await));

	// Data fields may not implement Copy, PartialEq, etc., so only derive Debug and Clone
	let struct_derives = if data_fields.is_empty() {
		quote!(#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)])
	} else {
		quote!(#[derive(Debug, Clone)])
	};

	// Generate serialize method if serialize attribute is specified
	let serialize_impl = if let Some(serialize_fn) = &parsed.attributes.serialize {
		let data_field_refs = data_field_names.iter().map(|name| quote!(&self.#name));
		quote! {
			fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
				#serialize_fn(#(#data_field_refs),*)
			}
		}
	} else {
		quote!()
	};

	let eval_prelude = quote! {
		use #core_types::misc::Clampable;

		#(#eval_args)*
		#(#min_max_args)*
	};

	let eval_impl = quote! {
		type Output = #core_types::registry::DynFuture<'n, #output_type>;
		#[inline]
		fn eval(&'n self, __input: #input_type) -> Self::Output {
			Box::pin(async move {
				#eval_prelude
				self::#fn_name(__input #(, &self.#data_field_names)* #(, #regular_field_names)*) #await_keyword
			})
		}

		#serialize_impl
	};

	// The mapped variant zips every ranked connector by frame slot (longest-list, last-element repeats), broadcasting bare parameters by clone
	let mapped_eval_impl = mapped_variant.then(|| {
		let ranked_names: Vec<_> = regular_fields
			.iter()
			.filter(|field| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { ty, .. }) if peel_item(ty).is_some()))
			.map(|field| &field.pat_ident.ident)
			.collect();

		let per_slot_args: Vec<_> = regular_fields
			.iter()
			.map(|field| {
				let name = &field.pat_ident.ident;
				match &field.ty {
					ParsedFieldType::Regular(RegularParsedField { ty, .. }) if peel_item(ty).is_some() => quote! {
						#name.clone_item(__slot_index.min(#name.len() - 1)).expect("A zip slot index is always within bounds")
					},
					ParsedFieldType::Regular(_) => quote!(#name.clone()),
					ParsedFieldType::Node(_) => quote!(#name),
				}
			})
			.collect();

		// The frame index is stamped onto each slot's context so lazy connectors can generate uniquely per slot.
		// A `()` generator frames purely over its param values (no stamp), so its kernel needs no context-extraction bounds.
		let has_lazy_connectors = regular_fields.iter().any(|field| matches!(&field.ty, ParsedFieldType::Node(_)));
		let slot_context = match has_lazy_connectors {
			true => quote!(#core_types::OwnedContextImpl::from(__input.clone()).with_index(__slot_index).into_context()),
			false => quote!(__input.clone()),
		};

		// An expander kernel (returning `List<U>`) flat-maps under the frame per the rank-2 force-flatten rule; a map kernel pushes one item per slot
		let (mapped_output_type, initial_output, collect_result) = match peel_item(output_type) {
			Some(element_ty) => (
				quote!(#core_types::list::List<#element_ty>),
				quote!(#core_types::list::List::with_capacity(__frame_length)),
				quote!(__output.push(__result);),
			),
			None => (quote!(#output_type), quote!(#core_types::list::List::new()), quote!(__output.extend(__result);)),
		};

		quote! {
			type Output = #core_types::registry::DynFuture<'n, #mapped_output_type>;
			#[inline]
			#[allow(clippy::clone_on_copy)]
			fn eval(&'n self, __input: #input_type) -> Self::Output {
				Box::pin(async move {
					#eval_prelude

					let __frame_length = [#(#ranked_names.len()),*].into_iter().max().unwrap_or(0);
					if [#(#ranked_names.len()),*].into_iter().any(|length| length == 0) {
						return #core_types::list::List::new();
					}

					let mut __output = #initial_output;
					for __slot_index in 0..__frame_length {
						let __slot_context = #slot_context;
						let __result = self::#fn_name(__slot_context #(, &self.#data_field_names)* #(, #per_slot_args)*) #await_keyword;
						#collect_result
					}

					__output
				})
			}

			#serialize_impl
		}
	});

	// The list-content variant evaluates a lazy primary's whole content `List` once, then feeds each slot into the kernel as a
	// precomputed stub (ambient footprint, no index stamp), zipping ranked params by slot with the frame taken from the content length.
	let list_content_eval_impl = list_content_variant.then(|| {
		let primary_name = &regular_fields[0].pat_ident.ident;

		let ranked_names: Vec<_> = regular_fields
			.iter()
			.filter(|field| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { ty, .. }) if peel_item(ty).is_some()))
			.map(|field| &field.pat_ident.ident)
			.collect();

		let per_slot_args: Vec<_> = regular_fields
			.iter()
			.enumerate()
			.map(|(index, field)| {
				let name = &field.pat_ident.ident;
				if index == 0 {
					return quote!(&__stub);
				}
				match &field.ty {
					ParsedFieldType::Regular(RegularParsedField { ty, .. }) if peel_item(ty).is_some() => quote! {
						#name.clone_item(__slot_index.min(#name.len() - 1)).expect("A zip slot index is always within bounds")
					},
					ParsedFieldType::Regular(_) => quote!(#name.clone()),
					ParsedFieldType::Node(_) => quote!(#name),
				}
			})
			.collect();

		let (list_content_output_type, initial_output, collect_result) = match peel_item(output_type) {
			Some(element_ty) => (
				quote!(#core_types::list::List<#element_ty>),
				quote!(#core_types::list::List::with_capacity(__frame_length)),
				quote!(__output.push(__result);),
			),
			None => (quote!(#output_type), quote!(#core_types::list::List::new()), quote!(__output.extend(__result);)),
		};

		let empty_param_check = (!ranked_names.is_empty()).then(|| {
			quote! {
				if [#(#ranked_names.len()),*].into_iter().any(|__length| __length == 0) {
					return #core_types::list::List::new();
				}
			}
		});

		quote! {
			type Output = #core_types::registry::DynFuture<'n, #list_content_output_type>;
			#[inline]
			#[allow(clippy::clone_on_copy)]
			fn eval(&'n self, __input: #input_type) -> Self::Output {
				Box::pin(async move {
					#eval_prelude

					let __content = #primary_name.eval(#core_types::OwnedContextImpl::from(__input.clone()).into_context()) #await_keyword;
					let __frame_length = __content.len();
					#empty_param_check

					let mut __output = #initial_output;
					for __slot_index in 0..__frame_length {
						let __stub = #core_types::value::PrecomputedItemNode::new(
							__content.clone_item(__slot_index).expect("A content slot index is always within bounds")
						);
						let __result = self::#fn_name(__input.clone() #(, &self.#data_field_names)* #(, #per_slot_args)*) #await_keyword;
						#collect_result
					}

					__output
				})
			}

			#serialize_impl
		}
	});

	let identifier = format_ident!("{}_proto_ident", fn_name);
	let identifier_path = match parsed.attributes.path.as_ref() {
		Some(path) => {
			let path = path.to_token_stream().to_string().replace(' ', "");
			quote!(#path)
		}
		None => quote!(std::module_path!()),
	};

	let mapped_struct_name = format_ident!("{}Mapped", struct_name);
	let list_content_struct_name = format_ident!("{}ListContent", struct_name);
	let register_node_impl = generate_register_node_impl(parsed, &field_names, &struct_name, &mapped_struct_name, &list_content_struct_name, &identifier)?;
	let import_name = format_ident!("_IMPORT_STUB_{}", mod_name.to_string().to_case(Case::UpperSnake));

	let properties = &attributes.properties_string.as_ref().map(|value| quote!(Some(#value))).unwrap_or(quote!(None));
	let memoize_flag = attributes.memoize;
	let inject_scope_flag = attributes.inject_scope;

	let cfg = crate::shader_nodes::modify_cfg(attributes);
	let node_input_accessor = generate_node_input_references(parsed, fn_generics, &field_idents, core_types, &identifier, &cfg);
	let ShaderTokens { shader_entry_point, gpu_node } = attributes.shader_node.as_ref().map(|n| n.codegen(crate_ident, parsed)).unwrap_or(Ok(ShaderTokens::default()))?;

	let mapped_node_impl = match (&mapped_struct_where_clause, &mapped_eval_impl) {
		(Some(mapped_where_clause), Some(mapped_eval)) => quote! {
			#cfg
			#[automatically_derived]
			impl<'n, #(#fn_generics,)* #(#node_generics,)* #(#future_idents,)*> #core_types::Node<'n, #input_type> for #mod_name::#mapped_struct_name<#(#struct_type_params,)*>
			#mapped_where_clause
			{
				#mapped_eval
			}
		},
		_ => quote!(),
	};

	let list_content_node_impl = match (&list_content_struct_where_clause, &list_content_eval_impl) {
		(Some(list_content_where_clause), Some(list_content_eval)) => quote! {
			#cfg
			#[automatically_derived]
			impl<'n, #(#fn_generics,)* #(#node_generics,)* #(#future_idents,)*> #core_types::Node<'n, #input_type> for #mod_name::#list_content_struct_name<#(#struct_type_params,)*>
			#list_content_where_clause
			{
				#list_content_eval
			}
		},
		_ => quote!(),
	};

	let mapped_struct_def = mapped_variant.then(|| {
		quote! {
			#struct_derives
			pub struct #mapped_struct_name<#(#struct_generic_params,)*> {
				#(#struct_fields,)*
			}

			#[automatically_derived]
			impl<'n, #(#struct_generic_params,)*> #mapped_struct_name<#(#struct_type_params,)*>
			{
				#[allow(clippy::too_many_arguments)]
				pub fn new(#(#new_args,)*) -> Self {
					Self {
						#(#all_field_inits,)*
					}
				}
			}
		}
	});

	let list_content_struct_def = list_content_variant.then(|| {
		quote! {
			#struct_derives
			pub struct #list_content_struct_name<#(#struct_generic_params,)*> {
				#(#struct_fields,)*
			}

			#[automatically_derived]
			impl<'n, #(#struct_generic_params,)*> #list_content_struct_name<#(#struct_type_params,)*>
			{
				#[allow(clippy::too_many_arguments)]
				pub fn new(#(#new_args,)*) -> Self {
					Self {
						#(#all_field_inits,)*
					}
				}
			}
		}
	});

	let mapped_struct_export = mapped_variant.then(|| {
		quote! {
			#cfg
			#[doc(hidden)]
			pub use #mod_name::#mapped_struct_name;
		}
	});

	let list_content_struct_export = list_content_variant.then(|| {
		quote! {
			#cfg
			#[doc(hidden)]
			pub use #mod_name::#list_content_struct_name;
		}
	});

	let display_name_header = format!("# {display_name}");
	let mut description_doc_attrs = vec![quote!(#[doc = #display_name_header]), quote!(#[doc = ""])];
	description_doc_attrs.extend(description.lines().map(|line| quote!(#[doc = #line])));

	// Add parameter list to doc comment
	if !input_names.is_empty() {
		description_doc_attrs.push(quote!(#[doc = ""]));
		description_doc_attrs.push(quote!(#[doc = "## Parameters"]));
		for (name, desc) in input_names.iter().zip(input_descriptions.iter()) {
			if desc.is_empty() {
				let header = format!("- **{name}**");
				description_doc_attrs.push(quote!(#[doc = #header]));
			} else {
				let first_line = desc.lines().next().unwrap_or("");
				let header = format!("- **{name}**: {first_line}");
				description_doc_attrs.push(quote!(#[doc = #header]));
				for line in desc.lines().skip(1) {
					let continuation = format!("  {line}");
					description_doc_attrs.push(quote!(#[doc = #continuation]));
				}
			}
		}
	}

	Ok(quote! {
		#(#description_doc_attrs)*
		#[inline]
		#[allow(clippy::too_many_arguments)]
		#vis #async_keyword fn #fn_name <'n, #(#fn_generics,)*> (#input_ident: #input_type #(, #data_field_idents: #data_field_types)* #(, #field_idents: #field_types)*) -> #output_type #where_clause #body

		#cfg
		#[automatically_derived]
		impl<'n, #(#fn_generics,)* #(#node_generics,)* #(#future_idents,)*> #core_types::Node<'n, #input_type> for #mod_name::#struct_name<#(#struct_type_params,)*>
		#struct_where_clause
		{
			#eval_impl
		}

		#mapped_node_impl

		#list_content_node_impl

		#cfg
		const fn #identifier() -> #core_types::ProtoNodeIdentifier {
			#core_types::ProtoNodeIdentifier::new(std::concat!(#identifier_path, "::", std::stringify!(#struct_name)))
		}

		#cfg
		#[doc(inline)]
		pub use #mod_name::#struct_name;

		#mapped_struct_export

		#list_content_struct_export

		#[doc(hidden)]
		#node_input_accessor

		#cfg
		#[doc(hidden)]
		#[allow(clippy::module_inception)]
		mod #mod_name {
			use super::*;
			use #core_types as gcore;
			use gcore::{Node, NodeIOTypes, concrete, fn_type, fn_type_fut, future, ProtoNodeIdentifier, WasmNotSync, NodeIO, ContextFeature};
			use gcore::value::ClonedNode;
			use gcore::ops::TypeNode;
			use gcore::registry::{NodeMetadata, FieldMetadata, NODE_REGISTRY, NODE_METADATA, DynAnyNode, DowncastBothNode, DynFuture, TypeErasedBox, PanicNode, RegistryValueSource, RegistryWidgetOverride};
			use gcore::ctor::ctor;

			// Use the types specified in the implementation

			static #import_name: core::marker::PhantomData<(#(#all_implementation_types,)*)> = core::marker::PhantomData;

			#struct_derives
			pub struct #struct_name<#(#struct_generic_params,)*> {
				#(#struct_fields,)*
			}

			#[automatically_derived]
			impl<'n, #(#struct_generic_params,)*> #struct_name<#(#struct_type_params,)*>
			{
				#[allow(clippy::too_many_arguments)]
				pub fn new(#(#new_args,)*) -> Self {
					Self {
						#(#all_field_inits,)*
					}
				}
			}

			#mapped_struct_def

			#list_content_struct_def

			#register_node_impl

			#[cfg_attr(not(target_family = "wasm"), ctor)]
			fn register_metadata() {
				let metadata = NodeMetadata {
					display_name: #display_name,
					category: #category,
					description: #description,
					properties: #properties,
					context_features: vec![#(ContextFeature::#context_features,)*],
					memoize: #memoize_flag,
					inject_scope: #inject_scope_flag,
					fields: vec![
						#(
							FieldMetadata {
								name: #input_names,
								widget_override: #widget_override,
								description: #input_descriptions,
								hidden: #input_hidden,
								exposed: #exposed,
								value_source: #value_sources,
								default_type: #default_types,
								number_soft_min: #number_soft_min_values,
								number_soft_max: #number_soft_max_values,
								number_hard_min: #number_hard_min_values,
								number_hard_max: #number_hard_max_values,
								number_mode_range: #number_mode_range_values,
								number_display_decimal_places: #number_display_decimal_places,
								number_step: #number_step,
								unit: #unit_suffix,
							},
						)*
					],
				};
				NODE_METADATA.lock().unwrap().insert(#identifier(), metadata);
			}
		}

		#shader_entry_point

		#gpu_node
	})
}

/// Generates strongly typed utilites to access inputs
fn generate_node_input_references(
	parsed: &ParsedNodeFn,
	fn_generics: &[crate::GenericParam],
	field_idents: &[&PatIdent],
	core_types: &TokenStream2,
	identifier: &Ident,
	cfg: &TokenStream2,
) -> TokenStream2 {
	let inputs_module_name = format_ident!("{}", parsed.struct_name.to_string().to_case(Case::Snake));

	let mut generated_input_accessor = Vec::new();
	if !parsed.attributes.skip_impl {
		let (mut modified, mut generic_collector) = FilterUsedGenerics::new(fn_generics);

		for (input_index, (parsed_input, input_ident)) in parsed.fields.iter().zip(field_idents).enumerate() {
			let mut ty = match &parsed_input.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
			}
			.clone();

			// The element-wise primary input's document wire carries the mapped List form
			if input_index == 0
				&& let Some(element_ty) = primary_item_element(parsed)
			{
				ty = parse_quote!(#core_types::list::List<#element_ty>);
			}

			// We only want the necessary generics.
			let used = generic_collector.filter_unnecessary_generics(&mut modified, &mut ty);
			// TODO: figure out a better name that doesn't conflict with so many types
			let struct_name = format_ident!("{}Input", input_ident.ident.to_string().to_case(Case::Pascal));
			let (fn_generic_params, phantom_data_declerations) = generate_phantom_data(used.iter());

			// Only create structs with phantom data where necessary.
			generated_input_accessor.push(if phantom_data_declerations.is_empty() {
				quote! {
					pub struct #struct_name;
				}
			} else {
				quote! {
					pub struct #struct_name <#(#used),*>{
						#(#phantom_data_declerations,)*
					}
				}
			});
			generated_input_accessor.push(quote! {
				impl <#(#used),*> #core_types::NodeInputDecleration for #struct_name <#(#fn_generic_params),*> {
					const INDEX: usize = #input_index;
					fn identifier() -> #core_types::ProtoNodeIdentifier {
						#inputs_module_name::IDENTIFIER.clone()
					}
					type Result = #ty;
				}
			})
		}
	}

	quote! {
		#cfg
		pub mod #inputs_module_name {
			use super::*;

			/// The `ProtoNodeIdentifier` of this node without any generics attached to it
			pub const IDENTIFIER: #core_types::ProtoNodeIdentifier = #identifier();
			#(#generated_input_accessor)*
		}
	}
}

/// It is necessary to generate PhantomData for each fn generic to avoid compiler errors.
fn generate_phantom_data<'a>(fn_generics: impl Iterator<Item = &'a crate::GenericParam>) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
	let mut phantom_data_declerations = Vec::new();
	let mut fn_generic_params = Vec::new();

	for fn_generic_param in fn_generics {
		let field_name = format_ident!("phantom_{}", phantom_data_declerations.len());

		match fn_generic_param {
			crate::GenericParam::Lifetime(lifetime_param) => {
				let lifetime = &lifetime_param.lifetime;

				fn_generic_params.push(quote! {#lifetime});
				phantom_data_declerations.push(quote! {#field_name: core::marker::PhantomData<&#lifetime ()>})
			}
			crate::GenericParam::Type(type_param) => {
				let generic_name = &type_param.ident;

				fn_generic_params.push(quote! {#generic_name});
				phantom_data_declerations.push(quote! {#field_name: core::marker::PhantomData<#generic_name>});
			}
			_ => {}
		}
	}
	(fn_generic_params, phantom_data_declerations)
}

/// The wire container a generated node variant is registered with, wrapping the kernel's primary input element type.
#[derive(Clone, Copy, PartialEq)]
enum WireWrapper {
	Item,
	List,
}

impl WireWrapper {
	fn apply(self, core_types: &TokenStream2, ty: &syn::Type) -> syn::Type {
		match self {
			WireWrapper::Item => parse_quote!(#core_types::list::Item<#ty>),
			WireWrapper::List => parse_quote!(#core_types::list::List<#ty>),
		}
	}
}

/// The variant of a node registered under one identifier: `Plain` for non-element-wise nodes, and the three element-wise wire
/// shapes (`Item` content and params, `Mapped` `List` params, `ListContent` `List` content for a lazy primary).
#[derive(Clone, Copy, PartialEq)]
enum RegisterVariant {
	Plain,
	Item,
	Mapped,
	ListContent,
}

impl RegisterVariant {
	/// The wrapper applied to ranked eager params for this variant.
	fn param_wrap(self) -> Option<WireWrapper> {
		match self {
			RegisterVariant::Item => Some(WireWrapper::Item),
			RegisterVariant::Mapped | RegisterVariant::ListContent => Some(WireWrapper::List),
			RegisterVariant::Plain => None,
		}
	}
}

/// Returns the element type of the node's primary input if it is declared `Item<T>` (directly, or as a lazy
/// connector's `Output = Item<T>`), which marks the node as element-wise.
fn primary_item_element(parsed: &ParsedNodeFn) -> Option<syn::Type> {
	// Manually registered nodes control their own variants
	if parsed.attributes.skip_impl {
		return None;
	}

	let field = parsed.fields.first()?;
	if field.is_data_field {
		return None;
	}

	match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => peel_item(ty),
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => peel_item(output_type),
	}
}

/// Whether the element-wise node gets a mapped `List` wire variant: an eager primary is its own frame source, while a lazy primary (or a `()` generator) draws the frame from its ranked eager params, so it needs at least one.
fn generates_mapped_variant(parsed: &ParsedNodeFn) -> bool {
	// A `()` generator frames over its ranked params exactly like a lazy primary, but generating fresh content per slot instead of transforming an upstream item
	if is_generator_frame(parsed) {
		return true;
	}

	if primary_item_element(parsed).is_none() {
		return false;
	}

	let mut regular_fields = parsed.fields.iter().filter(|field| !field.is_data_field);
	match regular_fields.next().map(|field| &field.ty) {
		Some(ParsedFieldType::Node(_)) => regular_fields.any(|field| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { ty, .. }) if peel_item(ty).is_some())),
		_ => true,
	}
}

/// Whether the node is a `()`-primary generator that frames over its ranked params: a unit primary with at least one ranked (`Item<T>`) param.
/// Its mapped variant stamps the frame index (like a lazy primary) since each slot is a fresh generation, not a pre-evaluated content slot.
fn is_generator_frame(parsed: &ParsedNodeFn) -> bool {
	if parsed.attributes.skip_impl {
		return false;
	}

	let Some(primary) = parsed.fields.first() else { return false };
	if primary.is_data_field {
		return false;
	}
	let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &primary.ty else { return false };
	if !is_unit_type(ty) {
		return false;
	}

	parsed
		.fields
		.iter()
		.skip(1)
		.any(|field| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { ty, .. }) if peel_item(ty).is_some()))
}

/// Whether the type is the unit type `()`, which marks a generator with no primary input.
fn is_unit_type(ty: &syn::Type) -> bool {
	matches!(ty, syn::Type::Tuple(tuple) if tuple.elems.is_empty())
}

/// Whether the element-wise node gets a list-content `List` wire variant: only a lazy primary connector qualifies, since it draws its frame from the whole content `List` (an eager primary already maps over `List` content via the mapped variant).
fn generates_list_content_variant(parsed: &ParsedNodeFn) -> bool {
	primary_item_element(parsed).is_some() && matches!(parsed.fields.first().map(|field| &field.ty), Some(ParsedFieldType::Node(_)))
}

/// Extracts `T` from a wrapper type like `Item<T>` or `List<T>`, if the type's outermost segment matches the wrapper name.
fn peel_wrapper(ty: &syn::Type, wrapper: &str) -> Option<syn::Type> {
	let syn::Type::Path(type_path) = ty else { return None };
	let segment = type_path.path.segments.last()?;
	if segment.ident != wrapper {
		return None;
	}

	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else { return None };
	match arguments.args.first()? {
		GenericArgument::Type(inner) => Some(inner.clone()),
		_ => None,
	}
}

pub(crate) fn peel_item(ty: &syn::Type) -> Option<syn::Type> {
	peel_wrapper(ty, "Item")
}

fn generate_register_node_impl(
	parsed: &ParsedNodeFn,
	field_names: &[&Ident],
	struct_name: &Ident,
	mapped_struct_name: &Ident,
	list_content_struct_name: &Ident,
	identifier: &Ident,
) -> Result<TokenStream2, Error> {
	// On native, `register_node` and `register_metadata` run automatically via `#[ctor]`.
	// On Wasm, `ctor` isn't available, so this `extern "C"` fn is invoked from JS to register the same way.
	// `skip_impl` nodes don't generate a `register_node`, so the shim calls only `register_metadata` for them.
	let registry_name = format_ident!("__node_registry_{}_{}", NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst), struct_name);
	let register_node_call = if parsed.attributes.skip_impl { quote!() } else { quote!(register_node();) };
	let wasm_shim = quote! {
		#[cfg(target_family = "wasm")]
		#[unsafe(no_mangle)]
		extern "C" fn #registry_name() {
			#register_node_call
			register_metadata();
		}
	};

	if parsed.attributes.skip_impl {
		return Ok(wasm_shim);
	}

	let mut constructors = Vec::new();
	let unit = parse_quote!(gcore::Context);

	let regular_fields: Vec<_> = parsed.fields.iter().filter(|f| !f.is_data_field).collect();

	let parameter_types: Vec<_> = regular_fields
		.iter()
		.map(|field| {
			match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { implementations, ty, .. }) => {
					if !implementations.is_empty() {
						implementations.iter().map(|ty| (&unit, ty)).collect()
					} else {
						vec![(&unit, ty)]
					}
				}
				ParsedFieldType::Node(NodeParsedField {
					implementations,
					input_type,
					output_type,
					..
				}) => {
					if !implementations.is_empty() {
						implementations.iter().map(|impl_| (&impl_.input, &impl_.output)).collect()
					} else {
						vec![(input_type, output_type)]
					}
				}
			}
			.into_iter()
			.map(|(input, out)| (substitute_lifetimes(input.clone(), "_"), substitute_lifetimes(out.clone(), "_")))
			.collect::<Vec<_>>()
		})
		.collect();

	let max_implementations = parameter_types.iter().map(|x| x.len()).chain([parsed.input.implementations.len().max(1)]).max();

	// Element-wise nodes register a variant per wire shape per implementations row; all other nodes register one
	let gcore = quote!(gcore);
	let variants: Vec<RegisterVariant> = if primary_item_element(parsed).is_some() {
		let mut variants = vec![RegisterVariant::Item];
		if generates_mapped_variant(parsed) {
			variants.push(RegisterVariant::Mapped);
		}
		if generates_list_content_variant(parsed) {
			variants.push(RegisterVariant::ListContent);
		}
		variants
	} else if is_generator_frame(parsed) {
		// A `()` generator registers an Item form (single generation) plus a mapped form framed over its ranked params
		vec![RegisterVariant::Item, RegisterVariant::Mapped]
	} else {
		vec![RegisterVariant::Plain]
	};

	for i in 0..max_implementations.unwrap_or(0) {
		for &variant in &variants {
			let mut temp_constructors = Vec::new();
			let mut temp_node_io = Vec::new();
			let mut panic_node_types = Vec::new();

			for (j, types) in parameter_types.iter().enumerate() {
				let field_name = field_names[j];
				let (input_type, output_type) = &types[i.min(types.len() - 1)];
				// Rankedness comes from the field's declared type; its #[implementations(...)] entries are bare element types
				let field_is_ranked = matches!(&regular_fields[j].ty, ParsedFieldType::Regular(RegularParsedField { ty, .. }) if peel_item(ty).is_some());
				let is_lazy_primary = j == 0 && matches!(regular_fields[j].ty, ParsedFieldType::Node { .. });
				// The list-content variant lifts its lazy primary connector's `Item<E>` content to `List<E>`; ranked params follow the variant's param wrap
				let output_type = if is_lazy_primary && variant == RegisterVariant::ListContent {
					let element_ty = peel_item(output_type).unwrap_or_else(|| output_type.clone());
					WireWrapper::List.apply(&gcore, &element_ty)
				} else {
					match (field_is_ranked, variant.param_wrap()) {
						(true, Some(wrap)) => {
							let element_ty = peel_item(output_type).unwrap_or_else(|| output_type.clone());
							wrap.apply(&gcore, &element_ty)
						}
						_ => output_type.clone(),
					}
				};

				let node = matches!(regular_fields[j].ty, ParsedFieldType::Node { .. });

				let downcast_node = quote!(
					let #field_name: DowncastBothNode<#input_type, #output_type> = DowncastBothNode::new(args[#j].clone());
				);
				if node && !parsed.is_async {
					return Err(Error::new_spanned(&parsed.fn_name, "Node needs to be async if you want to use lambda parameters"));
				}
				temp_constructors.push(downcast_node);
				temp_node_io.push(quote!(fn_type_fut!(#input_type, #output_type, alias: #output_type)));
				panic_node_types.push(quote!(#input_type, DynFuture<'static, #output_type>));
			}
			let input_type = match parsed.input.implementations.is_empty() {
				true => parsed.input.ty.clone(),
				false => parsed.input.implementations[i.min(parsed.input.implementations.len() - 1)].clone(),
			};
			let variant_struct_name = match variant {
				RegisterVariant::Mapped => mapped_struct_name,
				RegisterVariant::ListContent => list_content_struct_name,
				RegisterVariant::Item | RegisterVariant::Plain => struct_name,
			};
			constructors.push(quote!(
				(
					|args| {
						Box::pin(async move {
							#(#temp_constructors;)*
							let node = #variant_struct_name::new(#(#field_names,)*);
							// try polling futures
							let any: DynAnyNode<#input_type, _, _> = DynAnyNode::new(node);
							Box::new(any) as TypeErasedBox<'_>
						})
					}, {
						let node = #variant_struct_name::new(#(PanicNode::<#panic_node_types>::new(),)*);
						let params = vec![#(#temp_node_io,)*];
						let mut node_io = NodeIO::<'_, #input_type>::to_async_node_io(&node, params);
						node_io

					}
				)
			));
		}
	}
	let native = quote! {
	#[cfg_attr(not(target_family = "wasm"), ctor)]
	fn register_node() {
		let mut registry = NODE_REGISTRY.lock().unwrap();
		registry.insert(
			#identifier(),
			vec![
				#(#constructors,)*
			]
		);
	}
	};
	if cfg!(feature = "disable-registration") {
		return Ok(native);
	}

	Ok(quote! {
		#native
		#wasm_shim
	})
}

use crate::crate_ident::CrateIdent;
use crate::shader_nodes::{ShaderCodegen, ShaderTokens};
use syn::visit_mut::VisitMut;
use syn::{GenericArgument, Lifetime, Type};

struct LifetimeReplacer(&'static str);

impl VisitMut for LifetimeReplacer {
	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		lifetime.ident = Ident::new(self.0, lifetime.ident.span());
	}

	fn visit_type_mut(&mut self, ty: &mut Type) {
		match ty {
			Type::Reference(type_reference) => {
				if let Some(lifetime) = &mut type_reference.lifetime {
					self.visit_lifetime_mut(lifetime);
				}
				self.visit_type_mut(&mut type_reference.elem);
			}
			_ => syn::visit_mut::visit_type_mut(self, ty),
		}
	}

	fn visit_generic_argument_mut(&mut self, arg: &mut GenericArgument) {
		if let GenericArgument::Lifetime(lifetime) = arg {
			self.visit_lifetime_mut(lifetime);
		} else {
			syn::visit_mut::visit_generic_argument_mut(self, arg);
		}
	}
}

#[must_use]
fn substitute_lifetimes(mut ty: Type, lifetime: &'static str) -> Type {
	LifetimeReplacer(lifetime).visit_type_mut(&mut ty);
	ty
}

/// Get only the necessary generics.
struct FilterUsedGenerics {
	all: Vec<crate::GenericParam>,
	used: Vec<bool>,
}

impl VisitMut for FilterUsedGenerics {
	fn visit_lifetime_mut(&mut self, used_lifetime: &mut Lifetime) {
		for (generic, used) in self.all.iter().zip(self.used.iter_mut()) {
			let crate::GenericParam::Lifetime(lifetime_param) = generic else { continue };
			if used_lifetime == &lifetime_param.lifetime {
				*used = true;
			}
		}
	}

	fn visit_path_mut(&mut self, path: &mut syn::Path) {
		for (index, (generic, used)) in self.all.iter().zip(self.used.iter_mut()).enumerate() {
			let crate::GenericParam::Type(type_param) = generic else { continue };
			if path.leading_colon.is_none() && !path.segments.is_empty() && path.segments[0].arguments.is_none() && path.segments[0].ident == type_param.ident {
				*used = true;
				// Sometimes the generics conflict with the type name so we rename the generics.
				path.segments[0].ident = format_ident!("G{index}");
			}
		}
		for mut el in Punctuated::pairs_mut(&mut path.segments) {
			self.visit_path_segment_mut(el.value_mut());
		}
	}
}

impl FilterUsedGenerics {
	fn new(fn_generics: &[crate::GenericParam]) -> (Vec<crate::GenericParam>, Self) {
		let mut all_possible_generics = fn_generics.to_vec();
		// The 'n lifetime may also be needed; we must add it in
		all_possible_generics.insert(0, syn::GenericParam::Lifetime(syn::LifetimeParam::new(Lifetime::new("'n", proc_macro2::Span::call_site()))));

		let modified = all_possible_generics
			.iter()
			.cloned()
			.enumerate()
			.map(|(index, mut generic)| {
				let crate::GenericParam::Type(type_param) = &mut generic else { return generic };
				// Sometimes the generics conflict with the type name so we rename the generics.
				type_param.ident = format_ident!("G{index}");
				generic
			})
			.collect::<Vec<_>>();

		let generic_collector = Self {
			used: vec![false; all_possible_generics.len()],
			all: all_possible_generics,
		};

		(modified, generic_collector)
	}

	fn used<'a>(&'a self, modified: &'a [crate::GenericParam]) -> impl Iterator<Item = &'a crate::GenericParam> {
		modified.iter().zip(&self.used).filter(|(_, used)| **used).map(move |(value, _)| value)
	}

	fn filter_unnecessary_generics(&mut self, modified: &mut Vec<syn::GenericParam>, ty: &mut Type) -> Vec<syn::GenericParam> {
		self.used.fill(false);

		// Find out which generics are necessary to support the node input
		self.visit_type_mut(ty);

		// Sometimes generics may reference other generics. This is a non-optimal way of dealing with that.
		for _ in 0..=self.all.len() {
			for (index, item) in modified.iter_mut().enumerate() {
				if self.used[index] {
					self.visit_generic_param_mut(item);
				}
			}
		}

		self.used(&*modified).cloned().collect()
	}
}

/// Check if a type contains a reference to a specific identifier (e.g., a generic type parameter)
fn type_contains_ident(ty: &Type, ident: &Ident) -> bool {
	struct IdentChecker<'a> {
		target: &'a Ident,
		found: bool,
	}

	impl<'a, 'ast> syn::visit::Visit<'ast> for IdentChecker<'a> {
		fn visit_ident(&mut self, i: &'ast Ident) {
			if i == self.target {
				self.found = true;
			}
		}
	}

	let mut checker = IdentChecker { target: ident, found: false };
	syn::visit::visit_type(&mut checker, ty);
	checker.found
}
