#![doc(html_root_url = "http://docs.rs/dyn-any-derive/0.1.0")]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, GenericParam, Lifetime, TypeParamBound, parse_macro_input};

/// Derives an implementation for the [`DynAny`] trait.
///
/// # Note
///
/// Currently only works with `struct` inputs.
///
/// # Example
///
/// ## Struct
///
/// ```
/// # use dyn_any::{DynAny, StaticType};
/// #[derive(DynAny)]
/// pub struct Color<'a, 'b> {
///     r: &'a u8,
///     g: &'b u8,
///     b: &'a u8,
/// }
///
///
/// // Generated Impl
///
/// // impl<'dyn_any> StaticType for Color<'dyn_any, 'dyn_any> {
/// //     type Static = Color<'static, 'static>;
/// // }
///
/// ```
#[proc_macro_derive(DynAny, attributes(dyn_any_derive))]
pub fn system_desc_derive(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let struct_name = &ast.ident;
	let generics = &ast.generics;

	let static_params = generic_arguments(generics, "'static");
	let dyn_params = generic_arguments(generics, "'dyn_any");

	let impl_params = generics.params.iter().map(|param| match param {
		GenericParam::Type(t) => {
			let mut t = t.clone();
			t.bounds.push(TypeParamBound::Lifetime(Lifetime::new("'static", Span::call_site())));
			quote! {#t}
		}
		param => quote! {#param},
	});
	quote! {
		unsafe impl<'dyn_any, #(#impl_params,)*> dyn_any::StaticType for #struct_name <#(#dyn_params,)*> {
			type Static =  #struct_name <#(#static_params,)*>;
		}
	}
	.into()
}

/// The struct's generic parameters as argument tokens: bare idents for type
/// and const parameters (bounds are illegal in argument position), the
/// replacement for lifetimes.
fn generic_arguments(generics: &syn::Generics, replacement: &str) -> Vec<proc_macro2::TokenStream> {
	generics
		.params
		.iter()
		.map(|param| match param {
			GenericParam::Lifetime(_) => {
				let lifetime = Lifetime::new(replacement, Span::call_site());
				quote! {#lifetime}
			}
			GenericParam::Type(t) => {
				let ident = &t.ident;
				quote! {#ident}
			}
			GenericParam::Const(c) => {
				let ident = &c.ident;
				quote! {#ident}
			}
		})
		.collect::<Vec<_>>()
}
