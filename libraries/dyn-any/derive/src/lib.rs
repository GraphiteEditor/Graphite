#![doc(html_root_url = "http://docs.rs/dyn-any-derive/0.1.0")]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
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

	// A container's type parameter maps through its own projection, so `List<Graphic<'e>>`
	// erases and relifts its contents. A value type whose parameter carries trait bounds
	// cannot: the projected type need not satisfy them, so it passes through instead, and
	// the `'static` bound keeps it borrow-free. `#[dyn_any_derive(project)]` picks the first.
	let projects = ast
		.attrs
		.iter()
		.any(|attr| attr.path().is_ident("dyn_any_derive") && attr.parse_args::<syn::Ident>().is_ok_and(|arg| arg == "project"));

	let dyn_params = generic_arguments(generics, "'dyn_any");
	let static_params = match projects {
		true => projected_arguments(generics, "'static", |ident| quote! { <#ident as dyn_any::StaticTypeSized>::Static }),
		false => projected_arguments(generics, "'static", |ident| quote! { #ident }),
	};
	let live_params = match projects {
		true => projected_arguments(generics, "'dyn_any_live", |ident| quote! { <#ident as dyn_any::Relift>::Live<'dyn_any_live> }),
		false => projected_arguments(generics, "'dyn_any_live", |ident| quote! { #ident }),
	};
	let self_params = projected_arguments(generics, "'static", |ident| quote! { #ident });

	// The declared bounds come along, since the impl restates the struct's own parameters.
	let bounded = |bound: TypeParamBound| -> Vec<TokenStream2> {
		generics
			.params
			.iter()
			.filter_map(|param| match param {
				GenericParam::Type(t) => {
					let mut t = t.clone();
					t.bounds.push(bound.clone());
					Some(quote! { #t })
				}
				GenericParam::Const(c) => Some(quote! { #c }),
				GenericParam::Lifetime(_) => None,
			})
			.collect()
	};
	let static_bound: TypeParamBound = match projects {
		true => syn::parse_quote!(dyn_any::StaticTypeSized),
		false => TypeParamBound::Lifetime(Lifetime::new("'static", Span::call_site())),
	};
	let relift_bound: TypeParamBound = match projects {
		true => syn::parse_quote!(dyn_any::Relift),
		false => TypeParamBound::Lifetime(Lifetime::new("'static", Span::call_site())),
	};
	let static_bounds = bounded(static_bound);
	let relift_bounds = bounded(relift_bound);

	// A projected parameter must still satisfy what the struct declared of it, which only
	// the author can promise: `Image<P: Pixel>` needs `P::Static: Pixel` to name its own
	// static form. The relift side quantifies over the lifetime the associated type takes.
	let declared_bounds = |project: &dyn Fn(&syn::Ident) -> TokenStream2| -> Vec<TokenStream2> {
		generics
			.params
			.iter()
			.filter_map(|param| match param {
				GenericParam::Type(t) if projects && !t.bounds.is_empty() => {
					let projected = project(&t.ident);
					let bounds = &t.bounds;
					Some(quote! { #projected: #bounds })
				}
				_ => None,
			})
			.collect()
	};
	let static_where = declared_bounds(&|ident| quote! { <#ident as dyn_any::StaticTypeSized>::Static });
	let relift_where = declared_bounds(&|ident| quote! { <#ident as dyn_any::Relift>::Live<'dyn_any_live> });
	let relift_where = (!relift_where.is_empty()).then(|| quote! { where for<'dyn_any_live> #(#relift_where),* });

	quote! {
		unsafe impl<'dyn_any, #(#static_bounds,)*> dyn_any::StaticType for #struct_name <#(#dyn_params,)*>
		where #(#static_where,)*
		{
			type Static =  #struct_name <#(#static_params,)*>;
		}

		unsafe impl<#(#relift_bounds,)*> dyn_any::Relift for #struct_name <#(#self_params,)*>
			#relift_where
		{
			type Live<'dyn_any_live> = #struct_name <#(#live_params,)*>;
		}
	}
	.into()
}

/// The struct's generic arguments with lifetimes replaced and type parameters put
/// through `project`, so a container's argument can be mapped rather than passed on.
fn projected_arguments(generics: &syn::Generics, replacement: &str, project: impl Fn(&syn::Ident) -> TokenStream2) -> Vec<TokenStream2> {
	generics
		.params
		.iter()
		.map(|param| match param {
			GenericParam::Lifetime(_) => {
				let lifetime = Lifetime::new(replacement, Span::call_site());
				quote! {#lifetime}
			}
			GenericParam::Type(t) => project(&t.ident),
			GenericParam::Const(c) => {
				let ident = &c.ident;
				quote! {#ident}
			}
		})
		.collect()
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
