use std::sync::atomic::AtomicU64;

use crate::parsing::*;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::FoundCrate;
use quote::{format_ident, quote};
use syn::{parse_quote, punctuated::Punctuated, spanned::Spanned, token::Comma, Error, Ident, Token, WhereClause, WherePredicate};
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
	let input_type = &input.ty;

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

	let field_types: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { ty, .. } => ty.clone(),
			ParsedField::Node { output_type, input_type, .. } => match parsed.is_async {
				true => parse_quote!(&'n impl #graphene_core::Node<'n, #input_type, Output: core::future::Future<Output=#output_type> + #graphene_core::WasmNotSend>),

				false => parse_quote!(&'n impl #graphene_core::Node<'n, #input_type, Output = #output_type>),
			},
		})
		.collect();

	let value_sources: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { value_source, .. } => match value_source {
				ValueSource::Default(data) => quote!(ValueSource::Default(stringify!(#data))),
				ValueSource::Scope(data) => quote!(ValueSource::Scope(#data)),
				_ => quote!(ValueSource::None),
			},
			_ => quote!(ValueSource::None),
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
			quote! { let #name = self.#name.eval(()); }
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

	let mut clauses = Vec::new();
	for (field, name) in fields.iter().zip(struct_generics.iter()) {
		clauses.push(match (field, *is_async) {
			(ParsedField::Regular { ty, .. }, _) => quote!(#name: #graphene_core::Node<'n, (), Output = #ty> ),
			(ParsedField::Node { input_type, output_type, .. }, false) => {
				quote!(for<'all_input> #name: #graphene_core::Node<'all_input, #input_type, Output = #output_type> + #graphene_core::WasmNotSync)
			}
			(ParsedField::Node { input_type, output_type, .. }, true) => {
				quote!(for<'all_input> #name: #graphene_core::Node<'all_input, #input_type, Output: core::future::Future<Output = #output_type> + #graphene_core::WasmNotSend> + #graphene_core::WasmNotSync)
			}
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

	let eval_impl = if *is_async {
		quote! {
			type Output = #graphene_core::registry::DynFuture<'n, #output_type>;
			#[inline]
			fn eval(&'n self, __input: #input_type) -> Self::Output {
				#(#eval_args)*
				Box::pin(self::#fn_name(__input #(, #field_names)*))
			}
		}
	} else {
		quote! {
			type Output = #output_type;
			#[inline]
			fn eval(&'n self, __input: #input_type) -> Self::Output {
				#(#eval_args)*
				self::#fn_name(__input #(, #field_names)*)
			}
		}
	};
	let path = match parsed.attributes.path {
		Some(ref path) => quote!(stringify!(#path).replace(' ', "")),
		None => quote!(std::module_path!().rsplit_once("::").unwrap().0),
	};
	let identifier = quote!(format!("{}::{}", #path, stringify!(#struct_name)));

	let register_node_impl = generate_register_node_impl(parsed, &field_names, &struct_name, &identifier)?;
	let import_name = format_ident!("_IMPORT_STUB_{}", mod_name.to_string().to_case(Case::UpperSnake));

	Ok(quote! {
		/// Underlying implementation for [#struct_name]
		#[inline]
		#[allow(clippy::too_many_arguments)]
		#async_keyword fn #fn_name <'n, #(#fn_generics,)*> (#input_ident: #input_type #(, #field_idents: #field_types)*) -> #output_type #where_clause #body

		#[automatically_derived]
		impl<'n, #(#fn_generics,)* #(#struct_generics,)*> #graphene_core::Node<'n, #input_type> for #mod_name::#struct_name<#(#struct_generics,)*>
		#struct_where_clause
		{
			#eval_impl
		}
		#[doc(inline)]
		pub use #mod_name::#struct_name;

		#[doc(hidden)]
		mod #mod_name {
			use super::*;
			use #graphene_core as gcore;
			use gcore::{Node, NodeIOTypes, concrete, fn_type, future, ProtoNodeIdentifier, WasmNotSync, NodeIO};
			use gcore::value::ClonedNode;
			use gcore::ops::TypeNode;
			use gcore::registry::{NodeMetadata, FieldMetadata, NODE_REGISTRY, NODE_METADATA, DynAnyNode, DowncastBothNode, DynFuture, TypeErasedBox, PanicNode, ValueSource};
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
					fields: vec![
						#(
							FieldMetadata {
								name: #input_names,
								exposed: #exposed,
								value_source: #value_sources,
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

fn generate_register_node_impl(parsed: &ParsedNodeFn, field_names: &[&Ident], struct_name: &Ident, identifier: &TokenStream2) -> Result<TokenStream2, syn::Error> {
	if parsed.attributes.skip_impl {
		return Ok(quote!());
	}

	let mut constructors = Vec::new();
	let unit = parse_quote!(());
	let parameter_types: Vec<_> = parsed
		.fields
		.iter()
		.map(|field| {
			match field {
				ParsedField::Regular { implementations, ty, .. } => {
					if !implementations.is_empty() {
						implementations.iter().map(|ty| (&unit, ty, false)).collect()
					} else {
						vec![(&unit, ty, false)]
					}
				}
				ParsedField::Node {
					implementations,
					input_type,
					output_type,
					..
				} => {
					if !implementations.is_empty() {
						implementations.iter().map(|impl_| (&impl_.input, &impl_.output, true)).collect()
					} else {
						vec![(input_type, output_type, true)]
					}
				}
			}
			.into_iter()
			.map(|(input, out, node)| (substitute_lifetimes(input.clone()), substitute_lifetimes(out.clone()), node))
			.collect::<Vec<_>>()
		})
		.collect();

	let max_implementations = parameter_types.iter().map(|x| x.len()).chain([parsed.input.implementations.len().max(1)]).max();
	let future_node = (!parsed.is_async).then(|| quote!(let node = gcore::registry::FutureWrapperNode::new(node);));

	for i in 0..max_implementations.unwrap_or(0) {
		let mut temp_constructors = Vec::new();
		let mut temp_node_io = Vec::new();
		let mut panic_node_types = Vec::new();

		for (j, types) in parameter_types.iter().enumerate() {
			let field_name = field_names[j];
			let (input_type, output_type, impl_node) = &types[i.min(types.len() - 1)];

			let node = matches!(parsed.fields[j], ParsedField::Node { .. });

			let downcast_node = quote!(
			let #field_name: DowncastBothNode<#input_type, #output_type> = DowncastBothNode::new(args[#j].clone());
			 );
			temp_constructors.push(if node {
				if !parsed.is_async {
					return Err(Error::new_spanned(&parsed.fn_name, "Node needs to be async if you want to use lambda parameters"));
				}
				downcast_node
			} else {
				quote!(
						#downcast_node
						let #field_name = #field_name.eval(()).await;
						let #field_name = ClonedNode::new(#field_name);
						let #field_name: TypeNode<_, #input_type, #output_type> = TypeNode::new(#field_name);
						// try polling futures
				)
			});
			temp_node_io.push(quote!(fn_type!(#input_type, #output_type, alias: #output_type)));
			match parsed.is_async && *impl_node {
				true => panic_node_types.push(quote!(#input_type, DynFuture<'static, #output_type>)),
				false => panic_node_types.push(quote!(#input_type, #output_type)),
			};
		}
		let input_type = match parsed.input.implementations.is_empty() {
			true => parsed.input.ty.clone(),
			false => parsed.input.implementations[i.min(parsed.input.implementations.len() - 1)].clone(),
		};
		let node_io = if parsed.is_async { quote!(to_async_node_io) } else { quote!(to_node_io) };
		constructors.push(quote!(
			(
				|args| {
					Box::pin(async move {
						#(#temp_constructors;)*
						let node = #struct_name::new(#(#field_names,)*);
						// try polling futures
						#future_node
						let any: DynAnyNode<#input_type, _, _> = DynAnyNode::new(node);
						Box::new(any) as TypeErasedBox<'_>
					})
				}, {
					let node = #struct_name::new(#(PanicNode::<#panic_node_types>::new(),)*);
					let params = vec![#(#temp_node_io,)*];
					let mut node_io = NodeIO::<'_, #input_type>::#node_io(&node, params);
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

struct LifetimeReplacer;

impl VisitMut for LifetimeReplacer {
	fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
		lifetime.ident = syn::Ident::new("_", lifetime.ident.span());
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
fn substitute_lifetimes(mut ty: Type) -> Type {
	LifetimeReplacer.visit_type_mut(&mut ty);
	ty
}
