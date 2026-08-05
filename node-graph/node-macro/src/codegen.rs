use crate::crate_ident::CrateIdent;
use crate::parsing::*;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use std::sync::atomic::AtomicU64;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{GenericArgument, GenericParam, Ident, Lifetime, PatIdent, PathArguments, Type, TypeParam, TypeParamBound};
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

	let record = record_shape(parsed);
	let routing = routing_io(parsed);
	let flip = record_flip(parsed);
	let record_skips_carrier = record.as_ref().is_some_and(|shape| shape.skips_carrier());
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

	// Combined struct type parameters: data field generic idents (T, U, ...) + node generics (Node0, Node1, ...)
	// For struct type instantiation: MemoizeNode<T, Node0>
	let struct_type_params: Vec<Ident> = data_field_generic_idents.iter().cloned().chain(node_generics.iter().cloned()).collect();

	// Combined struct generic parameters with bounds for struct definition
	// struct MemoizeNode<T: Clone, Node0>
	let struct_generic_params: Vec<TokenStream2> = data_field_generics.iter().map(|gp| quote!(#gp)).chain(node_generics.iter().map(|id| quote!(#id))).collect();
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

	let record_state_fields: Vec<TokenStream2> = match &record {
		Some(shape) => {
			let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout)];
			if !shape.skips_carrier() {
				state.push(quote!(pub(super) __carrier: gcore::record::Layout));
				state.push(quote!(pub(super) __plan: ::std::vec::Vec<(usize, usize, usize)>));
			}
			state.push(quote!(pub(super) __frame_bytes: usize));
			state.extend((0..parsed.attribute_reads.len()).map(|index| {
				let slot = format_ident!("__read_{index}");
				quote!(pub(super) #slot: Option<usize>)
			}));
			state.extend((0..shape.write_markers.len()).map(|index| {
				let slot = format_ident!("__write_{index}");
				quote!(pub(super) #slot: usize)
			}));
			state
		}
		None if routing.is_some() => vec![quote!(pub(super) __layout: gcore::record::Layout)],
		None if flip => {
			let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout), quote!(pub(super) __frame_bytes: usize)];
			state.extend((0..struct_regular_fields.len()).map(|index| {
				let slot = format_ident!("__in_{index}");
				quote!(pub(super) #slot: gcore::record::Layout)
			}));
			state
		}
		None => Vec::new(),
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
	let struct_derives = if record.is_some() || routing.is_some() || flip {
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
	let routing_layout_param = routing.is_some().then(|| quote!(__layout: &gcore::record::Layout,)).into_iter();
	let routing_layout_init = routing.is_some().then(|| quote!(__layout: __layout.clone(),)).into_iter();
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
		.then(|| {
			quote! {
				let __layout = gcore::record::Layout::default().with_writes(0, gcore::record::element_dims::<#slot_value_type>(), &[]);
				let __frame_bytes = __layout.frame_bytes();
			}
		})
		.into_iter();
	let flip_output_inits = flip.then(|| quote!(__layout, __frame_bytes,)).into_iter();
	let new_impl = match record.is_none() {
		true => quote! {
			#[automatically_derived]
			impl<'n, #(#struct_generic_params,)*> #struct_name<#(#struct_type_params,)*>
			{
				#[allow(clippy::too_many_arguments)]
				pub fn new(#(#new_args,)* #(#routing_layout_param)* #(#flip_layout_params)*) -> Self {
					#(#flip_prelude)*
					Self {
						#(#all_field_inits,)*
						#(#routing_layout_init)*
						#(#flip_layout_inits)*
						#(#flip_output_inits)*
					}
				}
			}
		},
		false => quote!(),
	};

	let import_name = format_ident!("_IMPORT_STUB_{}", mod_name.to_string().to_case(Case::UpperSnake));
	let node = generate_node_impl(crate_ident, parsed)?;
	let node_in_mod = node.in_mod;
	let node_top_level = node.top_level;
	let entries_name = format_ident!("{}_entries", parsed.fn_name);
	let register_entries = match node_in_mod.is_empty() {
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
		#node_top_level

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

			#struct_derives
			pub struct #struct_name<#(#struct_generic_params,)*> {
				#(#struct_fields,)*
			}

			#new_impl

			#node_in_mod

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

use crate::shader_nodes::{ShaderCodegen, ShaderTokens};
use syn::visit_mut::VisitMut;

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

pub(crate) struct NodeImplTokens {
	pub(crate) in_mod: TokenStream2,
	pub(crate) top_level: TokenStream2,
}

pub(crate) fn generate_node_impl(crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<NodeImplTokens> {
	let core_types = crate_ident.gcore()?;

	let ctx_param = context_param(parsed);
	let ctx_ident = match ctx_param {
		Some(ctx_param) => ctx_param.ident.clone(),
		None => format_ident!("__Ctx"),
	};
	let async_fn = parsed.is_async;
	let future_kernel = is_source_kernel(&parsed.output_type);
	let async_source = async_fn || future_kernel;
	if async_fn && parsed.fields.iter().any(|field| matches!(field.ty, ParsedFieldType::Node(_))) {
		return Ok(NodeImplTokens {
			in_mod: quote!(),
			top_level: quote!(),
		});
	}
	let record = record_shape(parsed);
	if record.is_none() && has_record_io(parsed) {
		return Ok(NodeImplTokens {
			in_mod: quote!(),
			top_level: quote!(),
		});
	}
	let record_token = match record.as_ref().map(|shape| &shape.carrier) {
		Some(RecordCarrier::Token(token)) => Some(token.clone()),
		_ => None,
	};
	let skips_carrier = record.as_ref().is_some_and(|shape| shape.skips_carrier());
	let routing = routing_io(parsed);
	let flip = record_flip(parsed);
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

	let has_lend = parsed.fields.iter().any(|field| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. })));
	let declared_arena_lifetime = ctx_param.and_then(|ctx_param| {
		ctx_param.bounds.iter().find_map(|bound| {
			let TypeParamBound::Trait(trait_bound) = bound else { return None };
			let segment = trait_bound.path.segments.last()?;
			if segment.ident != "ExtractArena" {
				return None;
			}
			let PathArguments::AngleBracketed(args) = &segment.arguments else { return None };
			match args.args.first() {
				Some(GenericArgument::Lifetime(lifetime)) => Some(lifetime.clone()),
				_ => None,
			}
		})
	});
	let introduced_lend_lifetime = (has_lend && declared_arena_lifetime.is_none()).then(|| Lifetime::new("'__lend", proc_macro2::Span::call_site()));
	let lend_lifetime = declared_arena_lifetime.or_else(|| introduced_lend_lifetime.clone());
	if let Some(lifetime) = &introduced_lend_lifetime {
		ctx_bounds.push(quote!(#core_types::context::ExtractArena<ArenaRef = &#lifetime #core_types::arena::Arena>));
	}

	let derives = ctx_param.is_some_and(|ctx_param| {
		ctx_param.bounds.iter().any(|bound| match bound {
			TypeParamBound::Trait(trait_bound) => trait_bound.path.segments.last().is_some_and(|segment| segment.ident == "DeriveCtx"),
			_ => false,
		})
	});
	let derive_routing = derives && routing.is_some();

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
			GenericParam::Type(type_param) => !derive_routing || Some(&type_param.ident) != routing.as_ref().map(|routing| &routing.generic),
			_ => true,
		})
		.map(&generic_tokens)
		.collect();
	let mut impl_generics: Vec<TokenStream2> = parsed
		.fn_generics
		.iter()
		.filter(|param| match param {
			GenericParam::Type(type_param) => {
				Some(&type_param.ident) != routing.as_ref().map(|routing| &routing.generic) && Some(&type_param.ident) != record_token.as_ref()
			}
			_ => true,
		})
		.map(&generic_tokens)
		.collect();
	if ctx_param.is_none() {
		generics.push(ctx_generic.clone());
		impl_generics.push(ctx_generic);
	}
	if let Some(lifetime) = &introduced_lend_lifetime {
		generics.insert(0, quote!(#lifetime));
		impl_generics.insert(0, quote!(#lifetime));
	}
	if routing.is_some() || record.is_some() || flip {
		impl_generics.insert(0, quote!('__record));
	}
	if derive_routing {
		generics.insert(0, quote!('__record));
	}

	let fn_name = &parsed.fn_name;
	let mod_name = format_ident!("_{}_mod", parsed.mod_name);
	let struct_name = format_ident!("{}Node", parsed.struct_name);
	let output_type = &parsed.output_type;
	let trait_output = match (&record, &routing) {
		(Some(_), _) | (None, Some(_)) => syn::parse_quote!(#core_types::record::RecordValue<'__record>),
		(None, None) if flip => syn::parse_quote!(#core_types::record::RecordValue<'__record>),
		(None, None) => slot_value_type(&parsed.output_type),
	};
	let raw_lazy = matches!(kernel_kind(&parsed.output_type), KernelKind::Poll(_));
	let injected_name = |ident: &Ident| async_source && (ident == "_runtime" || ident == "_source");
	let where_predicates: Vec<TokenStream2> = parsed.where_clause.iter().flat_map(|clause| clause.predicates.iter()).map(|predicate| quote!(#predicate)).collect();

	let (data_fields, regular_fields): (Vec<_>, Vec<_>) = parsed.fields.iter().partition(|field| field.is_data_field);
	let regular_fields: Vec<_> = regular_fields.into_iter().skip(skips_carrier as usize).collect();

	if derive_routing {
		for (index, field) in regular_fields.iter().enumerate() {
			let source_ty = match &field.ty {
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
			};
			if matches!((&routing, source_ty), (Some(routing), Type::Path(path)) if path.path.get_ident() == Some(&routing.generic)) {
				let source_generic = format_ident!("__Source{index}");
				generics.push(quote! {
					#source_generic: for<'__derived> #core_types::record::DerivedRecordEdge<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>
				});
			}
		}
	}

	let data_field_generic_idents: Vec<Ident> = parsed
		.fn_generics
		.iter()
		.filter_map(|generic| match generic {
			GenericParam::Type(type_param) => Some(type_param.ident.clone()),
			_ => None,
		})
		.filter(|ident| {
			data_fields.iter().any(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => crate::codegen::type_contains_ident(ty, ident),
				_ => false,
			})
		})
		.collect();

	let node_generics: Vec<Ident> = regular_fields.iter().enumerate().map(|(index, _)| format_ident!("Node{}", index)).collect();
	let struct_type_params: Vec<Ident> = data_field_generic_idents.iter().cloned().chain(node_generics.iter().cloned()).collect();

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

	let routing_source = |ty: &Type| matches!((&routing, ty), (Some(routing), Type::Path(path)) if path.path.get_ident() == Some(&routing.generic));

	let attr_kernel_params = parsed.attribute_reads.iter().map(|read| {
		let pat = &read.pat_ident;
		let marker = &read.marker;
		quote!(#pat: #core_types::attribute::Attr<#marker>)
	});
	let kernel_params = regular_fields
		.iter()
		.enumerate()
		.filter(|(_, field)| !injected_name(&field.pat_ident.ident))
		.map(|(index, field)| {
			let pat = &field.pat_ident;
			match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(#pat: &#ty),
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#pat: #ty),
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) if derive_routing && routing_source(output_type) => {
					let source_generic = format_ident!("__Source{index}");
					quote!(#pat: #core_types::record::RecordLazyInput<'_, '__record, #source_generic>)
				}
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) if raw_lazy => {
					let bound = lazy_bound(output_type);
					quote!(#pat: &impl #bound)
				}
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
					let bound = lazy_bound(output_type);
					quote!(#pat: #core_types::node::LazyInput<'_, impl #bound>)
				}
			}
		})
		.chain(attr_kernel_params);

	let record_value_ty: Type = syn::parse_quote!(#core_types::record::RecordValue<'__record>);
	let node_bounds = regular_fields.iter().enumerate().zip(&node_generics).map(|((index, field), node_generic)| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => {
			let lifetime = lend_lifetime.as_ref().expect("lend fields imply the lend lifetime");
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = &#lifetime #ty>)
		}
		ParsedFieldType::Regular(_) if record.is_some() && !skips_carrier && index == 0 => {
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>)
		}
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) if routing_source(ty) => {
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>)
		}
		ParsedFieldType::Regular(_) if flip => quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #record_value_ty>),
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #ty>),
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) if routing_source(output_type) => match derives {
			true => quote!(#node_generic: for<'__derived> #core_types::record::DerivedRecordEdge<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>),
			false => {
				let bound = lazy_bound(&record_value_ty);
				quote!(#node_generic: #bound)
			}
		},
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
			let bound = lazy_bound(output_type);
			quote!(#node_generic: #bound)
		}
	});

	let mut lend_outlives: Vec<TokenStream2> = regular_fields
		.iter()
		.filter_map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => {
				let lifetime = lend_lifetime.as_ref().expect("lend fields imply the lend lifetime");
				Some(quote!(#ty: #lifetime))
			}
			_ => None,
		})
		.collect();
	if let Type::Reference(reference) = &trait_output
		&& let Some(lifetime) = &reference.lifetime
	{
		let inner = &reference.elem;
		lend_outlives.push(quote!(#inner: #lifetime));
	}

	let mut async_bounds = match (async_fn, future_kernel) {
		(false, false) => Vec::new(),
		(false, true) => vec![quote!(#trait_output: Clone)],
		(true, _) => {
			let output_clone = std::iter::once(quote!(#trait_output: Clone));
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

	let eval_values = regular_fields.iter().enumerate().map(|(index, field)| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			ParsedFieldType::Regular(_) if record.is_some() && !skips_carrier && index == 0 => quote!(),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) if flip => {
				let slot = format_ident!("__in_{index}");
				quote! {
					let #name = match __cell.eval_input(#index, &self.#name, __input) {
						Ok(value) => value,
						Err(interrupt) => return interrupt.into(),
					};
					let #name: #ty = unsafe { #core_types::record::read_element(self.#slot.rec(&#name)) };
				}
			}
			ParsedFieldType::Regular(_) => quote! {
				let #name = match __cell.eval_input(#index, &self.#name, __input) {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				};
			},
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) if derive_routing && routing_source(output_type) => quote! {
				let #name = #core_types::record::RecordLazyInput::new(&self.#name, &__cell, #index);
			},
			ParsedFieldType::Node(_) if raw_lazy => quote!(),
			ParsedFieldType::Node(_) => quote! {
				let #name = #core_types::node::LazyInput::new(&self.#name, &__cell, #index);
			},
		}
	});

	let clamps = regular_fields.iter().filter_map(|field| {
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
	});

	let call_args = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).map(|field| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			ParsedFieldType::Node(_) if raw_lazy => quote!(&self.#name),
			_ => quote!(#name),
		}
	});

	let value_field_names: Vec<&Ident> = regular_fields
		.iter()
		.filter(|field| matches!(field.ty, ParsedFieldType::Regular(_)))
		.map(|field| &field.pat_ident.ident)
		.collect();

	let extent_impl = match &parsed.attributes.extent {
		Some(path) => quote! {
			fn extent(&self, __input: &#ctx_ident) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent> {
				#path(self, __input)
			}
		},
		None if value_field_names.is_empty() => quote!(),
		None => {
			let first = value_field_names[0];
			let mut meet = quote!(self.#first.extent(__input));
			for name in &value_field_names[1..] {
				meet = quote!(#core_types::gpoll::Extent::meet(#meet, self.#name.extent(__input)));
			}
			quote! {
				fn extent(&self, __input: &#ctx_ident) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent> {
					#meet
				}
			}
		}
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
				&self,
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
	let body = &parsed.body;
	let vis = &parsed.vis;
	let kernel_fields: Vec<&&ParsedField> = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).collect();
	// A bare `Attr<M>` in the return type cannot elide its lifetime, so the
	// kernel gets a fresh one; reference-valued writes name their real
	// lifetime explicitly and pass through untouched.
	let kernel_output = record.as_ref().and_then(|_| inject_attr_lifetimes(&parsed.output_type));
	let attr_lifetime = kernel_output.is_some().then(|| quote!('__attr,));
	let kernel_output = match derive_routing {
		true => {
			let generic = &routing.as_ref().expect("derive routing implies routing").generic;
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
	let lift = match kernel_kind(&parsed.output_type) {
		KernelKind::Interrupt(_) => quote! {
			match #kernel_call {
				Ok(value) => __cell.finish(value),
				Err(interrupt) => interrupt.into(),
			}
		},
		KernelKind::Poll(_) => quote!(__cell.merge(#kernel_call)),
		_ => quote!(__cell.finish(#kernel_call)),
	};

	let placeholder_value_names: Vec<&Ident> = kernel_fields
		.iter()
		.filter(|field| matches!(field.ty, ParsedFieldType::Regular(_)))
		.map(|field| &field.pat_ident.ident)
		.collect();
	let inflight = match &parsed.attributes.placeholder {
		Some(path) => quote!(__cell.merge(#core_types::gpoll::GPoll::Partial(#path(#(&#placeholder_value_names),*)))),
		None => quote!(#core_types::gpoll::GPoll::Pending),
	};
	let slot_check = quote! {
		let __scope = #core_types::context::DeriveCtx::scope(__input).excluding(_source);
		let __key = #core_types::registry::cache_key(&#core_types::context::DeriveCtx::with_scope(__input, &__scope));
		{
			let __entries = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
			if let Some(__state) = __entries.get(&__key) {
				return match __state {
					Some(value) => __cell.merge(value.clone()),
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
	let spawn_tail = |completion: TokenStream2, fallback: TokenStream2| {
		quote! {
			self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, None);
			let __slot = std::sync::Arc::clone(&self.slot);
			if _runtime.0.spawn(_source, Box::pin(async move {
				let __value = #completion;
				__slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, Some(__value));
			})) {
				let __entries = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
				if let Some(Some(__value)) = __entries.get(&__key) {
					return __cell.merge(__value.clone());
				}
			}
			#fallback
		}
	};
	let record_tail = record.as_ref().map(|shape| {
		let carrier_arg = match &shape.carrier {
			RecordCarrier::None => None,
			RecordCarrier::Token(_) => Some(quote!(#core_types::record::ElToken)),
			RecordCarrier::Read(ty) => Some(quote!(unsafe { __src_rec.element::<#ty>() })),
		}
		.into_iter();
		let value_args = regular_fields.iter().skip(if shape.skips_carrier() { 0 } else { 1 }).map(|field| {
			let name = &field.pat_ident.ident;
			quote!(#name)
		});
		let attr_args = parsed.attribute_reads.iter().map(|read| {
			let pat = &read.pat_ident.ident;
			quote!(#pat)
		});
		let record_kernel_call = quote!(self::#fn_name(__input #(, &self.#data_names)* #(, #carrier_arg)* #(, #value_args)* #(, #attr_args)*));
		let carrier_eval = (!shape.skips_carrier()).then(|| {
			let name = &regular_fields[0].pat_ident.ident;
			quote! {
				let __src = match __cell.eval_input(0, &self.#name, __input) {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				};
				let __src_rec = self.__carrier.rec(&__src);
			}
		});
		let carry = (!shape.skips_carrier()).then(|| quote!(unsafe { #core_types::record::apply_plan(__src_rec, __dst, &self.__plan) };));
		let read_bindings = parsed.attribute_reads.iter().enumerate().map(|(index, read)| {
			let pat = &read.pat_ident;
			let marker = &read.marker;
			let slot = format_ident!("__read_{index}");
			quote! {
				let #pat = #core_types::attribute::Attr::<#marker>(match self.#slot {
					Some(__offset) => unsafe { __src_rec.read(__offset) },
					None => <#marker as #core_types::attribute::Attribute>::default(),
				});
			}
		});
		let kernel_value = match shape.dialect {
			RecordDialect::Plain => quote!(#record_kernel_call),
			RecordDialect::Interrupt => quote! {
				match #record_kernel_call {
					Ok(__value) => __value,
					Err(__interrupt) => return __interrupt.into(),
				}
			},
		};
		let attr_binders: Vec<Ident> = (0..shape.write_markers.len()).map(|index| format_ident!("__attr_{index}")).collect();
		let element_binder = match &shape.element_write {
			Some(_) => quote!(__element),
			None => quote!(_),
		};
		let destructure = match attr_binders.is_empty() {
			true => quote!(let #element_binder = __kernel_value;),
			false => quote!(let (#element_binder #(, #core_types::attribute::Attr(#attr_binders))*) = __kernel_value;),
		};
		let element_store = shape.element_write.as_ref().map(|ty| quote!(unsafe { #core_types::record::write_field::<#ty>(__dst, 0, __element) };));
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
			#(#read_bindings)*
			let __kernel_value = #kernel_value;
			#destructure
			#element_store
			#(#attr_stores)*
			if self.__frame_bytes != 0 {
				#core_types::record::stack::pop(__dst);
				__value = #core_types::record::RecordValue::spilled(unsafe { #core_types::record::Rec::new(__dst.cast_const()) });
			}
			__cell.finish(__value)
		}
	});
	let flip_tail = flip.then(|| {
		let kernel_value = match kernel_kind(&parsed.output_type) {
			KernelKind::Interrupt(_) => quote! {
				match #kernel_call {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				}
			},
			_ => quote!(#kernel_call),
		};
		quote! {
			let __kernel_value = #kernel_value;
			let mut __value = #core_types::record::RecordValue::zeroed();
			let __dst = match self.__frame_bytes {
				0 => __value.as_mut_ptr(),
				__bytes => #core_types::record::stack::push(__bytes),
			};
			let __written = unsafe { #core_types::record::write_element(__dst, __kernel_value, #core_types::context::ExtractArena::arena(__input)) };
			if self.__frame_bytes != 0 {
				#core_types::record::stack::pop(__dst);
				__value = #core_types::record::RecordValue::spilled(unsafe { #core_types::record::Rec::new(__dst.cast_const()) });
			}
			match __written {
				Some(()) => __cell.finish(__value),
				None => #core_types::gpoll::GPoll::Error(::std::boxed::Box::new(#core_types::gpoll::GraphError {
					kind: #core_types::gpoll::ErrorKind::ArenaExhausted,
					trace: ::std::vec::Vec::new(),
				})),
			}
		}
	});
	let eval_tail = match (async_fn, future_kernel) {
		(false, false) => match record_tail {
			Some(tail) => tail,
			None => flip_tail.unwrap_or(lift),
		},
		(true, _) => {
			let kernel_value_names: Vec<&Ident> = kernel_fields.iter().map(|field| &field.pat_ident.ident).collect();
			let snapshot_binding = snapshot_ctx.then(|| quote!(let __snapshot = #core_types::context::CtxSnapshot::capture(__input);)).into_iter();
			let snapshot_arg = snapshot_ctx.then(|| quote!(__snapshot)).into_iter();
			let future_args = snapshot_arg
				.chain(data_names.iter().map(|name| quote!(self.#name.clone())))
				.chain(kernel_value_names.iter().map(|name| quote!(#name.clone())));
			let completion = future_completion(&parsed.output_type);
			let tail = spawn_tail(completion, inflight.clone());
			quote! {
				#slot_check
				#(#snapshot_binding)*
				let __future = self::#fn_name(#(#future_args),*);
				#tail
			}
		}
		(false, true) => {
			let (placeholder_binding, spawn_return) = match &parsed.attributes.placeholder {
				Some(path) => (
					quote!(let __placeholder = #path(#(&#placeholder_value_names),*);),
					quote!(__cell.merge(#core_types::gpoll::GPoll::Partial(__placeholder))),
				),
				None => (quote!(), quote!(#core_types::gpoll::GPoll::Pending)),
			};
			let acquire = match kernel_kind(&parsed.output_type) {
				KernelKind::FutureInterrupt(_) => quote! {
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
			quote! {
				#slot_check
				#placeholder_binding
				#acquire
				#tail
			}
		}
	};

	let record_bounds: Vec<TokenStream2> = match &record {
		Some(shape) if shape.skips_carrier() => {
			vec![quote!(#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__record #core_types::arena::Arena>)]
		}
		None if derive_routing || flip => {
			vec![quote!(#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__record #core_types::arena::Arena>)]
		}
		_ => Vec::new(),
	};

	let record_layout_impl = match record.is_some() || routing.is_some() || flip {
		true => quote! {
			fn layout(&self) -> Option<&#core_types::record::Layout> {
				Some(&self.__layout)
			}
		},
		false => quote!(),
	};

	let entries = entries_tokens(parsed, &struct_name, &data_field_generic_idents, &regular_fields);
	let cfg = crate::shader_nodes::modify_cfg(&parsed.attributes);

	let record_wiring = record.as_ref().map(|shape| {
		let layout_fn = format_ident!("{}_layout", fn_name);
		let write_descs: Vec<TokenStream2> = shape
			.write_markers
			.iter()
			.map(|marker| quote!(#core_types::record::FieldWrite::of::<#marker>(0)))
			.collect();
		let element_dims = match &shape.element_write {
			Some(ty) => quote!((::core::mem::size_of::<#ty>(), ::core::mem::align_of::<#ty>())),
			None => quote!((__carrier.element_size, __carrier.element_align)),
		};
		let layout_def = match shape.skips_carrier() {
			true => quote! {
				#vis fn #layout_fn() -> #core_types::record::Layout {
					#core_types::record::Layout::default().with_writes(0, #element_dims, &[#(#write_descs),*])
				}
			},
			false => quote! {
				#vis fn #layout_fn(__carrier: &#core_types::record::Layout) -> #core_types::record::Layout {
					__carrier.with_writes(__carrier.depth, #element_dims, &[#(#write_descs),*])
				}
			},
		};
		let edge_args = regular_fields.iter().zip(&node_generics).map(|(field, generic)| {
			let name = &field.pat_ident.ident;
			quote!(#name: #generic)
		});
		let carrier_layout_param = (!shape.skips_carrier()).then(|| quote!(__carrier_layout: &#core_types::record::Layout,)).into_iter();
		let layout_binding = match shape.skips_carrier() {
			true => quote!(let __layout = self::#layout_fn();),
			false => quote!(let __layout = self::#layout_fn(__carrier_layout);),
		};
		let carry_element = shape.carries_element();
		let plan_binding = (!shape.skips_carrier()).then(|| quote!(let __plan = #core_types::record::copy_plan(__carrier_layout, &__layout, #carry_element);));
		let read_inits = parsed.attribute_reads.iter().enumerate().map(|(index, read)| {
			let marker = &read.marker;
			let slot = format_ident!("__read_{index}");
			quote!(let #slot = __carrier_layout.offset_of(<#marker as #core_types::attribute::Attribute>::NAME, 0);)
		});
		let write_inits = shape.write_markers.iter().enumerate().map(|(index, marker)| {
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
		let carrier_init = (!shape.skips_carrier()).then(|| quote!(__carrier: __carrier_layout.clone(),)).into_iter();
		let plan_init = (!shape.skips_carrier()).then(|| quote!(__plan,)).into_iter();
		let read_names = (0..parsed.attribute_reads.len()).map(|index| format_ident!("__read_{index}")).map(|slot| quote!(#slot,));
		let write_names = (0..shape.write_markers.len()).map(|index| format_ident!("__write_{index}")).map(|slot| quote!(#slot,));
		quote! {
			#layout_def

			#[automatically_derived]
			impl<#(#data_field_generic_idents,)* #(#node_generics,)*> #mod_name::#struct_name<#(#struct_type_params,)*> {
				#[allow(clippy::too_many_arguments)]
				#vis fn new(#(#edge_args,)* #(#carrier_layout_param)*) -> Self {
					#layout_binding
					#plan_binding
					#(#read_inits)*
					#(#write_inits)*
					let __frame_bytes = __layout.frame_bytes();
					Self {
						#(#data_inits)*
						#(#edge_inits)*
						#(#carrier_init)*
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
			#(#where_predicates,)*
		{
			type Output = #trait_output;

			fn eval(&self, __input: &#ctx_ident) -> #core_types::gpoll::GPoll<Self::Output> {
				let __cell = #cell_constructor;
				#(#eval_values)*
				#(#clamps)*
				#eval_tail
			}

			#extent_impl

			#serialize_impl

			#record_layout_impl

			#batch_impl
		}
	};

	Ok(NodeImplTokens {
		in_mod: entries,
		top_level: quote! {
			#kernel

			#record_wiring

			#top_level
		},
	})
}

pub(crate) enum RecordDialect {
	Plain,
	Interrupt,
}

/// How a record node's primary input lowers.
pub(crate) enum RecordCarrier {
	/// `_: ()`: no carrier edge, the kernel writes a fresh record.
	None,
	/// An unbounded generic returned in the element position: the element
	/// bytes carry through the copy plan and the kernel sees `ElToken`.
	Token(Ident),
	/// An element type read at offset 0, monomorphized per its
	/// implementations list where generic.
	Read(Type),
}

/// The record io of a node fn: how the carrier lowers, the element write,
/// and the written markers. Present exactly when the signature declares
/// attribute reads or writes in a shape the record tier supports; malformed
/// record io is reported by validation and generates no node impl.
pub(crate) struct RecordShape {
	pub(crate) carrier: RecordCarrier,
	pub(crate) element_write: Option<Type>,
	pub(crate) write_markers: Vec<Type>,
	pub(crate) dialect: RecordDialect,
}

impl RecordShape {
	pub(crate) fn skips_carrier(&self) -> bool {
		matches!(self.carrier, RecordCarrier::None)
	}

	pub(crate) fn carries_element(&self) -> bool {
		self.element_write.is_none()
	}
}

pub(crate) fn has_record_io(parsed: &ParsedNodeFn) -> bool {
	!parsed.attribute_reads.is_empty() || record_writes(&slot_value_type(&parsed.output_type)).is_some()
}

/// Replaces the routing generic in a derive-routing kernel's return type with
/// the routing record value, since the kernel's edges rebind to '__record.
fn substitute_routing_record(output: &Type, generic: &Ident, core_types: &TokenStream2) -> Type {
	struct Subst<'a> {
		generic: &'a Ident,
		replacement: Type,
	}

	impl VisitMut for Subst<'_> {
		fn visit_type_mut(&mut self, ty: &mut Type) {
			if let Type::Path(path) = ty
				&& path.qself.is_none()
				&& path.path.get_ident() == Some(self.generic)
			{
				*ty = self.replacement.clone();
				return;
			}
			syn::visit_mut::visit_type_mut(self, ty);
		}
	}

	let mut ty = output.clone();
	let mut subst = Subst {
		generic,
		replacement: syn::parse_quote!(#core_types::record::RecordValue<'__record>),
	};
	subst.visit_type_mut(&mut ty);
	ty
}

fn inject_attr_lifetimes(output: &Type) -> Option<Type> {
	struct Injector {
		changed: bool,
	}

	impl VisitMut for Injector {
		fn visit_path_segment_mut(&mut self, segment: &mut syn::PathSegment) {
			if segment.ident == "Attr"
				&& let PathArguments::AngleBracketed(args) = &mut segment.arguments
				&& !args.args.iter().any(|arg| matches!(arg, GenericArgument::Lifetime(_)))
			{
				args.args.insert(0, GenericArgument::Lifetime(Lifetime::new("'__attr", proc_macro2::Span::call_site())));
				self.changed = true;
			}
			syn::visit_mut::visit_path_segment_mut(self, segment);
		}
	}

	let mut ty = output.clone();
	let mut injector = Injector { changed: false };
	injector.visit_type_mut(&mut ty);
	injector.changed.then_some(ty)
}

pub(crate) fn contains_open_generic(parsed: &ParsedNodeFn, ty: &Type) -> bool {
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	parsed
		.fn_generics
		.iter()
		.any(|param| matches!(param, GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref() && type_contains_ident(ty, &type_param.ident)))
}

pub(crate) fn unbounded_generic(parsed: &ParsedNodeFn, ty: &Type) -> Option<Ident> {
	let ident = bare_ident(ty)?.clone();
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	parsed
		.fn_generics
		.iter()
		.find(|param| matches!(param, GenericParam::Type(type_param) if type_param.ident == ident && type_param.bounds.is_empty() && Some(&type_param.ident) != ctx_ident.as_ref()))?;
	if let Some(where_clause) = &parsed.where_clause
		&& tokens_contain_ident(where_clause.to_token_stream(), &ident)
	{
		return None;
	}
	Some(ident)
}

pub(crate) fn record_shape(parsed: &ParsedNodeFn) -> Option<RecordShape> {
	let (value, dialect) = match kernel_kind(&parsed.output_type) {
		KernelKind::Plain => (parsed.output_type.clone(), RecordDialect::Plain),
		KernelKind::Interrupt(inner) => (inner, RecordDialect::Interrupt),
		_ => return None,
	};
	let writes = record_writes(&value);
	if parsed.attribute_reads.is_empty() && writes.is_none() {
		return None;
	}
	if parsed.is_async || parsed.fields.iter().any(|field| matches!(field.ty, ParsedFieldType::Node(_))) {
		return None;
	}
	let carrier_field = parsed.fields.first()?;
	if carrier_field.is_data_field {
		return None;
	}
	let ParsedFieldType::Regular(RegularParsedField { ty, lend: None, implementations, .. }) = &carrier_field.ty else {
		return None;
	};
	let carrier = match ty {
		Type::Tuple(tuple) if tuple.elems.is_empty() => RecordCarrier::None,
		ty => match implementations.is_empty().then(|| unbounded_generic(parsed, ty)).flatten() {
			Some(token) => RecordCarrier::Token(token),
			None => {
				if contains_open_generic(parsed, ty) {
					return None;
				}
				RecordCarrier::Read(ty.clone())
			}
		},
	};
	let (element, write_markers) = match writes {
		Some(RecordWrites { element, markers }) => (element, markers),
		None => (value, Vec::new()),
	};
	let element_write = match &carrier {
		RecordCarrier::Token(token) => match bare_ident(&element) {
			Some(ident) if ident == token => None,
			_ => return None,
		},
		_ => {
			if contains_open_generic(parsed, &element) {
				return None;
			}
			Some(element)
		}
	};
	if matches!(carrier, RecordCarrier::None) && !parsed.attribute_reads.is_empty() {
		return None;
	}
	Some(RecordShape {
		carrier,
		element_write,
		write_markers,
		dialect,
	})
}

pub(crate) fn is_poll_kernel(output: &Type) -> bool {
	matches!(kernel_kind(output), KernelKind::Poll(_))
}

/// A routing family: an unbounded generic shared by lazy inputs (and
/// optionally the first parameter) and returned whole, instantiated at
/// `RecordValue` so opaque records flow through the kernel. Detected only
/// when the family's fields carry no implementations lists, so the existing
/// per-type row spelling keeps its meaning.
pub(crate) struct RoutingIo {
	pub(crate) generic: Ident,
}

/// Whether a plain node's lowering flips onto record wires: sync,
/// fully-concrete value-input nodes in this cut; batch, shader, async, lend,
/// lazy, and generic nodes keep the plain lowering until their record forms
/// land.
pub(crate) fn record_flip(parsed: &ParsedNodeFn) -> bool {
	if record_shape(parsed).is_some() || has_record_io(parsed) || routing_io(parsed).is_some() {
		return false;
	}
	if parsed.is_async || is_source_kernel(&parsed.output_type) {
		return false;
	}
	if parsed.attributes.batch.is_some() || parsed.attributes.shader_node.is_some() {
		return false;
	}
	if matches!(kernel_kind(&parsed.output_type), KernelKind::Poll(_)) {
		return false;
	}
	if matches!(slot_value_type(&parsed.output_type), Type::Reference(_)) {
		return false;
	}
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	let concrete = parsed.fn_generics.iter().all(|param| match param {
		GenericParam::Type(type_param) => Some(&type_param.ident) == ctx_ident.as_ref(),
		GenericParam::Lifetime(_) => false,
		GenericParam::Const(_) => false,
	});
	if !concrete {
		return false;
	}
	parsed.fields.iter().all(|field| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { lend, .. }) => lend.is_none(),
		ParsedFieldType::Node(_) => false,
	})
}

pub(crate) fn routing_io(parsed: &ParsedNodeFn) -> Option<RoutingIo> {
	if has_record_io(parsed) || parsed.is_async {
		return None;
	}
	if !matches!(kernel_kind(&parsed.output_type), KernelKind::Plain | KernelKind::Interrupt(_)) {
		return None;
	}
	let value = slot_value_type(&parsed.output_type);
	let Type::Path(path) = &value else { return None };
	let ident = path.path.get_ident()?.clone();
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	parsed
		.fn_generics
		.iter()
		.find(|param| matches!(param, GenericParam::Type(type_param) if type_param.ident == ident && type_param.bounds.is_empty() && Some(&type_param.ident) != ctx_ident.as_ref()))?;
	if let Some(where_clause) = &parsed.where_clause
		&& tokens_contain_ident(where_clause.to_token_stream(), &ident)
	{
		return None;
	}
	let mut sources = 0;
	for (index, field) in parsed.fields.iter().enumerate() {
		match &field.ty {
			ParsedFieldType::Node(NodeParsedField {
				output_type,
				input_type,
				implementations,
			}) => {
				if bare_ident(output_type) == Some(&ident) {
					if !implementations.is_empty() || type_contains_ident(input_type, &ident) {
						return None;
					}
					sources += 1;
				} else if type_contains_ident(output_type, &ident) || type_contains_ident(input_type, &ident) {
					return None;
				}
			}
			ParsedFieldType::Regular(RegularParsedField { ty, implementations, lend, .. }) => {
				if bare_ident(ty) == Some(&ident) {
					if index != 0 || field.is_data_field || !implementations.is_empty() || lend.is_some() {
						return None;
					}
					sources += 1;
				} else if type_contains_ident(ty, &ident) {
					return None;
				}
			}
		}
	}
	(sources > 0).then(|| RoutingIo { generic: ident })
}

pub(crate) fn bare_ident(ty: &Type) -> Option<&Ident> {
	let Type::Path(path) = ty else { return None };
	path.path.get_ident()
}

fn tokens_contain_ident(tokens: TokenStream2, ident: &Ident) -> bool {
	tokens.into_iter().any(|token| match token {
		proc_macro2::TokenTree::Ident(candidate) => &candidate == ident,
		proc_macro2::TokenTree::Group(group) => tokens_contain_ident(group.stream(), ident),
		_ => false,
	})
}

pub(crate) fn slot_value_type(output: &Type) -> Type {
	match kernel_kind(output) {
		KernelKind::Plain => output.clone(),
		KernelKind::Poll(inner) | KernelKind::Interrupt(inner) => inner,
		KernelKind::Future(payload) | KernelKind::FutureInterrupt(payload) => match kernel_kind(&payload) {
			KernelKind::Poll(inner) | KernelKind::Interrupt(inner) => inner,
			_ => payload,
		},
	}
}

pub(crate) fn is_source_kernel(output: &Type) -> bool {
	matches!(kernel_kind(output), KernelKind::Future(_) | KernelKind::FutureInterrupt(_))
}

enum KernelKind {
	Plain,
	Interrupt(Type),
	Poll(Type),
	Future(Type),
	FutureInterrupt(Type),
}

fn source_future_payload(segment: &syn::PathSegment) -> Type {
	let PathArguments::AngleBracketed(args) = &segment.arguments else {
		return syn::parse_quote!(());
	};
	args.args
		.iter()
		.find_map(|argument| match argument {
			GenericArgument::Type(ty) => Some(ty.clone()),
			_ => None,
		})
		.unwrap_or_else(|| syn::parse_quote!(()))
}

fn kernel_kind(output: &Type) -> KernelKind {
	let plain = || KernelKind::Plain;
	let Type::Path(path) = output else { return plain() };
	let Some(segment) = path.path.segments.last() else { return plain() };
	match segment.ident.to_string().as_str() {
		"GPoll" => {
			let PathArguments::AngleBracketed(args) = &segment.arguments else { return plain() };
			let inner = args.args.iter().find_map(|argument| match argument {
				GenericArgument::Type(ty) => Some(ty.clone()),
				_ => None,
			});
			inner.map(KernelKind::Poll).unwrap_or_else(plain)
		}
		"SourceFuture" => KernelKind::Future(source_future_payload(segment)),
		"Result" => {
			let PathArguments::AngleBracketed(args) = &segment.arguments else { return plain() };
			let mut types = args.args.iter().filter_map(|argument| match argument {
				GenericArgument::Type(ty) => Some(ty),
				_ => None,
			});
			let (Some(inner), Some(Type::Path(error_path))) = (types.next(), types.next()) else {
				return plain();
			};
			if error_path.path.segments.last().is_none_or(|segment| segment.ident != "Interrupt") {
				return plain();
			}
			if let Type::Path(inner_path) = inner
				&& let Some(inner_segment) = inner_path.path.segments.last()
				&& inner_segment.ident == "SourceFuture"
			{
				return KernelKind::FutureInterrupt(source_future_payload(inner_segment));
			}
			KernelKind::Interrupt(inner.clone())
		}
		_ => plain(),
	}
}

fn context_param(parsed: &ParsedNodeFn) -> Option<&TypeParam> {
	let Type::Path(path) = &parsed.input.ty else {
		return None;
	};
	let ident = path.path.get_ident()?;
	parsed.fn_generics.iter().find_map(|param| match param {
		GenericParam::Type(type_param) if &type_param.ident == ident => Some(type_param),
		_ => None,
	})
}

fn type_disqualifies(ty: &Type) -> bool {
	struct Disqualifier {
		found: bool,
	}

	impl<'ast> Visit<'ast> for Disqualifier {
		fn visit_type_reference(&mut self, _: &'ast syn::TypeReference) {
			self.found = true;
		}

		fn visit_type_impl_trait(&mut self, _: &'ast syn::TypeImplTrait) {
			self.found = true;
		}

		fn visit_lifetime(&mut self, _: &'ast Lifetime) {
			self.found = true;
		}
	}

	let mut visitor = Disqualifier { found: false };
	visitor.visit_type(ty);
	visitor.found
}

fn desugar_extract_lifetime(bound: &TypeParamBound, core_types: &TokenStream2) -> TokenStream2 {
	let TypeParamBound::Trait(trait_bound) = bound else {
		return quote!(#bound);
	};
	let Some(segment) = trait_bound.path.segments.last() else {
		return quote!(#bound);
	};
	if segment.ident != "ExtractArena" {
		return quote!(#bound);
	}
	let PathArguments::AngleBracketed(args) = &segment.arguments else {
		return quote!(#bound);
	};
	if args.args.len() != 1 {
		return quote!(#bound);
	}
	let Some(GenericArgument::Lifetime(lifetime)) = args.args.first() else {
		return quote!(#bound);
	};
	quote!(#core_types::context::ExtractArena<ArenaRef = &#lifetime #core_types::arena::Arena>)
}

fn entries_tokens(parsed: &ParsedNodeFn, struct_name: &Ident, data_field_generic_idents: &[Ident], regular_fields: &[&ParsedField]) -> TokenStream2 {
	if !data_field_generic_idents.is_empty() {
		return quote!();
	}
	if has_record_io(parsed) {
		return record_entries_tokens(parsed, struct_name, regular_fields);
	}
	if routing_io(parsed).is_some() {
		return routing_entries_tokens(parsed, struct_name, regular_fields);
	}
	if record_flip(parsed) {
		return flip_entries_tokens(parsed, struct_name, regular_fields);
	}
	let Some(rows) = implementation_rows(parsed, regular_fields) else {
		return quote!();
	};
	let rows: Vec<&Vec<Type>> = rows.iter().filter(|row| row.iter().all(|ty| !type_disqualifies(ty))).collect();
	if rows.is_empty() {
		return quote!();
	}

	let ref_output_inner = match slot_value_type(&parsed.output_type) {
		Type::Reference(reference) => Some((*reference.elem).clone()),
		_ => None,
	};
	if let Some(inner) = &ref_output_inner {
		let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
		let open_generics = parsed.fn_generics.iter().filter_map(|param| match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref() => Some(&type_param.ident),
			_ => None,
		});
		if open_generics.into_iter().any(|generic| type_contains_ident(inner, generic)) {
			return quote!();
		}
	}

	let fn_name = &parsed.fn_name;
	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();
	let lend_flags: Vec<bool> = regular_fields
		.iter()
		.map(|field| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. })))
		.collect();

	let entries = rows.iter().map(|row| {
		let input_types = row.iter().zip(&lend_flags).map(|(ty, lend)| match lend {
			true => quote!(gcore::registry::lend_edge_type::<#ty>()),
			false => quote!(gcore::registry::edge_type::<#ty>()),
		});
		let edge_types = row.iter().zip(&lend_flags).map(|(ty, lend)| match lend {
			true => quote!(gcore::registry::SharedEdge<gcore::registry::ErasedLendNode<#ty>>),
			false => quote!(gcore::registry::SharedEdge<gcore::registry::ErasedNode<#ty>>),
		});
		let output = quote!(<#struct_name<#(#edge_types),*> as gcore::node::Node<gcore::context::ContextImpl<'static>>>::Output);
		let (io_output, construct) = match &ref_output_inner {
			Some(inner) => (
				quote!(gcore::registry::ref_type::<#inner>()),
				quote!(Ok(gcore::registry::EdgeHandle::new_ref(::std::sync::Arc::new(#struct_name::new(#(#names),*)) as ::std::sync::Arc<gcore::registry::ErasedLendNode<#inner>>))),
			),
			None => (
				quote!(gcore::concrete!(#output)),
				quote!(Ok(gcore::registry::EdgeHandle::new(::std::sync::Arc::new(#struct_name::new(#(#names),*)) as ::std::sync::Arc<gcore::registry::ErasedNode<#output>>))),
			),
		};
		let downcasts = names.iter().zip(row.iter()).zip(&lend_flags).map(|((name, ty), lend)| match lend {
			true => quote!(let #name = inputs.next().unwrap().downcast_lend::<#ty>()?;),
			false => quote!(let #name = inputs.next().unwrap().downcast::<#ty>()?;),
		});
		quote! {
			gcore::registry::RegistryEntry {
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
					#construct
				},
			}
		}
	});

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			vec![#(#entries),*]
		}
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
	if type_disqualifies(&output) {
		return quote!();
	}

	let fn_name = &parsed.fn_name;
	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();

	let entries = rows.iter().map(|row| {
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
		quote! {
			gcore::registry::RegistryEntry {
				io: gcore::registry::NodeIOTypes::new(
					gcore::concrete!(gcore::context::ContextImpl<'static>),
					gcore::registry::record_type::<#output>(),
					vec![#(#input_types),*],
				),
				constructor: |inputs| {
					if inputs.len() != #arity {
						return Err(gcore::registry::ConstructionError::Arity { expected: #arity, got: inputs.len() });
					}
					let mut inputs = inputs.into_iter();
					#(#downcasts)*
					let __node = #struct_name::new(#(#names,)* #(#layout_args)*);
					Ok(gcore::registry::EdgeHandle::new_record::<#output>(::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>))
				},
			}
		}
	});

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
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
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(gcore::registry::lend_edge_type::<#ty>()),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(gcore::registry::edge_type::<#ty>()),
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
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(let #name = inputs.next().unwrap().downcast_lend::<#ty>()?;),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(let #name = inputs.next().unwrap().downcast::<#ty>()?;),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => quote!(let #name = inputs.next().unwrap().downcast::<#output_type>()?;),
		}
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
					let __node = #struct_name::new(#(#names,)* &__union);
					Ok(gcore::registry::EdgeHandle::new_erased(
						::std::sync::Arc::new(__node) as ::std::sync::Arc<gcore::registry::ErasedRecordNode>,
						#first_source_ty,
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

	let input_types = regular_fields.iter().enumerate().map(|(index, field)| {
		let ParsedFieldType::Regular(RegularParsedField { ty, lend, .. }) = &field.ty else {
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
		match lend.is_some() {
			true => quote!(gcore::registry::lend_edge_type::<#ty>()),
			false => quote!(gcore::registry::edge_type::<#ty>()),
		}
	});
	let downcasts = regular_fields.iter().enumerate().map(|(index, field)| {
		let name = &field.pat_ident.ident;
		let ParsedFieldType::Regular(RegularParsedField { ty, lend, .. }) = &field.ty else {
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
		match lend.is_some() {
			true => quote!(let #name = inputs.next().unwrap().downcast_lend::<#ty>()?;),
			false => quote!(let #name = inputs.next().unwrap().downcast::<#ty>()?;),
		}
	});
	let wire_layout_arg = carrier_in_fields.then(|| quote!(&__carrier_layout,)).into_iter();
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
					let __node = #struct_name::new(#(#names,)* #(#wire_layout_arg)*);
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
