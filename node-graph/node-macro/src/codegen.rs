use std::sync::atomic::AtomicU64;

use crate::parsing::*;
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::FoundCrate;
use quote::{format_ident, quote};
use syn::{parse_quote, Ident};
static NODE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn generate_node_code(parsed: &ParsedNodeFn) -> TokenStream2 {
	let ParsedNodeFn {
		attributes,
		fn_name,
		struct_name,
		mod_name,
		fn_generics,
		input_type,
		input_name,
		output_type,
		is_async,
		fields,
		body,
		crate_name: graphene_core_crate,
		..
	} = parsed;

	let category = &attributes.category.as_ref().map(|value| quote!(Some(#value))).unwrap_or(quote!(None));

	let display_name = match &attributes.display_name.as_ref() {
		Some(lit) => lit.value(),
		_ => struct_name.to_string().to_case(Case::Title),
	};

	let struct_generics: Vec<Ident> = fields.iter().enumerate().map(|(i, _)| format_ident!("Node{}", i)).collect();

	let struct_fields = fields.iter().zip(struct_generics.iter()).map(|(field, gen)| {
		let name = match field {
			ParsedField::Regular { name, .. } | ParsedField::Node { name, .. } => name,
		};
		quote! { pub(super) #name: #gen }
	});

	let field_names: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { name, .. } | ParsedField::Node { name, .. } => name,
		})
		.collect();

	let field_types: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { ty, .. } | ParsedField::Node { ty, .. } => ty,
		})
		.collect();

	let default_values: Vec<_> = fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular {
				default_value: Some(default_value), ..
			} => quote!(Some(stringify!(#default_value))),
			_ => quote!(None),
		})
		.collect();

	let eval_args = fields.iter().map(|field| match field {
		ParsedField::Regular { name, .. } => {
			quote! { let #name = self.#name.eval(()); }
		}
		ParsedField::Node { name, .. } => {
			quote! { let #name = &self.#name; }
		}
	});

	let mut clauses = Vec::new();
	for (field, name) in fields.iter().zip(struct_generics.iter()) {
		clauses.push(match (field, *is_async) {
			(ParsedField::Regular { ty, .. }, _) => quote!(#name: Node<'n, (), Output = #ty> ),
			(ParsedField::Node { input_type, output_type, .. }, false) => quote!(#name: Node<'n, #input_type, Output = #output_type> + WasmNotSync + 'n),
			(ParsedField::Node { input_type, output_type, .. }, true) => {
				quote!(#name: Node<'n, #input_type, Output: core::future::Future<Output = #output_type>> + WasmNotSync + 'n)
			}
		});
	}

	let where_clause = quote! {
		where
			#(#clauses,)*
			#output_type: 'n,
	};

	let new_args = struct_generics.iter().zip(field_names.iter()).map(|(gen, name)| {
		quote! { #name: #gen }
	});

	let async_keyword = is_async.then(|| quote!(async));

	let eval_impl = if *is_async {
		quote! {
			type Output = DynFuture<'n, #output_type>;
			fn eval(&'n self, __input: #input_type) -> Self::Output {
				#(#eval_args)*
				Box::pin(#fn_name(__input #(, #field_names)*))
			}
		}
	} else {
		quote! {
			type Output = #output_type;
			fn eval(&'n self, __input: #input_type) -> Self::Output {
				#(#eval_args)*
				#fn_name(__input #(, #field_names)*)
			}
		}
	};
	let identifier = quote!(concat![std::module_path!(), "::", stringify!(#struct_name)]);

	let register_node_impl = generate_register_node_impl(parsed, &field_names, struct_name, &identifier);

	let graphene_core = match graphene_core_crate {
		FoundCrate::Itself => quote!(crate),
		FoundCrate::Name(name) => {
			let ident = Ident::new(name, proc_macro2::Span::call_site());
			quote!( #ident )
		}
	};

	quote! {
		#async_keyword fn #fn_name <'n, #(#fn_generics,)*> (#input_name: #input_type #(, #field_names: #field_types)*) -> #output_type #body

		#[automatically_derived]
		impl<'n,  #(#fn_generics,)* #(#struct_generics,)*> #graphene_core::Node<'n, #input_type> for #mod_name::#struct_name<#(#struct_generics,)*>
		#where_clause
		{
			#eval_impl
		}

		mod #mod_name {
			use super::*;
			use #graphene_core as gcore;
			use gcore::{Node, NodeIOTypes, concrete, fn_type, ProtoNodeIdentifier, WasmNotSync, NodeIO};
			use gcore::value::ClonedNode;
			use gcore::ops::TypeNode;
			use gcore::registry::{NodeMetadata, FieldMetadata, NODE_REGISTRY, NODE_METADATA, DynAnyNode, DowncastBothNode, DynFuture, TypeErasedBox, PanicNode};
			use ctor::ctor;

			pub struct #struct_name<#(#struct_generics,)*> {
				#(#struct_fields,)*
			}

			#[automatically_derived]
			impl<'n, #(#struct_generics,)*> #struct_name<#(#struct_generics,)*>
			{
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
					fields: vec![
						#(
							FieldMetadata {
								name: stringify!(#field_names).to_string(),
								default_value: #default_values,
							},
						)*
					],
				};
				NODE_METADATA.lock().unwrap().insert(#identifier, metadata);
			}
		}
	}
}

fn generate_register_node_impl(parsed: &ParsedNodeFn, field_names: &[&Ident], struct_name: &Ident, identifer: &TokenStream2) -> TokenStream2 {
	let input_type = &parsed.input_type;
	let output_type = &parsed.output_type;
	let mut constructors = Vec::new();
	let unit = parse_quote!(());
	let parameter_types: Vec<_> = parsed
		.fields
		.iter()
		.map(|field| match field {
			ParsedField::Regular { implementations, ty, .. } => {
				if !implementations.is_empty() {
					implementations.into_iter().map(|ty| (&unit, ty)).collect()
				} else {
					vec![(&unit, ty)]
				}
			}
			ParsedField::Node {
				implementations,
				output_type,
				input_type,
				..
			} => {
				if !implementations.is_empty() {
					implementations.into_iter().map(|tup| (&tup.elems[0], &tup.elems[1])).collect()
				} else {
					vec![(input_type, output_type)]
				}
			}
		})
		.collect();

	let max_implementations = parameter_types.iter().map(|x| x.len()).max();
	let future_node = (!parsed.is_async).then(|| quote!(let node = gcore::registry::FutureWrapperNode::new(node);));

	for i in 0..max_implementations.unwrap_or(0) {
		let mut temp_constructors = Vec::new();
		let mut temp_node_io = Vec::new();

		for (j, types) in parameter_types.iter().enumerate() {
			let field_name = field_names[j];
			let (input_type, output_type) = types[i.min(types.len() - 1)];

			let node = matches!(parsed.fields[j], ParsedField::Node { .. });

			let downcast_node = quote!(
			let #field_name: DowncastBothNode<#input_type, #output_type> = DowncastBothNode::new(args[#j].clone());
			 );
			temp_constructors.push(if node {
				assert!(parsed.is_async, "Node needs to be async if you want to use lambda parameters");
				downcast_node
			} else {
				quote!(
						#downcast_node
						let value = #field_name.eval(()).await;
						let #field_name = ClonedNode::new(value);
						let #field_name: TypeNode<_, #input_type, #output_type> = TypeNode::new(#field_name);
						// try polling futures
				)
			});
			temp_node_io.push(quote!(#input_type, #output_type));
		}
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
						Box::new(any)  as TypeErasedBox<'_>
					})
				}, {
					let node = #struct_name::new(#(PanicNode::<#temp_node_io>::new(),)*);
					let params = vec![#(fn_type!(#temp_node_io),)*];
					let mut node_io = NodeIO::<'_, #input_type>::#node_io(&node, params);
					node_io

				}
			)
		));
	}
	let registry_name = format_ident!("__node_registry_{}_{}", NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst), struct_name);

	quote! {

		#[cfg_attr(not(target_arch = "wasm32"), ctor)]
		fn register_node() {
			let mut registry = NODE_REGISTRY.lock().unwrap();
			registry.insert(
				#identifer,
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
	}
}
