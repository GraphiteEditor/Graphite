#![doc(html_root_url = "http://docs.rs/dyn-any-derive/0.1.0")]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, GenericParam, Lifetime, LifetimeParam, TypeParamBound};

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

	let static_params = replace_lifetimes(generics, "'static");
	let dyn_params = replace_lifetimes(generics, "'dyn_any");

	let old_params = &generics.params.iter().collect::<Vec<_>>();
	quote! {
		unsafe impl<'dyn_any, #(#old_params,)*> StaticType for #struct_name <#(#dyn_params,)*> {
			type Static =  #struct_name <#(#static_params,)*>;
		}
	}
	.into()
}

fn replace_lifetimes(generics: &syn::Generics, replacement: &str) -> Vec<proc_macro2::TokenStream> {
	let params = generics
		.params
		.iter()
		.map(|param| {
			let param = match param {
				GenericParam::Lifetime(_) => GenericParam::Lifetime(LifetimeParam::new(Lifetime::new(replacement, Span::call_site()))),
				GenericParam::Type(t) => {
					let mut t = t.clone();
					t.bounds.iter_mut().for_each(|bond| {
						if let TypeParamBound::Lifetime(ref mut t) = bond {
							*t = Lifetime::new(replacement, Span::call_site())
						}
					});
					GenericParam::Type(t.clone())
				}
				c => c.clone(),
			};
			quote! {#param}
		})
		.collect::<Vec<_>>();
	params
}
