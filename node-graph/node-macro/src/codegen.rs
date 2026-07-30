use crate::parsing::*;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use std::sync::atomic::AtomicU64;
use syn::punctuated::Punctuated;
use syn::{Ident, PatIdent};
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
		is_async,
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

	let regular_field_defs = regular_field_names.iter().zip(node_generics.iter()).map(|(name, r#gen)| {
		quote! { pub(super) #name: #r#gen }
	});

	let async_source = *is_async || crate::gcodegen::is_source_kernel(output_type);
	let slot_value_type = crate::gcodegen::slot_value_type(output_type);
	let slot_field = async_source
		.then(|| quote! { pub(super) slot: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<u64, Option<gcore::gpoll::GPoll<#slot_value_type>>>>> })
		.into_iter();
	let struct_fields = data_field_defs.chain(regular_field_defs).chain(slot_field);

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
	let new_args = node_generics.iter().zip(regular_field_names.iter()).map(|(r#gen, name)| {
		quote! { #name: #r#gen }
	});

	// Initialize data fields with Default, regular fields with parameters
	let data_inits = data_field_names.iter().map(|name| {
		quote! { #name: Default::default() }
	});
	let regular_inits = regular_field_names.iter().map(|name| {
		quote! { #name }
	});
	let slot_init = async_source.then(|| quote! { slot: Default::default() }).into_iter();
	let all_field_inits = data_inits.chain(regular_inits).chain(slot_init);

	// Data fields may not implement Copy, PartialEq, etc., so only derive Debug and Clone
	let struct_derives = if data_fields.is_empty() && !async_source {
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
	let import_name = format_ident!("_IMPORT_STUB_{}", mod_name.to_string().to_case(Case::UpperSnake));
	let gnode = crate::gcodegen::generate_gnode_code(crate_ident, parsed)?;
	let gnode_in_mod = gnode.in_mod;
	let gnode_top_level = gnode.top_level;
	let entries_name = format_ident!("{}_entries", parsed.fn_name);
	let register_entries = match gnode_in_mod.is_empty() {
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
		#gnode_top_level

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

			#gnode_in_mod

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

use crate::crate_ident::CrateIdent;
use crate::shader_nodes::{ShaderCodegen, ShaderTokens};
use syn::visit_mut::VisitMut;
use syn::{Lifetime, Type};

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
