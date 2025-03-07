use std::sync::atomic::AtomicU64;

use crate::parsing::*;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::FoundCrate;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{parse_quote, Error, Ident, PatIdent, Token, WhereClause, WherePredicate};
static NODE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn generate_node_code(parsed: &ParsedNodeFn) -> syn::Result<TokenStream2> {
	let ParsedNodeFn {
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
		crate_name: graphene_core_crate,
		description,
		..
	} = parsed;

	let category = &attributes.category.as_ref().map(|value| quote!(Some(#value))).unwrap_or(quote!(None));
	let mod_name = format_ident!("_{}_mod", mod_name);

	let display_name = match &attributes.display_name.as_ref() {
		Some(lit) => lit.value(),
		None => struct_name.to_string().to_case(Case::Title),
	};
	let struct_name = format_ident!("{}Node", struct_name);

	let struct_generics: Vec<Ident> = fields.iter().enumerate().map(|(i, _)| format_ident!("Node{}", i)).collect();
	let input_ident = &input.pat_ident;

	let field_idents: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { pat_ident, .. } | ParsedField::Node { pat_ident, .. } => pat_ident,
		})
		.collect();
	let field_names: Vec<_> = field_idents.iter().map(|pat_ident| &pat_ident.ident).collect();

	let input_names: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { name, .. } | ParsedField::Node { name, .. } => name,
		})
		.zip(field_names.iter())
		.map(|zipped| match zipped {
			(Some(name), _) => name.value(),
			(_, name) => name.to_string().to_case(convert_case::Case::Title),
		})
		.collect();

	let input_descriptions: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { description, .. } | ParsedField::Node { description, .. } => description,
		})
		.collect();

	let struct_fields = field_names.iter().zip(struct_generics.iter()).map(|(name, gen)| {
		quote! { pub(super) #name: #gen }
	});

	let graphene_core = match graphene_core_crate {
		FoundCrate::Itself => quote!(crate),
		FoundCrate::Name(name) => {
			let ident = Ident::new(name, proc_macro2::Span::call_site());
			quote!( #ident )
		}
	};

	let mut future_idents = Vec::new();

	let field_types: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { ty, .. } => ty.clone(),
			ParsedField::Node { output_type, input_type, .. } => match parsed.is_async {
				true => parse_quote!(&'n impl #graphene_core::Node<'n, #input_type, Output = impl core::future::Future<Output=#output_type>>),
				false => parse_quote!(&'n impl #graphene_core::Node<'n, #input_type, Output = #output_type>),
			},
		})
		.collect();

	let widget_override: Vec<_> = fields
		.iter()
		.map(|field| {
			let parsed_widget_override = match field {
				ParsedField::Regular { widget_override, .. } => widget_override,
				ParsedField::Node { widget_override, .. } => widget_override,
			};
			match parsed_widget_override {
				ParsedWidgetOverride::None => quote!(RegistryWidgetOverride::None),
				ParsedWidgetOverride::Hidden => quote!(RegistryWidgetOverride::Hidden),
				ParsedWidgetOverride::String(lit_str) => quote!(RegistryWidgetOverride::String(#lit_str)),
				ParsedWidgetOverride::Custom(lit_str) => quote!(RegistryWidgetOverride::Custom(#lit_str)),
			}
		})
		.collect();

	let value_sources: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { value_source, .. } => match value_source {
				ParsedValueSource::Default(data) => quote!(RegistryValueSource::Default(stringify!(#data))),
				ParsedValueSource::Scope(data) => quote!(RegistryValueSource::Scope(#data)),
				_ => quote!(RegistryValueSource::None),
			},
			_ => quote!(RegistryValueSource::None),
		})
		.collect();

	let default_types: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { implementations, .. } => match implementations.first() {
				Some(ty) => quote!(Some(concrete!(#ty))),
				_ => quote!(None),
			},
			_ => quote!(None),
		})
		.collect();

	let number_min_values: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { number_min: Some(number_min), .. } => quote!(Some(#number_min)),
			_ => quote!(None),
		})
		.collect();
	let number_max_values: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { number_max: Some(number_max), .. } => quote!(Some(#number_max)),
			_ => quote!(None),
		})
		.collect();
	let number_mode_range_values: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular {
				number_mode_range: Some(number_mode_range),
				..
			} => quote!(Some(#number_mode_range)),
			_ => quote!(None),
		})
		.collect();

	let exposed: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { exposed, .. } => quote!(#exposed),
			_ => quote!(true),
		})
		.collect();

	let eval_args = fields.iter().map(|field| match field {
		ParsedField::Regular { pat_ident, .. } => {
			let name = &pat_ident.ident;
			quote! { let #name = self.#name.eval(__input.clone()).await; }
		}
		ParsedField::Node { pat_ident, .. } => {
			let name = &pat_ident.ident;
			quote! { let #name = &self.#name; }
		}
	});

	let all_implementation_types = fields.iter().flat_map(|field| match field {
		ParsedField::Regular { implementations, .. } => implementations.into_iter().cloned().collect::<Vec<_>>(),
		ParsedField::Node { implementations, .. } => implementations
			.into_iter()
			.flat_map(|implementation| [implementation.input.clone(), implementation.output.clone()])
			.collect(),
	});
	let all_implementation_types = all_implementation_types.chain(input.implementations.iter().cloned());

	let input_type = &parsed.input.ty;
	let mut clauses = Vec::new();
	for (field, name) in fields.iter().zip(struct_generics.iter()) {
		clauses.push(match (field, *is_async) {
			(ParsedField::Regular { ty, .. }, _) => {
				let all_lifetime_ty = substitute_lifetimes(ty.clone(), "all");
				let id = future_idents.len();
				let fut_ident = format_ident!("F{}", id);
				future_idents.push(fut_ident.clone());
				quote!(
					#fut_ident: core::future::Future<Output = #ty> + #graphene_core::WasmNotSend + 'n,
					for<'all> #all_lifetime_ty: #graphene_core::WasmNotSend,
					#name: #graphene_core::Node<'n, #input_type, Output = #fut_ident> + #graphene_core::WasmNotSync
				)
			}
			(ParsedField::Node { input_type, output_type, .. }, true) => {
				let id = future_idents.len();
				let fut_ident = format_ident!("F{}", id);
				future_idents.push(fut_ident.clone());

				quote!(
					#fut_ident: core::future::Future<Output = #output_type> + #graphene_core::WasmNotSend + 'n,
					#name: #graphene_core::Node<'n, #input_type, Output = #fut_ident > + #graphene_core::WasmNotSync
				)
			}
			(ParsedField::Node { .. }, false) => unreachable!(),
		});
	}
	let where_clause = where_clause.clone().unwrap_or(WhereClause {
		where_token: Token![where](output_type.span()),
		predicates: Default::default(),
	});

	let mut struct_where_clause = where_clause.clone();
	let extra_where: Punctuated<WherePredicate, Comma> = parse_quote!(
		#(#clauses,)*
		#output_type: 'n,
	);
	struct_where_clause.predicates.extend(extra_where);

	let new_args = struct_generics.iter().zip(field_names.iter()).map(|(gen, name)| {
		quote! { #name: #gen }
	});

	let async_keyword = is_async.then(|| quote!(async));
	let await_keyword = is_async.then(|| quote!(.await));

	let eval_impl = quote! {
		type Output = #graphene_core::registry::DynFuture<'n, #output_type>;
		#[inline]
		fn eval(&'n self, __input: #input_type) -> Self::Output {
			Box::pin(async move {
				#(#eval_args)*
				self::#fn_name(__input #(, #field_names)*) #await_keyword
			})
		}
	};
	let path = match parsed.attributes.path {
		Some(ref path) => quote!(stringify!(#path).replace(' ', "")),
		None => quote!(std::module_path!().rsplit_once("::").unwrap().0),
	};
	let identifier = quote!(format!("{}::{}", #path, stringify!(#struct_name)));

	let register_node_impl = generate_register_node_impl(parsed, &field_names, &struct_name, &identifier)?;
	let import_name = format_ident!("_IMPORT_STUB_{}", mod_name.to_string().to_case(Case::UpperSnake));

	let properties = &attributes.properties_string.as_ref().map(|value| quote!(Some(#value))).unwrap_or(quote!(None));

	let node_input_accessor = generate_node_input_references(parsed, fn_generics, &field_idents, &graphene_core, &identifier);
	Ok(quote! {
		/// Underlying implementation for [#struct_name]
		#[inline]
		#[allow(clippy::too_many_arguments)]
		pub(crate) #async_keyword fn #fn_name <'n, #(#fn_generics,)*> (#input_ident: #input_type #(, #field_idents: #field_types)*) -> #output_type #where_clause #body

		#[automatically_derived]
		impl<'n, #(#fn_generics,)* #(#struct_generics,)* #(#future_idents,)*> #graphene_core::Node<'n, #input_type> for #mod_name::#struct_name<#(#struct_generics,)*>
		#struct_where_clause
		{
			#eval_impl
		}
		#[doc(inline)]
		pub use #mod_name::#struct_name;

		#[doc(hidden)]
		#node_input_accessor

		#[doc(hidden)]
		mod #mod_name {
			use super::*;
			use #graphene_core as gcore;
			use gcore::{Node, NodeIOTypes, concrete, fn_type, fn_type_fut, future, ProtoNodeIdentifier, WasmNotSync, NodeIO};
			use gcore::value::ClonedNode;
			use gcore::ops::TypeNode;
			use gcore::registry::{NodeMetadata, FieldMetadata, NODE_REGISTRY, NODE_METADATA, DynAnyNode, DowncastBothNode, DynFuture, TypeErasedBox, PanicNode, RegistryValueSource, RegistryWidgetOverride};
			use gcore::ctor::ctor;

			// Use the types specified in the implementation

			static #import_name: core::marker::PhantomData<(#(#all_implementation_types,)*)> = core::marker::PhantomData;

			#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
			pub struct #struct_name<#(#struct_generics,)*> {
				#(#struct_fields,)*
			}

			#[automatically_derived]
			impl<'n, #(#struct_generics,)*> #struct_name<#(#struct_generics,)*>
			{
				#[allow(clippy::too_many_arguments)]
				pub fn new(#(#new_args,)*) -> Self {
					Self {
						#(#field_names,)*
					}
				}
			}

			#register_node_impl

			#[cfg_attr(not(target_arch = "wasm32"), ctor)]
			fn register_metadata() {
				let metadata = NodeMetadata {
					display_name: #display_name,
					category: #category,
					description: #description,
					properties: #properties,
					fields: vec![
						#(
							FieldMetadata {
								name: #input_names,
								widget_override: #widget_override,
								description: #input_descriptions,
								exposed: #exposed,
								value_source: #value_sources,
								default_type: #default_types,
								number_min: #number_min_values,
								number_max: #number_max_values,
								number_mode_range: #number_mode_range_values,
							},
						)*
					],
				};
				NODE_METADATA.lock().unwrap().insert(#identifier, metadata);
			}
		}
	})
}

/// Generates strongly typed utilites to access inputs
fn generate_node_input_references(parsed: &ParsedNodeFn, fn_generics: &[crate::GenericParam], field_idents: &[&PatIdent], graphene_core: &TokenStream2, identifier: &TokenStream2) -> TokenStream2 {
	if parsed.attributes.skip_impl {
		return quote! {};
	}
	let inputs_module_name = format_ident!("{}", parsed.struct_name.to_string().to_case(Case::Snake));

	let (mut modified, mut generic_collector) = FilterUsedGenerics::new(fn_generics);

	let mut generated_input_accessor = Vec::new();
	for (input_index, (parsed_input, input_ident)) in parsed.fields.iter().zip(field_idents).enumerate() {
		let mut ty = match parsed_input {
			ParsedField::Regular { ty, .. } => ty,
			ParsedField::Node { output_type, .. } => output_type,
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
			impl <#(#used),*> #graphene_core::NodeInputDecleration for #struct_name <#(#fn_generic_params),*> {
				const INDEX: usize = #input_index;
				fn identifier() -> &'static str {
					protonode_identifier()
				}
				type Result = #ty;
			}
		})
	}

	quote! {
		pub mod #inputs_module_name {
			use super::*;

			pub fn protonode_identifier() -> &'static str {
				// Storing the string in a once lock should reduce allocations (since we call this in a loop)?
				static NODE_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
				NODE_NAME.get_or_init(|| #identifier )
			}
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

fn generate_register_node_impl(parsed: &ParsedNodeFn, field_names: &[&Ident], struct_name: &Ident, identifier: &TokenStream2) -> Result<TokenStream2, syn::Error> {
	if parsed.attributes.skip_impl {
		return Ok(quote!());
	}

	let mut constructors = Vec::new();
	let unit = parse_quote!(gcore::Context);
	let parameter_types: Vec<_> = parsed
		.fields
		.iter()
		.map(|field| {
			match field {
				ParsedField::Regular { implementations, ty, .. } => {
					if !implementations.is_empty() {
						implementations.iter().map(|ty| (&unit, ty)).collect()
					} else {
						vec![(&unit, ty)]
					}
				}
				ParsedField::Node {
					implementations,
					input_type,
					output_type,
					..
				} => {
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

	for i in 0..max_implementations.unwrap_or(0) {
		let mut temp_constructors = Vec::new();
		let mut temp_node_io = Vec::new();
		let mut panic_node_types = Vec::new();

		for (j, types) in parameter_types.iter().enumerate() {
			let field_name = field_names[j];
			let (input_type, output_type) = &types[i.min(types.len() - 1)];

			let node = matches!(parsed.fields[j], ParsedField::Node { .. });

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
		constructors.push(quote!(
			(
				|args| {
					Box::pin(async move {
						#(#temp_constructors;)*
						let node = #struct_name::new(#(#field_names,)*);
						// try polling futures
						let any: DynAnyNode<#input_type, _, _> = DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox<'_>
					})
				}, {
					let node = #struct_name::new(#(PanicNode::<#panic_node_types>::new(),)*);
					let params = vec![#(#temp_node_io,)*];
					let mut node_io = NodeIO::<'_, #input_type>::to_async_node_io(&node, params);
					node_io

				}
			)
		));
	}
	let registry_name = format_ident!("__node_registry_{}_{}", NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst), struct_name);

	Ok(quote! {

		#[cfg_attr(not(target_arch = "wasm32"), ctor)]
		fn register_node() {
			let mut registry = NODE_REGISTRY.lock().unwrap();
			registry.insert(
				#identifier,
				vec![
					#(#constructors,)*
				]
			);
		}
		#[cfg(target_arch = "wasm32")]
		#[no_mangle]
		extern "C" fn #registry_name() {
			register_node();
			register_metadata();
		}
	})
}

use syn::{visit_mut::VisitMut, GenericArgument, Lifetime, Type};

struct LifetimeReplacer(&'static str);

impl VisitMut for LifetimeReplacer {
	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		lifetime.ident = syn::Ident::new(self.0, lifetime.ident.span());
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
	fn visit_lifetime_mut(&mut self, used_lifetime: &mut syn::Lifetime) {
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
