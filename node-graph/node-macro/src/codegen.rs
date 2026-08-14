use crate::crate_ident::CrateIdent;
use crate::parsing::*;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use std::sync::atomic::AtomicU64;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;
use syn::{GenericArgument, GenericParam, Ident, Lifetime, PatIdent, PathArguments, Type, TypeParam, TypeParamBound};
use crate::shader_nodes::{ShaderCodegen, ShaderTokens};

mod classify;
mod entries;
pub(crate) mod ir;
mod metadata;
pub(crate) use classify::*;
use entries::entries_tokens;
use ir::{LazyBinding, ValueBinding};
use metadata::generate_node_input_references;

static NODE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn generate_node_code(crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<TokenStream2> {
	let ParsedNodeFn {
		attributes,
		fn_name,
		struct_name,
		mod_name,
		fn_generics,
		input,
		output_type,
		fields,
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

	let model = analyze(parsed);
	let node = crate::codegen::ir::build(parsed);
	let kind = model.as_ref().map(|_| crate::codegen::ir::node_kind(&node));
	let carrier_present = node.inputs.first().is_some_and(|input| input.subject);
	let record_io = matches!(kind, Some(crate::codegen::ir::NodeKind::RecordIo));
	let flip = matches!(kind, Some(crate::codegen::ir::NodeKind::Flip));
	let carrier_flip = flip && carrier_present;
	let opaque = matches!(kind, Some(crate::codegen::ir::NodeKind::Opaque));
	let routing_generic = match (kind, &node.output.shape.element) {
		(Some(crate::codegen::ir::NodeKind::Routing), crate::codegen::ir::Element::Generic(ident)) => Some(ident.clone()),
		_ => None,
	};
	let record_skips_carrier = record_io && !carrier_present;
	// Record nodes with a `_: ()` primary input have no carrier edge; the unit
	// field stays visible in the metadata but claims no struct field.
	let struct_regular_fields: Vec<_> = regular_fields.iter().skip(record_skips_carrier as usize).copied().collect();
	let struct_regular_field_names: Vec<_> = struct_regular_fields.iter().map(|f| &f.pat_ident.ident).collect();

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
	let node_generics: Vec<Ident> = struct_regular_fields.iter().enumerate().map(|(i, _)| format_ident!("Node{}", i)).collect();

	// Extract just the idents from data_field_generics for struct type parameters
	let data_field_generic_idents: Vec<Ident> = data_field_generics
		.iter()
		.filter_map(|gp| match gp {
			syn::GenericParam::Type(tp) => Some(tp.ident.clone()),
			_ => None,
		})
		.collect();

	// Flipped nodes carry their kernel generics as struct parameters: record
	// edges no longer bind them through `Output`, so the struct must.
	let ctx_ident_for_flip = context_param(parsed).map(|ctx| ctx.ident.clone());
	let flip_generics: Vec<&syn::GenericParam> = if flip {
		fn_generics
			.iter()
			.filter(|param| match param {
				syn::GenericParam::Type(tp) => Some(&tp.ident) != ctx_ident_for_flip.as_ref() && !data_field_generic_idents.contains(&tp.ident),
				_ => false,
			})
			.collect()
	} else {
		Vec::new()
	};
	let flip_generic_idents: Vec<Ident> = flip_generics
		.iter()
		.filter_map(|param| match param {
			syn::GenericParam::Type(tp) => Some(tp.ident.clone()),
			_ => None,
		})
		.collect();

	// Combined struct type parameters: data field generic idents (T, U, ...) + node generics (Node0, Node1, ...)
	// For struct type instantiation: MemoizeNode<T, Node0>
	let struct_type_params: Vec<Ident> = data_field_generic_idents
		.iter()
		.cloned()
		.chain(node_generics.iter().cloned())
		.chain(flip_generic_idents.iter().cloned())
		.collect();

	// Combined struct generic parameters with bounds for struct definition
	// struct MemoizeNode<T: Clone, Node0>
	let struct_generic_params: Vec<TokenStream2> = data_field_generics
		.iter()
		.map(|gp| quote!(#gp))
		.chain(node_generics.iter().map(|id| quote!(#id)))
		.chain(flip_generics.iter().map(|gp| quote!(#gp)))
		.collect();
	let context_features = &input.context_features;

	// Regular field idents and names (for function parameters)
	let field_idents: Vec<_> = regular_fields.iter().map(|f| &f.pat_ident).collect();
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

	let regular_field_defs = struct_regular_field_names.iter().zip(node_generics.iter()).map(|(name, r#gen)| {
		quote! { pub(super) #name: #r#gen }
	});

	let record_state_fields: Vec<TokenStream2> = if record_io {
		let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout)];
		if !record_skips_carrier {
			state.push(quote!(pub(super) __carrier: gcore::record::Layout));
			state.push(quote!(pub(super) __plan: ::std::vec::Vec<(usize, usize, usize)>));
		}
		state.push(quote!(pub(super) __frame_bytes: usize));
		state.extend(reading_secondary_indices(&struct_regular_fields, record_skips_carrier).into_iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(pub(super) #slot: gcore::record::Layout)
		}));
		let total_reads: usize = struct_regular_fields.iter().map(|field| field.attribute_reads.len()).sum();
		state.extend((0..total_reads).map(|index| {
			let slot = format_ident!("__read_{index}");
			quote!(pub(super) #slot: Option<usize>)
		}));
		state.extend((0..node.output.shape.attrs.len()).map(|index| {
			let slot = format_ident!("__write_{index}");
			quote!(pub(super) #slot: usize)
		}));
		state
	} else if routing_generic.is_some() {
		let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout)];
		state.extend(routing_value_indices(&struct_regular_fields, routing_generic.as_ref().expect("guarded by the arm")).into_iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(pub(super) #slot: gcore::record::Layout)
		}));
		state
	} else if opaque {
		vec![quote!(pub(super) __layout: gcore::record::Layout)]
	} else if flip {
		let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout), quote!(pub(super) __frame_bytes: usize)];
		if carrier_flip {
			state.push(quote!(pub(super) __plan: ::std::vec::Vec<(usize, usize, usize)>));
		}
		state.extend((0..struct_regular_fields.len()).map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(pub(super) #slot: gcore::record::Layout)
		}));
		state.extend(lazy_read_fields(&struct_regular_fields).into_iter().map(|(index, field)| {
			let slot = format_ident!("__reads_{index}");
			let arity = field.attribute_reads.len();
			quote!(pub(super) #slot: [Option<usize>; #arity])
		}));
		if !flip_generic_idents.is_empty() {
			state.push(quote!(pub(super) __marker: ::core::marker::PhantomData<fn() -> (#(#flip_generic_idents,)*)>));
		}
		state
	} else {
		Vec::new()
	};

	let async_source = parsed.injects_async_source_fields();
	let slot_value_type = slot_value_type(output_type);
	let slot_field = async_source
		.then(|| quote! { pub(super) slot: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<u64, Option<gcore::gpoll::GPoll<#slot_value_type>>>>> })
		.into_iter();
	let struct_fields = data_field_defs.chain(regular_field_defs).chain(record_state_fields.iter().cloned()).chain(slot_field);

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
					if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(_), .. }) = &**data {
						quote!(RegistryValueSource::Scope(#data))
					} else {
						quote!(RegistryValueSource::Scope(#data.as_static_str()))
					}
				}
				ParsedValueSource::SourceId => quote!(RegistryValueSource::SourceId),
				_ => quote!(RegistryValueSource::None),
			},
			_ => quote!(RegistryValueSource::None),
		})
		.collect();

	let default_types: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { implementations, .. }) => match implementations.first() {
				Some(ty) => quote!(Some(concrete!(#ty))),
				_ => quote!(None),
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
	let all_implementation_types = fields.iter().flat_map(|field| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { implementations, .. }) => implementations.iter().cloned().collect::<Vec<_>>(),
		ParsedFieldType::Node(NodeParsedField { implementations, .. }) => implementations
			.iter()
			.flat_map(|implementation| [implementation.input.clone(), implementation.output.clone()])
			.collect(),
	});
	let all_implementation_types = all_implementation_types.chain(input.implementations.iter().cloned());

	// Only regular fields are parameters to new()
	let new_args = node_generics.iter().zip(struct_regular_field_names.iter()).map(|(r#gen, name)| {
		quote! { #name: #r#gen }
	});

	// Initialize data fields with Default, regular fields with parameters
	let data_inits = data_field_names.iter().map(|name| {
		quote! { #name: Default::default() }
	});
	let regular_inits = struct_regular_field_names.iter().map(|name| {
		quote! { #name }
	});
	let slot_init = async_source.then(|| quote! { slot: Default::default() }).into_iter();
	let all_field_inits = data_inits.chain(regular_inits).chain(slot_init);

	// Data fields may not implement Copy, PartialEq, etc., so only derive Debug and Clone
	let struct_derives = if record_io || routing_generic.is_some() || flip {
		quote!(#[derive(Debug, Clone)])
	} else if data_fields.is_empty() && !async_source {
		quote!(#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)])
	} else {
		quote!(#[derive(Debug, Clone)])
	};

	let identifier = format_ident!("{}_proto_ident", fn_name);
	let identifier_path = match parsed.attributes.path.as_ref() {
		Some(path) => {
			let path = path.to_token_stream().to_string().replace(' ', "");
			quote!(#path)
		}
		None => quote!(std::module_path!()),
	};

	let registry_name = format_ident!("__node_registry_{}_{}", NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst), struct_name);
	let register_node_impl = quote! {
		#[cfg(target_family = "wasm")]
		#[unsafe(no_mangle)]
		extern "C" fn #registry_name() {
			register_metadata();
		}
	};
	// Record nodes construct through the generated `wire` fn, which resolves
	// offsets from the carrier layout; `new` cannot fill that state.
	let routing_layout_param = (routing_generic.is_some() || opaque).then(|| quote!(__layout: &gcore::record::Layout,)).into_iter();
	let routing_layout_init = (routing_generic.is_some() || opaque).then(|| quote!(__layout: __layout.clone(),)).into_iter();
	let routing_value_layouts: Vec<usize> = routing_generic
		.as_ref()
		.map(|generic| routing_value_indices(&struct_regular_fields, generic))
		.unwrap_or_default();
	let routing_in_params = routing_value_layouts.iter().map(|index| {
		let slot = format_ident!("__in_{index}");
		quote!(#slot: &gcore::record::Layout,)
	});
	let routing_in_inits = routing_value_layouts.iter().map(|index| {
		let slot = format_ident!("__in_{index}");
		quote!(#slot: #slot.clone(),)
	});
	let flip_layout_params = flip
		.then(|| {
			(0..struct_regular_fields.len()).map(|index| {
				let slot = format_ident!("__in_{index}");
				quote!(#slot: &gcore::record::Layout,)
			})
		})
		.into_iter()
		.flatten();
	let flip_layout_inits = flip
		.then(|| {
			(0..struct_regular_fields.len()).map(|index| {
				let slot = format_ident!("__in_{index}");
				quote!(#slot: #slot.clone(),)
			})
		})
		.into_iter()
		.flatten();
	let flip_prelude = flip
		.then(|| match carrier_flip {
			true => quote! {
				let __layout = __in_0.with_writes(__in_0.depth, gcore::record::element_write::<#slot_value_type>(), &[]);
				let __plan = gcore::record::copy_plan(__in_0, &__layout, false, &[]);
				let __frame_bytes = __layout.frame_bytes();
			},
			false => quote! {
				let __layout = gcore::record::Layout::default().with_writes(0, gcore::record::element_write::<#slot_value_type>(), &[]);
				let __frame_bytes = __layout.frame_bytes();
			},
		})
		.into_iter();
	let flip_read_bindings = flip
		.then(|| {
			lazy_read_fields(&struct_regular_fields).into_iter().map(|(index, field)| {
				let arr = format_ident!("__reads_{index}");
				let slot = format_ident!("__in_{index}");
				let offsets = field.attribute_reads.iter().map(|read| {
					let marker = &read.marker;
					quote!(#slot.offset_of(<#marker as gcore::attribute::Attribute>::NAME, 0))
				});
				quote!(let #arr = [#(#offsets),*];)
			})
		})
		.into_iter()
		.flatten();
	let flip_read_inits = flip
		.then(|| {
			lazy_read_fields(&struct_regular_fields).into_iter().map(|(index, _)| {
				let arr = format_ident!("__reads_{index}");
				quote!(#arr,)
			})
		})
		.into_iter()
		.flatten();
	let flip_output_inits = flip
		.then(|| {
			let plan = carrier_flip.then(|| quote!(__plan,));
			match flip_generic_idents.is_empty() {
				true => quote!(__layout, __frame_bytes, #plan),
				false => quote!(__layout, __frame_bytes, #plan __marker: ::core::marker::PhantomData,),
			}
		})
		.into_iter();
	// The flip prelude's `element_write` instantiates the erased glue at the
	// output type, so `new` carries the bounds the glue needs.
	let new_where = flip
		.then(|| {
			let existing = parsed.where_clause.iter().flat_map(|clause| clause.predicates.iter());
			quote!(where #(#existing,)* #slot_value_type: ::core::clone::Clone + ::core::marker::Send + ::core::marker::Sync + 'static)
		})
		.into_iter();
	let new_impl = match !record_io {
		true => quote! {
			#[automatically_derived]
			impl<'n, #(#struct_generic_params,)*> #struct_name<#(#struct_type_params,)*> #(#new_where)*
			{
				#[allow(clippy::too_many_arguments)]
				pub fn new(#(#new_args,)* #(#routing_layout_param)* #(#routing_in_params)* #(#flip_layout_params)*) -> Self {
					#(#flip_prelude)*
					#(#flip_read_bindings)*
					Self {
						#(#all_field_inits,)*
						#(#routing_layout_init)*
						#(#routing_in_inits)*
						#(#flip_layout_inits)*
						#(#flip_read_inits)*
						#(#flip_output_inits)*
					}
				}
			}
		},
		false => quote!(),
	};

	let import_name = format_ident!("_IMPORT_STUB_{}", mod_name.to_string().to_case(Case::UpperSnake));
	let mut plan = generate_node_impl(
		crate_ident,
		parsed,
		&model,
		NodeFields {
			data_fields: data_fields.clone(),
			regular_fields: struct_regular_fields.clone(),
			node_generics: node_generics.clone(),
			data_field_generic_idents: data_field_generic_idents.clone(),
			struct_type_params: struct_type_params.clone(),
		},
	)?;
	plan.struct_item = quote! {
		#struct_derives
		pub struct #struct_name<#(#struct_generic_params,)*> {
			#(#struct_fields,)*
		}
	};
	plan.value_ctor = new_impl;
	let NodePlan {
		struct_item,
		value_ctor,
		kernel,
		lazy_read_fns,
		record_ctor,
		node_impl,
		entries,
	} = plan;
	let entries_name = format_ident!("{}_entries", parsed.fn_name);
	let register_entries = match entries.is_empty() {
		true => quote!(),
		false => quote!(gcore::registry::NODE_REGISTRY.lock().unwrap().entry(#identifier()).or_default().extend(#entries_name());),
	};

	let properties = &attributes.properties_string.as_ref().map(|value| quote!(Some(#value))).unwrap_or(quote!(None));
	let memoize_flag = attributes.memoize;
	let inject_scope_flag = attributes.inject_scope;

	let cfg = crate::shader_nodes::modify_cfg(attributes);
	let node_input_accessor = generate_node_input_references(parsed, fn_generics, &field_idents, core_types, &identifier, &cfg);
	let ShaderTokens { shader_entry_point, gpu_node } = attributes.shader_node.as_ref().map(|n| n.codegen(crate_ident, parsed)).unwrap_or(Ok(ShaderTokens::default()))?;

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
		#kernel

		#lazy_read_fns

		#record_ctor

		#node_impl

		#cfg
		const fn #identifier() -> #core_types::ProtoNodeIdentifier {
			#core_types::ProtoNodeIdentifier::new(std::concat!(#identifier_path, "::", std::stringify!(#struct_name)))
		}

		#cfg
		#[doc(inline)]
		pub use #mod_name::#struct_name;

		#[doc(hidden)]
		#node_input_accessor

		#cfg
		#[doc(hidden)]
		#[allow(clippy::module_inception)]
		mod #mod_name {
			use super::*;
			use #core_types as gcore;
			use gcore::{ContextFeature, concrete};
			use gcore::registry::{NodeMetadata, FieldMetadata, NODE_METADATA, RegistryValueSource, RegistryWidgetOverride};
			use gcore::ctor::ctor;

			// Use the types specified in the implementation

			static #import_name: core::marker::PhantomData<(#(#all_implementation_types,)*)> = core::marker::PhantomData;

			#struct_item

			#value_ctor

			#entries

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
					async_source_fields: #async_source,
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
				#register_entries
			}
		}

		#shader_entry_point

		#gpu_node
	})
}

/// Check if a type contains a reference to a specific identifier (e.g., a generic type parameter)
pub(crate) fn type_contains_ident(ty: &Type, ident: &Ident) -> bool {
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

/// The generated items of one node, produced uniformly across classes and
/// stitched by the assembling `quote!` at the end of [`generate_node_code`].
/// `struct_item` and `value_ctor` are filled by the caller; the rest come from
/// [`generate_node_impl`].
#[derive(Default)]
pub(crate) struct NodePlan {
	pub(crate) struct_item: TokenStream2,
	pub(crate) value_ctor: TokenStream2,
	pub(crate) kernel: TokenStream2,
	pub(crate) lazy_read_fns: TokenStream2,
	pub(crate) record_ctor: TokenStream2,
	pub(crate) node_impl: TokenStream2,
	pub(crate) entries: TokenStream2,
}

/// The field and generic derivation shared by the struct/metadata side and the
/// impl side, computed once in [`generate_node_code`] and passed to
/// [`generate_node_impl`]. `regular_fields` is the carrier-skipped slice both
/// sides agree on.
pub(crate) struct NodeFields<'a> {
	pub(crate) data_fields: Vec<&'a ParsedField>,
	pub(crate) regular_fields: Vec<&'a ParsedField>,
	pub(crate) node_generics: Vec<Ident>,
	pub(crate) data_field_generic_idents: Vec<Ident>,
	pub(crate) struct_type_params: Vec<Ident>,
}

/// Rewrites a creator kernel's `emit(a, b, ..)` tail into the row tuple `(a, b, ..)`.
/// `emit`'s parentheses double as the tuple constructor, so it is pure sugar.
fn rewrite_emit(body: &TokenStream2) -> TokenStream2 {
	let Ok(mut block) = syn::parse2::<syn::Block>(body.clone()) else {
		return body.clone();
	};
	struct EmitToTuple;
	impl VisitMut for EmitToTuple {
		fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
			if let syn::Expr::Call(call) = expr
				&& matches!(&*call.func, syn::Expr::Path(path) if path.path.is_ident("emit"))
			{
				*expr = syn::Expr::Tuple(syn::ExprTuple {
					attrs: Vec::new(),
					paren_token: Default::default(),
					elems: call.args.clone(),
				});
			}
			syn::visit_mut::visit_expr_mut(self, expr);
		}
	}
	EmitToTuple.visit_block_mut(&mut block);
	block.to_token_stream()
}

pub(crate) fn generate_node_impl(crate_ident: &CrateIdent, parsed: &ParsedNodeFn, model: &Option<Dialect>, fields: NodeFields) -> syn::Result<NodePlan> {
	let core_types = crate_ident.gcore()?;

	let ctx_param = context_param(parsed);
	let ctx_ident = match ctx_param {
		Some(ctx_param) => ctx_param.ident.clone(),
		None => format_ident!("__Ctx"),
	};
	let Some(model) = model.as_ref() else {
		return Ok(NodePlan::default());
	};
	let async_fn = matches!(*model, Dialect::AsyncFn);
	let future_kernel = matches!(*model, Dialect::Future | Dialect::FutureInterrupt);
	let async_source = async_fn || future_kernel;
	let node = crate::codegen::ir::build(parsed);
	let kind = crate::codegen::ir::node_kind(&node);
	let carrier_present = node.inputs.first().is_some_and(|input| input.subject);
	let flip = matches!(kind, crate::codegen::ir::NodeKind::Flip);
	let carrier_flip = flip && carrier_present;
	let opaque = matches!(kind, crate::codegen::ir::NodeKind::Opaque);
	let record_io = matches!(kind, crate::codegen::ir::NodeKind::RecordIo);
	let routing_generic = match (kind, &node.output.shape.element) {
		(crate::codegen::ir::NodeKind::Routing, crate::codegen::ir::Element::Generic(ident)) => Some(ident.clone()),
		_ => None,
	};
	let record_token = match (kind, &node.output.shape.element) {
		(crate::codegen::ir::NodeKind::RecordIo, crate::codegen::ir::Element::Generic(ident)) => Some(ident.clone()),
		_ => None,
	};
	let skips_carrier = record_io && !carrier_present;
	// The record-io write set, resolved from the output item and carrier input.
	let write_markers: Vec<&Type> = node.output.shape.attrs.iter().map(|attr| &attr.marker).collect();
	let removes: Vec<&Type> = node.output.removes.iter().map(|attr| &attr.marker).collect();
	let element_write: Option<&Type> = match &node.output.shape.element {
		crate::codegen::ir::Element::Concrete(ty) => Some(ty),
		_ => None,
	};
	let carrier_read_ty: Option<&Type> = node.inputs.first().filter(|input| input.subject).and_then(|input| match &input.shape.element {
		crate::codegen::ir::Element::Concrete(ty) => Some(ty),
		_ => None,
	});
	// A creator pushes rank levels: its `IList` return raises the output depth
	// above its carrier's, so the layout writes one level deeper.
	let subject_depth = node.inputs.iter().find(|input| input.subject).map_or(0, |input| input.shape.depth);
	let level_delta = node.output.shape.depth as i8 - subject_depth as i8;
	let pushed_levels = level_delta.max(0) as u8;
	let output_row = crate::codegen::ir::strip_ilist(&slot_value_type(&parsed.output_type)).0;
	let snapshot_ctx = async_fn && matches!(&parsed.input.ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "CtxSnapshot"));

	let mut ctx_bounds: Vec<TokenStream2> = match ctx_param {
		Some(ctx_param) => ctx_param
			.bounds
			.iter()
			.filter_map(|bound| match bound {
				TypeParamBound::Lifetime(_) => None,
				bound => Some(desugar_extract_lifetime(bound, core_types)),
			})
			.collect(),
		None => vec![quote!(#core_types::Ctx)],
	};
	if async_source && !snapshot_ctx {
		ctx_bounds.push(quote!(#core_types::context::DeriveCtx));
	}
	if snapshot_ctx {
		ctx_bounds.extend([
			quote!(#core_types::context::DeriveCtx),
			quote!(#core_types::context::ExtractFootprint),
			quote!(#core_types::context::ExtractRealTime),
			quote!(#core_types::context::ExtractAnimationTime),
			quote!(#core_types::context::ExtractPointerPosition),
			quote!(#core_types::context::ExtractIndex),
			quote!(#core_types::context::ExtractPosition),
		]);
	}

	let derives = ctx_param.is_some_and(|ctx_param| {
		ctx_param.bounds.iter().any(|bound| match bound {
			TypeParamBound::Trait(trait_bound) => trait_bound.path.segments.last().is_some_and(|segment| segment.ident == "DeriveCtx"),
			_ => false,
		})
	});
	let derive_routing = derives && routing_generic.is_some();

	let ctx_generic = match ctx_bounds.is_empty() {
		true => quote!(#ctx_ident),
		false => quote!(#ctx_ident: #(#ctx_bounds)+*),
	};
	let generic_tokens = |param: &GenericParam| match param {
		GenericParam::Type(type_param) if Some(&type_param.ident) == ctx_param.map(|ctx_param| &ctx_param.ident) => ctx_generic.clone(),
		param => quote!(#param),
	};
	let mut generics: Vec<TokenStream2> = parsed
		.fn_generics
		.iter()
		.filter(|param| match param {
			GenericParam::Type(type_param) => !derive_routing || Some(&type_param.ident) != routing_generic.as_ref(),
			_ => true,
		})
		.map(|param| match param {
			// Flipped kernels clone bare-typed elements out of their records,
			// so those generics carry the bound wire values satisfy; a
			// generic only nested in a field's type stays as declared.
			GenericParam::Type(type_param)
				if flip
					&& Some(&type_param.ident) != ctx_param.map(|ctx_param| &ctx_param.ident)
					&& parsed.fields.iter().filter(|field| !field.is_data_field).any(|field| {
						let ty = match &field.ty {
							ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
							ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
						};
						matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.get_ident() == Some(&type_param.ident))
					}) =>
			{
				let mut bounded = type_param.clone();
				bounded.bounds.push(syn::parse_quote!(::core::clone::Clone));
				quote!(#bounded)
			}
			param => generic_tokens(param),
		})
		.collect();
	let mut impl_generics: Vec<TokenStream2> = parsed
		.fn_generics
		.iter()
		.filter(|param| match param {
			GenericParam::Type(type_param) => {
				Some(&type_param.ident) != routing_generic.as_ref() && Some(&type_param.ident) != record_token.as_ref()
			}
			_ => true,
		})
		.map(&generic_tokens)
		.collect();
	if ctx_param.is_none() {
		generics.push(ctx_generic.clone());
		impl_generics.push(ctx_generic);
	}
	if routing_generic.is_some() || record_io || flip {
		impl_generics.insert(0, quote!('__record));
	}
	if derive_routing {
		generics.insert(0, quote!('__record));
	}

	let fn_name = &parsed.fn_name;
	let mod_name = format_ident!("_{}_mod", parsed.mod_name);
	let struct_name = format_ident!("{}Node", parsed.struct_name);
	let output_type = &parsed.output_type;
	let trait_output = match (record_io, &routing_generic) {
		(true, _) | (false, Some(_)) => syn::parse_quote!(#core_types::record::RecordValue<'__record>),
		(false, None) if flip => syn::parse_quote!(#core_types::record::RecordValue<'__record>),
		(false, None) => slot_value_type(&parsed.output_type),
	};
	let raw_lazy = matches!(*model, Dialect::Poll);
	let injected_name = |ident: &Ident| async_source && (ident == "_runtime" || ident == "_source");
	let where_predicates: Vec<TokenStream2> = parsed.where_clause.iter().flat_map(|clause| clause.predicates.iter()).map(|predicate| quote!(#predicate)).collect();

	let NodeFields {
		data_fields,
		regular_fields,
		node_generics,
		data_field_generic_idents,
		struct_type_params,
	} = fields;

	if derive_routing {
		for (index, field) in regular_fields.iter().enumerate() {
			let source_ty = match &field.ty {
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
			};
			if matches!((&routing_generic, source_ty), (Some(generic), Type::Path(path)) if path.path.get_ident() == Some(generic)) {
				let source_generic = format_ident!("__Source{index}");
				generics.push(quote! {
					#source_generic: for<'__derived> #core_types::record::DerivedRecordEdge<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>
				});
			}
		}
	}
	if flip {
		let mut kernel_lazy = false;
		for (index, field) in regular_fields.iter().enumerate() {
			if matches!(&field.ty, ParsedFieldType::Node(_)) {
				kernel_lazy = true;
				let source_generic = format_ident!("__Source{index}");
				let derived_extra = derives
					.then(|| quote!(+ for<'__derived> #core_types::record::RecordEdge<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>))
					.into_iter();
				generics.push(quote! {
					#source_generic: #core_types::node::Node<#ctx_ident, Output = #core_types::record::RecordValue<'__record>> #(#derived_extra)*
				});
			}
		}
		if kernel_lazy {
			generics.insert(0, quote!('__record));
		}
	}
	if opaque {
		for (index, field) in regular_fields.iter().enumerate() {
			if let ParsedFieldType::Node(NodeParsedField { output_type, .. }) = &field.ty {
				let source_generic = format_ident!("__Source{index}");
				generics.push(quote!(#source_generic: #core_types::node::Node<#ctx_ident, Output = #output_type>));
			}
		}
	}

	let data_names: Vec<&Ident> = data_fields.iter().map(|field| &field.pat_ident.ident).collect();
	let data_params = data_fields.iter().map(|field| {
		let pat = &field.pat_ident;
		let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
			unreachable!("data fields are regular types");
		};
		quote!(#pat: &#ty)
	});

	let lazy_bound = |output_type: &Type| match derives {
		true => quote!(for<'__derived> #core_types::node::Node<#core_types::context::Derived<'__derived, #ctx_ident>, Output = #output_type>),
		false => quote!(#core_types::node::Node<#ctx_ident, Output = #output_type>),
	};

	let routing_source = |ty: &Type| matches!((&routing_generic, ty), (Some(generic), Type::Path(path)) if path.path.get_ident() == Some(generic));

	let lazy_read_out = |field: &ParsedField, output_type: &Type| {
		let attr_tys = field.attribute_reads.iter().map(|read| {
			let marker = &read.marker;
			quote!(#core_types::attribute::Attr<#marker>)
		});
		match field.attribute_reads.is_empty() {
			true => quote!(#output_type),
			false => quote!((#output_type #(, #attr_tys)*)),
		}
	};
	let read_tuple_param = |field: &ParsedField, value_param: TokenStream2, value_ty: TokenStream2| {
		let read_pats = field.attribute_reads.iter().map(|read| &read.pat_ident);
		let read_tys = field.attribute_reads.iter().map(|read| {
			let marker = &read.marker;
			quote!(#core_types::attribute::Attr<#marker>)
		});
		quote!((#value_param #(, #read_pats)*): (#value_ty #(, #read_tys)*))
	};
	let kernel_params = regular_fields
		.iter()
		.enumerate()
		.filter(|(_, field)| !injected_name(&field.pat_ident.ident))
		.map(|(index, field)| {
			let pat = &field.pat_ident;
			match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(#pat: &#ty),
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) if !field.attribute_reads.is_empty() => read_tuple_param(field, quote!(#pat), quote!(#ty)),
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#pat: #ty),
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
					let source_generic = format_ident!("__Source{index}");
					match (ir::lazy_binding(&node, index), raw_lazy) {
						(LazyBinding::DeriveRouting, _) => quote!(#pat: #core_types::record::RecordLazyInput<'_, '__record, #source_generic>),
						(LazyBinding::OpaqueRecord, _) => quote!(#pat: &#core_types::record::RecordEdgeInput<'_, #source_generic>),
						(LazyBinding::Element, true) => {
							let out = lazy_read_out(field, output_type);
							quote!(#pat: &#core_types::record::ElementEdge<'_, #out, #source_generic>)
						}
						(LazyBinding::Element, false) => {
							let out = lazy_read_out(field, output_type);
							quote!(#pat: #core_types::record::ElementLazyInput<'_, #out, #source_generic>)
						}
						(LazyBinding::Plain, true) => {
							let bound = lazy_bound(output_type);
							quote!(#pat: &impl #bound)
						}
						(LazyBinding::Plain, false) => {
							let bound = lazy_bound(output_type);
							quote!(#pat: #core_types::node::LazyInput<'_, impl #bound>)
						}
					}
				}
			}
		});

	let record_value_ty: Type = syn::parse_quote!(#core_types::record::RecordValue<'__record>);
	let node_bounds = regular_fields.iter().enumerate().zip(&node_generics).map(|((index, field), node_generic)| match &field.ty {
		ParsedFieldType::Regular(_) if flip => quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>),
		ParsedFieldType::Node(_) if flip => match derives {
			true => quote! {
				#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>,
				#node_generic: for<'__derived> #core_types::record::RecordEdge<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>
			},
			false => quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>),
		},
		ParsedFieldType::Regular(_) if record_io && !skips_carrier && index == 0 => {
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>)
		}
		ParsedFieldType::Regular(_) if record_io && !field.attribute_reads.is_empty() => {
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>)
		}
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) if routing_source(ty) => {
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>)
		}
		ParsedFieldType::Regular(_) if routing_generic.is_some() => {
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>)
		}
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #ty>),
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) if routing_source(output_type) => match derives {
			true => quote!(#node_generic: for<'__derived> #core_types::record::DerivedRecordEdge<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>),
			false => {
				let bound = lazy_bound(&record_value_ty);
				quote!(#node_generic: #bound)
			}
		},
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) if opaque && is_record_value(output_type) => {
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #output_type>)
		}
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
			let bound = lazy_bound(output_type);
			quote!(#node_generic: #bound)
		}
	});

	let mut lend_outlives: Vec<TokenStream2> = Vec::new();
	if let Type::Reference(reference) = &trait_output
		&& let Some(lifetime) = &reference.lifetime
	{
		let inner = &reference.elem;
		lend_outlives.push(quote!(#inner: #lifetime));
	}

	// The slot persists the plain value even on record wires, so the Clone
	// bound targets the slot type, not the (possibly lifted) trait output.
	let slot_ty = slot_value_type(&parsed.output_type);
	let mut async_bounds = match (async_fn, future_kernel) {
		(false, false) => Vec::new(),
		(false, true) => vec![quote!(#slot_ty: Clone)],
		(true, _) => {
			let output_clone = std::iter::once(quote!(#slot_ty: Clone));
			let value_clones = regular_fields.iter().filter_map(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some(quote!(#ty: Clone)),
				_ => None,
			});
			let data_clones = data_fields.iter().filter_map(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some(quote!(#ty: Clone)),
				_ => None,
			});
			output_clone.chain(value_clones).chain(data_clones).collect()
		}
	};
	if async_source {
		async_bounds.push(quote!(for<'__derived> #core_types::context::Derived<'__derived, #ctx_ident>: #core_types::CacheHash));
	}

	let clampable_bounds = regular_fields.iter().filter_map(|field| {
		let ParsedFieldType::Regular(RegularParsedField {
			ty, number_hard_min, number_hard_max, ..
		}) = &field.ty
		else {
			return None;
		};
		(number_hard_min.is_some() || number_hard_max.is_some()).then(|| quote!(#ty: #core_types::misc::Clampable))
	});

	let flat_reads = field_reads(&regular_fields);
	let read_binding = |slot: usize, read: &AttributeRead, rec: TokenStream2| {
		let pat = &read.pat_ident;
		let marker = &read.marker;
		let slot = format_ident!("__read_{slot}");
		quote! {
			let #pat = #core_types::attribute::Attr::<#marker>(match self.#slot {
				Some(__offset) => unsafe { #rec.read(__offset) },
				None => <#marker as #core_types::attribute::Attribute>::default(),
			});
		}
	};
	let reads_of = |field_index: usize| {
		flat_reads
			.iter()
			.enumerate()
			.filter(move |(_, (owner, _))| *owner == field_index)
			.map(|(slot, (_, read))| (slot, *read))
			.collect::<Vec<(usize, &AttributeRead)>>()
	};

	let bind_body = |index: usize, field: &ParsedField| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => match ir::value_binding(&node, index) {
				// A carrier primary evaluates beyond the node's own frame (in the
				// record/flip tail), so it does not bind here.
				ValueBinding::Carrier => quote!(),
				// A reading secondary input claims a record edge: the element and
				// the declared reads copy out right after its eval, before any
				// later sibling eval can reuse the record stack.
				ValueBinding::ReadingSecondary => {
					let slot = format_ident!("__in_{index}");
					let rec_local = format_ident!("__rec_{index}");
					let bindings: Vec<TokenStream2> = reads_of(index).into_iter().map(|(slot, read)| read_binding(slot, read, quote!(#rec_local))).collect();
					quote! {
						let #name = match __cell.eval_input(#index, &self.#name, __input) {
							Ok(value) => value,
							Err(interrupt) => return interrupt.into(),
						};
						let #rec_local = self.#slot.rec(&#name);
						#(#bindings)*
						let #name: #ty = unsafe { #core_types::record::read_element(#rec_local) };
					}
				}
				// The lend input's frame survives on the record stack until this
				// node's frame is reclaimed, so the borrow stays valid in place.
				ValueBinding::Lend => {
					let slot = format_ident!("__in_{index}");
					let record_local = format_ident!("__record_{index}");
					quote! {
						let #record_local = match __cell.eval_input(#index, &self.#name, __input) {
							Ok(value) => value,
							Err(interrupt) => return interrupt.into(),
						};
						let #name = unsafe { #core_types::record::borrow_element::<#ty>(self.#slot.rec(&#record_local)) };
					}
				}
				// A flip value or a routing non-source value rides a record edge; the
				// element copies out into `name`. The mark/rewind that reclaims the
				// record's frame is applied by the step lowering (see `reads_out`).
				ValueBinding::RecordElement => {
					let slot = format_ident!("__in_{index}");
					quote! {
						let #name = match __cell.eval_input(#index, &self.#name, __input) {
							Ok(value) => value,
							Err(interrupt) => return interrupt.into(),
						};
						let #name: #ty = unsafe { #core_types::record::read_element(self.#slot.rec(&#name)) };
					}
				}
				ValueBinding::Plain => quote! {
					let #name = match __cell.eval_input(#index, &self.#name, __input) {
						Ok(value) => value,
						Err(interrupt) => return interrupt.into(),
					};
				},
			},
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => match (ir::lazy_binding(&node, index), raw_lazy) {
				// A raw poll edge is threaded straight through, so it does not bind here.
				(LazyBinding::Plain, true) => quote!(),
				(LazyBinding::DeriveRouting, _) => quote! {
					let #name = #core_types::record::RecordLazyInput::new(&self.#name, &__cell, #index);
				},
				(LazyBinding::Element, true) => {
					let slot = format_ident!("__in_{index}");
					match field.attribute_reads.is_empty() {
						true => quote! {
							let #name = #core_types::record::ElementEdge::<#output_type, _>::new(&self.#name, &self.#slot);
						},
						false => {
							let arr = format_ident!("__reads_{index}");
							let read_fn = format_ident!("__{}_read_{}", fn_name, index);
							quote! {
								let #name = #core_types::record::ElementEdge::with_reads(&self.#name, &self.#slot, &self.#arr, self::#read_fn);
							}
						}
					}
				}
				(LazyBinding::Element, false) => {
					let slot = format_ident!("__in_{index}");
					match field.attribute_reads.is_empty() {
						true => quote! {
							let #name = #core_types::record::ElementLazyInput::<#output_type, _>::new(&self.#name, &__cell, #index, &self.#slot);
						},
						false => {
							let arr = format_ident!("__reads_{index}");
							let read_fn = format_ident!("__{}_read_{}", fn_name, index);
							quote! {
								let #name = #core_types::record::ElementLazyInput::with_reads(&self.#name, &__cell, #index, &self.#slot, &self.#arr, self::#read_fn);
							}
						}
					}
				}
				(LazyBinding::OpaqueRecord, _) => quote! {
					let #name = #core_types::record::RecordEdgeInput::new(&self.#name, &self.__layout);
				},
				(LazyBinding::Plain, false) => quote! {
					let #name = #core_types::node::LazyInput::new(&self.#name, &__cell, #index);
				},
			},
		}
	};

	let clamp_tokens = |field: &ParsedField| {
		let ParsedFieldType::Regular(RegularParsedField { number_hard_min, number_hard_max, .. }) = &field.ty else {
			return None;
		};
		let name = &field.pat_ident.ident;
		let mut tokens = quote!();
		if let Some(min) = number_hard_min {
			tokens.extend(quote!(let #name = #core_types::misc::Clampable::clamp_hard_min(#name, #min);));
		}
		if let Some(max) = number_hard_max {
			tokens.extend(quote!(let #name = #core_types::misc::Clampable::clamp_hard_max(#name, #max);));
		}
		(!tokens.is_empty()).then_some(tokens)
	};

	let call_args = regular_fields.iter().enumerate().filter(|(_, field)| !injected_name(&field.pat_ident.ident)).map(|(index, field)| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			// A lend param binds an owned edge; the kernel borrows the
			// evaluated value.
			ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }) if !flip => quote!(&#name),
			ParsedFieldType::Regular(_) => quote!(#name),
			ParsedFieldType::Node(_) => match (ir::lazy_binding(&node, index), raw_lazy) {
				(LazyBinding::Element, true) | (LazyBinding::OpaqueRecord, _) => quote!(&#name),
				(LazyBinding::Plain, true) => quote!(&self.#name),
				_ => quote!(#name),
			},
		}
	});

	// The extent override is the leveled `extent_at`; consumers query the
	// composite `extent(ctx, Level)`, which the trait derives from it. A node
	// without `extent = fn` keeps the scalar default (one item at every level).
	let extent_impl = match &parsed.attributes.extent {
		Some(path) => quote! {
			fn extent_at(&self, __input: &#ctx_ident, __level: u8) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent> {
				#path(self, __input, __level)
			}
		},
		None => quote!(),
	};

	let serialize_impl = match &parsed.attributes.serialize {
		Some(path) => {
			let data_refs = data_names.iter().map(|name| quote!(&self.#name));
			quote! {
				fn serialize(&self) -> Option<::std::sync::Arc<dyn ::std::any::Any + Send + Sync>> {
					#path(#(#data_refs),*)
				}
			}
		}
		None => quote!(),
	};

	let batch_impl = match &parsed.attributes.batch {
		Some(path) => quote! {
			fn eval_batch<'__batch>(
				&'__batch self,
				__input: &'__batch #ctx_ident,
				__range: ::std::ops::Range<u64>,
				__scratch: Option<&'__batch mut [::std::mem::MaybeUninit<Self::Output>]>,
			) -> #core_types::node::BatchStatus<'__batch, Self::Output>
			where
				#ctx_ident: #core_types::context::InjectIndex + Copy,
			{
				#path(self, __input, __range, __scratch)
			}
		},
		None => quote!(),
	};

	let ctx_pat = &parsed.input.pat_ident;
	let fn_where = &parsed.where_clause;
	let body = if level_delta > 0 { rewrite_emit(&parsed.body) } else { parsed.body.clone() };
	let vis = &parsed.vis;
	let kernel_fields: Vec<&&ParsedField> = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).collect();
	// A bare `Attr<M>` in the return type cannot elide its lifetime, so the
	// kernel gets a fresh one; reference-valued writes name their real
	// lifetime explicitly and pass through untouched.
	let kernel_output = record_io.then(|| inject_attr_lifetimes(&crate::codegen::ir::strip_ilist(&parsed.output_type).0)).flatten();
	let attr_lifetime = kernel_output.is_some().then(|| quote!('__attr,));
	let kernel_output = match derive_routing {
		true => {
			let generic = routing_generic.as_ref().expect("derive routing implies routing");
			let ty = substitute_routing_record(&parsed.output_type, generic, core_types);
			quote!(#ty)
		}
		false => kernel_output.map(|ty| quote!(#ty)).unwrap_or_else(|| quote!(#output_type)),
	};
	let kernel = match async_fn {
		false => quote! {
			#[allow(clippy::too_many_arguments)]
			#vis fn #fn_name<#attr_lifetime #(#generics,)*>(#ctx_pat: &#ctx_ident #(, #data_params)* #(, #kernel_params)*) -> #kernel_output #fn_where #body
		},
		true => {
			let kernel_generics = parsed.fn_generics.iter().filter(|param| match param {
				GenericParam::Type(type_param) => Some(&type_param.ident) != ctx_param.map(|ctx_param| &ctx_param.ident),
				_ => true,
			});
			let snapshot_param = snapshot_ctx.then(|| quote!(#ctx_pat: #core_types::context::CtxSnapshot)).into_iter();
			let data_kernel_params = data_fields.iter().map(|field| {
				let pat = &field.pat_ident;
				let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
					unreachable!("data fields are regular types");
				};
				quote!(#pat: #ty)
			});
			let value_kernel_params = kernel_fields.iter().map(|field| {
				let pat = &field.pat_ident;
				let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
					unreachable!("async source fields are eager values");
				};
				quote!(#pat: #ty)
			});
			let params = snapshot_param.chain(data_kernel_params).chain(value_kernel_params);
			quote! {
				#[allow(clippy::too_many_arguments)]
				#vis async fn #fn_name<#(#kernel_generics,)*>(#(#params),*) -> #output_type #fn_where #body
			}
		}
	};
	let cell_constructor = match parsed.attributes.no_partial {
		true => quote!(#core_types::node::StatusCell::no_partial()),
		false => quote!(#core_types::node::StatusCell::new()),
	};
	let kernel_call = quote!(self::#fn_name(__input #(, &self.#data_names)* #(, #call_args)*));
	let lift = match *model {
		Dialect::Interrupt => quote! {
			match #kernel_call {
				Ok(value) => __cell.finish(value),
				Err(interrupt) => interrupt.into(),
			}
		},
		Dialect::Poll => quote!(__cell.merge(#kernel_call)),
		_ => quote!(__cell.finish(#kernel_call)),
	};

	let placeholder_value_names: Vec<&Ident> = kernel_fields
		.iter()
		.filter(|field| matches!(field.ty, ParsedFieldType::Regular(_)))
		.map(|field| &field.pat_ident.ident)
		.collect();
	// A carried tail claims the node's frame first, evaluates the carrier
	// beyond it, and carries its fields; every exit closes the frame through
	// `lift_poll_into`.
	let carried_prelude = carrier_flip.then(|| {
		let field = regular_fields[0];
		let name = &field.pat_ident.ident;
		let read = match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => {
				quote!(let #name: &#ty = unsafe { #core_types::record::borrow_element(__src_rec) };)
			}
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(let #name: #ty = unsafe { #core_types::record::read_element(__src_rec) };),
			_ => unreachable!("a flip carrier is a regular value input"),
		};
		let clamp = clamp_tokens(field);
		quote! {
			let mut __carried = #core_types::record::RecordValue::zeroed();
			let __dst = match self.__frame_bytes {
				0 => __carried.as_mut_ptr(),
				__bytes => #core_types::record::stack::push(__bytes),
			};
			let __src = match __cell.eval_input(0, &self.#name, __input) {
				Ok(value) => value,
				Err(interrupt) => return interrupt.into(),
			};
			let __src_rec = self.__in_0.rec(&__src);
			unsafe { #core_types::record::apply_plan(__src_rec, __dst, &self.__plan) };
			#read
			#clamp
		}
	});
	// Async slots persist plain values across evaluations; a flipped source
	// lifts the slot value onto its record wire at every merge point, into
	// the carried frame when the node has a carrier.
	let merge_lifted = |poll: TokenStream2| match (flip, carrier_flip) {
		(true, true) => {
			quote!(__cell.merge(unsafe { #core_types::record::lift_poll_into(#poll, __dst, self.__frame_bytes, #core_types::context::ExtractArena::arena(__input)) }))
		}
		(true, false) => quote!(__cell.merge(#core_types::record::lift_poll(#poll, &self.__layout, #core_types::context::ExtractArena::arena(__input)))),
		(false, _) => quote!(__cell.merge(#poll)),
	};
	let pending_return = match flip && carrier_flip {
		true => quote! {
			unsafe { #core_types::record::lift_poll_into::<#slot_ty>(#core_types::gpoll::GPoll::Pending, __dst, self.__frame_bytes, #core_types::context::ExtractArena::arena(__input)) }
		},
		false => quote!(#core_types::gpoll::GPoll::Pending),
	};
	let inflight = match &parsed.attributes.placeholder {
		Some(path) => merge_lifted(quote!(#core_types::gpoll::GPoll::Partial(#path(#(&#placeholder_value_names),*)))),
		None => pending_return.clone(),
	};
	let slot_hit = merge_lifted(quote!(value.clone()));
	let slot_check = quote! {
		let __scope = #core_types::context::DeriveCtx::scope(__input).excluding(_source);
		let __key = #core_types::registry::cache_key(&#core_types::context::DeriveCtx::with_scope(__input, &__scope));
		{
			let __entries = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
			if let Some(__state) = __entries.get(&__key) {
				return match __state {
					Some(value) => #slot_hit,
					None => #inflight,
				};
			}
		}
	};
	let future_completion = |payload: &Type| match kernel_kind(payload) {
		KernelKind::Poll(_) => quote!(__future.await),
		KernelKind::Interrupt(_) => quote! {
			match __future.await {
				Ok(value) => #core_types::gpoll::GPoll::Final(value),
				Err(interrupt) => interrupt.into(),
			}
		},
		_ => quote!(#core_types::gpoll::GPoll::Final(__future.await)),
	};
	let spawned_hit = merge_lifted(quote!(__value.clone()));
	let spawn_tail = |completion: TokenStream2, fallback: TokenStream2| {
		let spawned_hit = spawned_hit.clone();
		quote! {
			self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, None);
			let __slot = std::sync::Arc::clone(&self.slot);
			if _runtime.0.spawn(_source, Box::pin(async move {
				let __value = #completion;
				__slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, Some(__value));
			})) {
				let __entries = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
				if let Some(Some(__value)) = __entries.get(&__key) {
					return #spawned_hit;
				}
			}
			#fallback
		}
	};
	let record_tail = record_io.then(|| {
		let tuple_arg = |field: &ParsedField, value: TokenStream2| match field.attribute_reads.is_empty() {
			true => value,
			false => {
				let read_pats = field.attribute_reads.iter().map(|read| &read.pat_ident.ident);
				quote!((#value #(, #read_pats)*))
			}
		};
		let carrier_arg = if skips_carrier {
			None
		} else if let Some(ty) = carrier_read_ty {
			Some(tuple_arg(regular_fields[0], quote!(unsafe { #core_types::record::read_element::<#ty>(__src_rec) })))
		} else {
			Some(tuple_arg(regular_fields[0], quote!(#core_types::record::ElToken)))
		}
		.into_iter();
		let value_args = regular_fields.iter().skip(if skips_carrier { 0 } else { 1 }).map(|field| {
			let name = &field.pat_ident.ident;
			match &field.ty {
				// A lend param binds an owned edge; the kernel borrows the
				// evaluated value.
				ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }) => quote!(&#name),
				_ => tuple_arg(field, quote!(#name)),
			}
		});
		let record_kernel_call = quote!(self::#fn_name(__input #(, &self.#data_names)* #(, #carrier_arg)* #(, #value_args)*));
		let carrier_eval = (!skips_carrier).then(|| {
			let name = &regular_fields[0].pat_ident.ident;
			quote! {
				let __src = match __cell.eval_input(0, &self.#name, __input) {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				};
				let __src_rec = self.__carrier.rec(&__src);
			}
		});
		let carry = (!skips_carrier).then(|| quote!(unsafe { #core_types::record::apply_plan(__src_rec, __dst, &self.__plan) };));
		let carrier_read_bindings: Vec<TokenStream2> = match skips_carrier {
			true => Vec::new(),
			false => reads_of(0).into_iter().map(|(slot, read)| read_binding(slot, read, quote!(__src_rec))).collect(),
		};
		let kernel_value = match *model {
			Dialect::Interrupt => quote! {
				match #record_kernel_call {
					Ok(__value) => __value,
					Err(__interrupt) => return __interrupt.into(),
				}
			},
			_ => quote!(#record_kernel_call),
		};
		let attr_binders: Vec<Ident> = (0..write_markers.len()).map(|index| format_ident!("__attr_{index}")).collect();
		let element_binder = match element_write {
			Some(_) => quote!(__element),
			None => quote!(_),
		};
		// Slot binders in the return tuple's own order: an `Attr` binds the
		// next write binder, a `RemoveAttr` binds nothing.
		let slot_binders: Vec<TokenStream2> = {
			let mut binders = attr_binders.iter();
			match output_row.clone() {
				Type::Tuple(tuple) => tuple
					.elems
					.iter()
					.skip(1)
					.map(|slot| match attr_marker(slot) {
						Some(_) => {
							let binder = binders.next().expect("write binders match the Attr slots");
							quote!(#core_types::attribute::Attr(#binder))
						}
						None => quote!(_),
					})
					.collect(),
				_ => Vec::new(),
			}
		};
		let destructure = match slot_binders.is_empty() {
			true => quote!(let #element_binder = __kernel_value;),
			false => quote!(let (#element_binder #(, #slot_binders)*) = __kernel_value;),
		};
		let element_store = element_write.map(|ty| quote!(unsafe { #core_types::record::write_field::<#ty>(__dst, 0, __element) };));
		let attr_stores = attr_binders.iter().enumerate().map(|(index, binder)| {
			let slot = format_ident!("__write_{index}");
			quote!(unsafe { #core_types::record::write_field(__dst, self.#slot, #binder) };)
		});
		quote! {
			let mut __value = #core_types::record::RecordValue::zeroed();
			let __dst = match self.__frame_bytes {
				0 => __value.as_mut_ptr(),
				__bytes => #core_types::record::stack::push(__bytes),
			};
			#carrier_eval
			#carry
			#(#carrier_read_bindings)*
			let __kernel_value = #kernel_value;
			#destructure
			#element_store
			#(#attr_stores)*
			if self.__frame_bytes != 0 {
				#core_types::record::stack::truncate_above(__dst, self.__frame_bytes);
				__value = #core_types::record::RecordValue::spilled(unsafe { #core_types::record::Rec::new(__dst.cast_const()) });
			}
			__cell.finish(__value)
		}
	});
	let flip_tail = flip.then(|| {
		if matches!(*model, Dialect::Poll) {
			return match &carried_prelude {
				Some(prelude) => quote! {
					#prelude
					__cell.merge(unsafe { #core_types::record::lift_poll_into(#kernel_call, __dst, self.__frame_bytes, #core_types::context::ExtractArena::arena(__input)) })
				},
				None => quote! {
					let mut __scratch = #core_types::record::RecordValue::zeroed();
					let __dst = match self.__frame_bytes {
						0 => __scratch.as_mut_ptr(),
						__bytes => #core_types::record::stack::push(__bytes),
					};
					__cell.merge(unsafe { #core_types::record::lift_poll_into(#kernel_call, __dst, self.__frame_bytes, #core_types::context::ExtractArena::arena(__input)) })
				},
			};
		}
		let kernel_value = match *model {
			Dialect::Interrupt => quote! {
				match #kernel_call {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				}
			},
			_ => quote!(#kernel_call),
		};
		match &carried_prelude {
			// The carrier evaluates beyond the claimed frame, so the kernel
			// runs after the push.
			Some(prelude) => quote! {
				#prelude
				let __kernel_value = #kernel_value;
				__cell.merge(unsafe {
					#core_types::record::lift_poll_into(#core_types::gpoll::GPoll::Final(__kernel_value), __dst, self.__frame_bytes, #core_types::context::ExtractArena::arena(__input))
				})
			},
			None => quote! {
				let mut __scratch = #core_types::record::RecordValue::zeroed();
				let __dst = match self.__frame_bytes {
					0 => __scratch.as_mut_ptr(),
					__bytes => #core_types::record::stack::push(__bytes),
				};
				let __kernel_value = #kernel_value;
				__cell.merge(unsafe { #core_types::record::lift_poll_into(#core_types::gpoll::GPoll::Final(__kernel_value), __dst, self.__frame_bytes, #core_types::context::ExtractArena::arena(__input)) })
			},
		}
	});
	let tail_form = if async_fn {
		Tail::SpawnAsyncFn
	} else if future_kernel {
		Tail::SpawnFuture
	} else {
		match ir::node_kind(&node) {
			ir::NodeKind::RecordIo => Tail::Record,
			ir::NodeKind::Flip => Tail::Flip,
			ir::NodeKind::Routing | ir::NodeKind::Opaque => Tail::Forward,
		}
	};
	let lower_tail = |form: Tail| match form {
		Tail::Forward => lift.clone(),
		Tail::Record => record_tail.clone().expect("a record-io node has a record tail"),
		Tail::Flip => flip_tail.clone().expect("a flip node has a flip tail"),
		Tail::SpawnAsyncFn => {
			let kernel_value_names: Vec<&Ident> = kernel_fields.iter().map(|field| &field.pat_ident.ident).collect();
			let snapshot_binding = snapshot_ctx.then(|| quote!(let __snapshot = #core_types::context::CtxSnapshot::capture(__input);)).into_iter();
			let snapshot_arg = snapshot_ctx.then(|| quote!(__snapshot)).into_iter();
			let future_args = snapshot_arg
				.chain(data_names.iter().map(|name| quote!(self.#name.clone())))
				.chain(kernel_value_names.iter().map(|name| quote!(#name.clone())));
			let completion = future_completion(&parsed.output_type);
			let tail = spawn_tail(completion, inflight.clone());
			let prelude = carried_prelude.iter();
			quote! {
				#(#prelude)*
				#slot_check
				#(#snapshot_binding)*
				let __future = self::#fn_name(#(#future_args),*);
				#tail
			}
		}
		Tail::SpawnFuture => {
			let (placeholder_binding, spawn_return) = match &parsed.attributes.placeholder {
				Some(path) => (
					quote!(let __placeholder = #path(#(&#placeholder_value_names),*);),
					merge_lifted(quote!(#core_types::gpoll::GPoll::Partial(__placeholder))),
				),
				None => (quote!(), pending_return.clone()),
			};
			let acquire = match *model {
				Dialect::FutureInterrupt => quote! {
					let __future = match #kernel_call {
						Ok(future) => future,
						Err(interrupt) => return interrupt.into(),
					};
				},
				_ => quote!(let __future = #kernel_call;),
			};
			let payload = match kernel_kind(&parsed.output_type) {
				KernelKind::Future(payload) | KernelKind::FutureInterrupt(payload) => payload,
				_ => unreachable!("guarded by future_kernel"),
			};
			let completion = future_completion(&payload);
			let tail = spawn_tail(completion, spawn_return);
			let prelude = carried_prelude.iter();
			quote! {
				#(#prelude)*
				#slot_check
				#placeholder_binding
				#acquire
				#tail
			}
		}
	};

	let record_bounds: Vec<TokenStream2> = {
		let arena_bound = (record_io && skips_carrier) || (!record_io && (derive_routing || flip));
		let mut bounds = if arena_bound {
			vec![quote!(#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__record #core_types::arena::Arena>)]
		} else {
			Vec::new()
		};
		// A reading secondary input's element copies out of its record, as
		// does a concrete carrier read.
		if record_io {
			bounds.extend(reading_secondary_indices(&regular_fields, skips_carrier).into_iter().filter_map(|index| match &regular_fields[index].ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some(quote!(#ty: ::core::clone::Clone)),
				_ => None,
			}));
			if let Some(ty) = carrier_read_ty {
				bounds.push(quote!(#ty: ::core::clone::Clone));
			}
		}
		// A routing node's value elements copy out of their records.
		if let Some(generic) = &routing_generic {
			bounds.extend(routing_value_indices(&regular_fields, generic).into_iter().filter_map(|index| match &regular_fields[index].ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some(quote!(#ty: ::core::clone::Clone)),
				_ => None,
			}));
		}
		bounds
	};

	let flip_bounds: Vec<TokenStream2> = match flip {
		true => {
			let mut bounds: Vec<TokenStream2> = regular_fields
				.iter()
				.filter_map(|field| match &field.ty {
					// The conditional arena-park moves a lend element once.
					ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => Some(quote!(#ty: ::core::marker::Send + ::core::marker::Sync + 'static)),
					ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some(quote!(#ty: ::core::clone::Clone)),
					ParsedFieldType::Node(NodeParsedField { output_type, .. }) => Some(quote!(#output_type: ::core::clone::Clone)),
				})
				.collect();
			let out = slot_value_type(&parsed.output_type);
			bounds.push(quote!(#out: ::core::marker::Send + ::core::marker::Sync + 'static));
			bounds
		}
		false => Vec::new(),
	};

	let record_layout_impl = match record_io || routing_generic.is_some() || flip || opaque {
		true => quote! {
			fn layout(&self) -> &#core_types::record::Layout {
				&self.__layout
			}
		},
		false => quote!(),
	};

	let entries = entries_tokens(parsed, &struct_name, &data_field_generic_idents, &regular_fields);
	let cfg = crate::shader_nodes::modify_cfg(&parsed.attributes);

	let record_wiring = record_io.then(|| {
		let layout_fn = format_ident!("{}_layout", fn_name);
		let write_descs: Vec<TokenStream2> = write_markers
			.iter()
			.map(|marker| quote!(#core_types::record::FieldWrite::of::<#marker>(0)))
			.collect();
		let remove_pairs: Vec<TokenStream2> = removes
			.iter()
			.map(|marker| quote!((<#marker as #core_types::attribute::Attribute>::NAME, 0)))
			.collect();
		let subtraction = (!remove_pairs.is_empty()).then(|| quote!(.without(&[#(#remove_pairs),*])));
		let element = match element_write {
			Some(ty) => quote!(#core_types::record::element_write::<#ty>()),
			None => quote!(__carrier.element),
		};
		let layout_def = match skips_carrier {
			true => quote! {
				#vis fn #layout_fn() -> #core_types::record::Layout {
					#core_types::record::Layout::default().with_writes(0, #element, &[#(#write_descs),*])
				}
			},
			false => quote! {
				#vis fn #layout_fn(__carrier: &#core_types::record::Layout) -> #core_types::record::Layout {
					__carrier #subtraction.with_writes(__carrier.depth + #pushed_levels, #element, &[#(#write_descs),*])
				}
			},
		};
		let layout_meta_fn = format_ident!("{}_layout_meta", fn_name);
		let element_spec = match element_write {
			Some(ty) => quote!(#core_types::record::ElementSpec::Concrete(#core_types::record::element_write::<#ty>())),
			None => quote!(#core_types::record::ElementSpec::Carried),
		};
		let layout_meta = crate::codegen::ir::layout_meta_tokens(&node, element_spec, core_types);
		let layout_meta_def = quote! {
			#vis fn #layout_meta_fn() -> #core_types::record::LayoutMeta {
				#layout_meta
			}
		};
		let reading_secondaries = reading_secondary_indices(&regular_fields, skips_carrier);
		let edge_args = regular_fields.iter().zip(&node_generics).map(|(field, generic)| {
			let name = &field.pat_ident.ident;
			quote!(#name: #generic)
		});
		let carrier_layout_param = (!skips_carrier).then(|| quote!(__carrier_layout: &#core_types::record::Layout,)).into_iter();
		let input_layout_params = reading_secondaries.iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(#slot: &#core_types::record::Layout,)
		});
		let layout_binding = match skips_carrier {
			true => quote!(let __layout = self::#layout_fn();),
			false => quote!(let __layout = self::#layout_fn(__carrier_layout);),
		};
		let carry_element = element_write.is_none();
		let plan_binding =
			(!skips_carrier).then(|| quote!(let __plan = #core_types::record::copy_plan(__carrier_layout, &__layout, #carry_element, &[#(#remove_pairs),*]);));
		let read_inits = flat_reads.iter().enumerate().map(|(slot, (owner, read))| {
			let marker = &read.marker;
			let slot = format_ident!("__read_{slot}");
			let source = match !skips_carrier && *owner == 0 {
				true => quote!(__carrier_layout),
				false => format_ident!("__in_{owner}").to_token_stream(),
			};
			quote!(let #slot = #source.offset_of(<#marker as #core_types::attribute::Attribute>::NAME, 0);)
		});
		let write_inits = write_markers.iter().enumerate().map(|(index, marker)| {
			let slot = format_ident!("__write_{index}");
			quote! {
				let #slot = __layout
					.offset_of(<#marker as #core_types::attribute::Attribute>::NAME, 0)
					.expect("a written attribute is always part of the wired layout");
			}
		});
		let data_inits = data_names.iter().map(|name| quote!(#name: ::core::default::Default::default(),));
		let edge_inits = regular_fields.iter().map(|field| {
			let name = &field.pat_ident.ident;
			quote!(#name,)
		});
		let carrier_init = (!skips_carrier).then(|| quote!(__carrier: __carrier_layout.clone(),)).into_iter();
		let input_layout_inits = reading_secondaries.iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(#slot: #slot.clone(),)
		});
		let plan_init = (!skips_carrier).then(|| quote!(__plan,)).into_iter();
		let read_names = (0..flat_reads.len()).map(|index| format_ident!("__read_{index}")).map(|slot| quote!(#slot,));
		let write_names = (0..write_markers.len()).map(|index| format_ident!("__write_{index}")).map(|slot| quote!(#slot,));
		quote! {
			#layout_def
			#layout_meta_def

			#[automatically_derived]
			impl<#(#data_field_generic_idents,)* #(#node_generics,)*> #mod_name::#struct_name<#(#struct_type_params,)*> {
				#[allow(clippy::too_many_arguments)]
				#vis fn new(#(#edge_args,)* #(#carrier_layout_param)* #(#input_layout_params)*) -> Self {
					#layout_binding
					#plan_binding
					#(#read_inits)*
					#(#write_inits)*
					let __frame_bytes = __layout.frame_bytes();
					Self {
						#(#data_inits)*
						#(#edge_inits)*
						#(#carrier_init)*
						#(#input_layout_inits)*
						__layout,
						#(#plan_init)*
						__frame_bytes,
						#(#read_names)*
						#(#write_names)*
					}
				}
			}
		}
	});

	// An inline node only leaks if an input spilled a frame. Flip nodes store
	// each input's layout, so gate precisely; inline record-io nodes are rare
	// (attributes usually spill the output) and their value-input layouts are
	// not all stored, so guard them whenever inline.
	let reclaim_active = match flip {
		true => {
			let spilled_inputs = (0..regular_fields.len()).map(|index| {
				let slot = format_ident!("__in_{index}");
				quote!(self.#slot.frame_bytes() != 0)
			});
			quote!(self.__frame_bytes == 0 && (false #(|| #spilled_inputs)*))
		}
		false => quote!(self.__frame_bytes == 0),
	};
	let reclaim_guard = (flip || record_io).then(|| {
		quote! {
			// SAFETY: an inline node returns its output by value, so nothing above the entry pointer is live when the guard rewinds.
			let __reclaim_guard = unsafe { #core_types::record::ReclaimGuard::new(#reclaim_active) };
		}
	});

	// The eval body as an ordered step sequence: bind each input, clamp, then the
	// tail. The record-stack mark/rewind of a read-out bind is applied here from
	// the role, so the discipline is structural rather than per-arm.
	let eval_steps: Vec<EvalStep> = regular_fields
		.iter()
		.enumerate()
		.map(|(index, field)| EvalStep::Bind(index, field))
		.chain(
			regular_fields
				.iter()
				.enumerate()
				.filter(|(index, _)| !(carrier_flip && *index == 0))
				.map(|(_, field)| EvalStep::Clamp(field)),
		)
		.chain(std::iter::once(EvalStep::Tail(tail_form)))
		.collect();
	let eval_body = eval_steps.iter().map(|step| match step {
		EvalStep::Bind(index, field) => {
			let body = bind_body(*index, field);
			let reads_out = matches!(&field.ty, ParsedFieldType::Regular(_)) && ir::value_binding(&node, *index).reads_out();
			match reads_out {
				false => body,
				true => {
					let mark = format_ident!("__mark_{index}");
					quote! {
						let #mark = #core_types::record::stack::sp();
						#body
						unsafe { #core_types::record::stack::rewind(#mark) };
					}
				}
			}
		}
		EvalStep::Clamp(field) => clamp_tokens(field).unwrap_or_default(),
		EvalStep::Tail(form) => lower_tail(*form),
	});

	let top_level = quote! {
		#cfg
		#[automatically_derived]
		impl<#(#impl_generics,)* #(#node_generics,)*> #core_types::node::Node<#ctx_ident> for #mod_name::#struct_name<#(#struct_type_params,)*>
		where
			#(#node_bounds,)*
			#(#lend_outlives,)*
			#(#clampable_bounds,)*
			#(#async_bounds,)*
			#(#record_bounds,)*
			#(#flip_bounds,)*
			#(#where_predicates,)*
		{
			type Output = #trait_output;

			fn eval(&self, __input: &#ctx_ident) -> #core_types::gpoll::GPoll<Self::Output> {
				let __cell = #cell_constructor;
				#reclaim_guard
				#(#eval_body)*
			}

			#extent_impl

			#serialize_impl

			#record_layout_impl

			#batch_impl
		}
	};

	let lazy_read_fns: Vec<TokenStream2> = lazy_read_fields(&regular_fields)
		.into_iter()
		.map(|(index, field)| {
			let ParsedFieldType::Node(NodeParsedField { output_type, .. }) = &field.ty else {
				unreachable!("lazy read fields are Node fields");
			};
			let read_fn = format_ident!("__{}_read_{}", fn_name, index);
			let generics: Vec<&Ident> = parsed
				.fn_generics
				.iter()
				.filter_map(|param| match param {
					GenericParam::Type(type_param) if type_contains_ident(output_type, &type_param.ident) => Some(&type_param.ident),
					_ => None,
				})
				.collect();
			let attr_slots = field.attribute_reads.iter().enumerate().map(|(slot, read)| {
				let marker = &read.marker;
				quote! {
					#core_types::attribute::Attr::<#marker>(match __reads[#slot] {
						Some(__offset) => unsafe { __rec.read(__offset) },
						None => <#marker as #core_types::attribute::Attribute>::default(),
					})
				}
			});
			let attr_tys = field.attribute_reads.iter().map(|read| {
				let marker = &read.marker;
				quote!(#core_types::attribute::Attr<'__read, #marker>)
			});
			quote! {
				/// # Safety
				/// `__rec` must be a record whose element is the declared output
				/// type, of the layout `__reads` was resolved against.
				unsafe fn #read_fn<'__read #(, #generics)*>(__rec: #core_types::record::Rec, __reads: &[Option<usize>]) -> (#output_type #(, #attr_tys)*)
				where
					#output_type: ::core::clone::Clone,
				{
					(unsafe { #core_types::record::read_element::<#output_type>(__rec) } #(, #attr_slots)*)
				}
			}
		})
		.collect();

	Ok(NodePlan {
		kernel,
		lazy_read_fns: quote!(#(#lazy_read_fns)*),
		record_ctor: quote!(#record_wiring),
		node_impl: top_level,
		entries,
		..Default::default()
	})
}
