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

	let async_source = parsed.injects_async_source_fields();
	let slot_value_type = slot_value_type(output_type);
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

	let ctx_generic = match ctx_bounds.is_empty() {
		true => quote!(#ctx_ident),
		false => quote!(#ctx_ident: #(#ctx_bounds)+*),
	};
	let mut generics: Vec<TokenStream2> = parsed
		.fn_generics
		.iter()
		.map(|param| match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) == ctx_param.map(|ctx_param| &ctx_param.ident) => ctx_generic.clone(),
			param => quote!(#param),
		})
		.collect();
	if ctx_param.is_none() {
		generics.push(ctx_generic);
	}
	if let Some(lifetime) = &introduced_lend_lifetime {
		generics.insert(0, quote!(#lifetime));
	}

	let fn_name = &parsed.fn_name;
	let mod_name = format_ident!("_{}_mod", parsed.mod_name);
	let struct_name = format_ident!("{}Node", parsed.struct_name);
	let output_type = &parsed.output_type;
	let trait_output = slot_value_type(&parsed.output_type);
	let raw_lazy = matches!(kernel_kind(&parsed.output_type), KernelKind::Poll(_));
	let injected_name = |ident: &Ident| async_source && (ident == "_runtime" || ident == "_source");
	let where_predicates: Vec<TokenStream2> = parsed.where_clause.iter().flat_map(|clause| clause.predicates.iter()).map(|predicate| quote!(#predicate)).collect();

	let (data_fields, regular_fields): (Vec<_>, Vec<_>) = parsed.fields.iter().partition(|field| field.is_data_field);

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

	let kernel_params = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).map(|field| {
		let pat = &field.pat_ident;
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(#pat: &#ty),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#pat: #ty),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) if raw_lazy => {
				let bound = lazy_bound(output_type);
				quote!(#pat: &impl #bound)
			}
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
				let bound = lazy_bound(output_type);
				quote!(#pat: #core_types::node::LazyInput<'_, impl #bound>)
			}
		}
	});

	let node_bounds = regular_fields.iter().zip(&node_generics).map(|(field, node_generic)| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => {
			let lifetime = lend_lifetime.as_ref().expect("lend fields imply the lend lifetime");
			quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = &#lifetime #ty>)
		}
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#node_generic: #core_types::node::Node<#ctx_ident, Output = #ty>),
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
			ParsedFieldType::Regular(_) => quote! {
				let #name = match __cell.eval_input(#index, &self.#name, __input) {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				};
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
	let kernel = match async_fn {
		false => quote! {
			#[allow(clippy::too_many_arguments)]
			#vis fn #fn_name<#(#generics,)*>(#ctx_pat: &#ctx_ident #(, #data_params)* #(, #kernel_params)*) -> #output_type #fn_where #body
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
	let eval_tail = match (async_fn, future_kernel) {
		(false, false) => lift,
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

	let entries = entries_tokens(parsed, &struct_name, &data_field_generic_idents, &regular_fields);
	let cfg = crate::shader_nodes::modify_cfg(&parsed.attributes);

	let top_level = quote! {
		#cfg
		#[automatically_derived]
		impl<#(#generics,)* #(#node_generics,)*> #core_types::node::Node<#ctx_ident> for #mod_name::#struct_name<#(#struct_type_params,)*>
		where
			#(#node_bounds,)*
			#(#lend_outlives,)*
			#(#clampable_bounds,)*
			#(#async_bounds,)*
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

			#batch_impl
		}
	};

	Ok(NodeImplTokens {
		in_mod: entries,
		top_level: quote! {
			#kernel

			#top_level
		},
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
