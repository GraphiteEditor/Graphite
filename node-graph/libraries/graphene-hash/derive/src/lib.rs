extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derives `CacheHash` for a struct or enum.
///
/// All fields must implement `CacheHash`. Fields annotated with `#[cache_hash(skip)]`
/// are excluded from hashing.
///
/// # Example
///
/// ```
/// # use graphene_hash::CacheHash;
/// #[derive(CacheHash)]
/// pub struct MyNode {
///     pub value: f64,
///     pub count: u32,
///     #[cache_hash(skip)]
///     pub debug_label: String,
/// }
/// ```
#[proc_macro_derive(CacheHash, attributes(cache_hash))]
pub fn derive_cache_hash(input: TokenStream) -> TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let name = &ast.ident;
	let mut generics = ast.generics.clone();
	for param in &mut generics.params {
		if let syn::GenericParam::Type(type_param) = param {
			type_param.bounds.push(syn::parse_quote!(graphene_hash::CacheHash));
		}
	}
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let body = match &ast.data {
		Data::Struct(s) => hash_fields(&s.fields, quote! { self }),
		Data::Enum(e) => {
			let arms = e.variants.iter().map(|variant| {
				let variant_name = &variant.ident;
				let (pattern, hash_body) = match &variant.fields {
					Fields::Unit => (quote! {}, quote! {}),
					Fields::Unnamed(fields) => {
						let bindings: Vec<_> = (0..fields.unnamed.len())
							.map(|i| {
								let ident = proc_macro2::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site());
								quote! { #ident }
							})
							.collect();
						let hash_stmts = fields.unnamed.iter().enumerate().filter_map(|(i, field)| {
							if has_skip_attr(&field.attrs) {
								return None;
							}
							let ident = proc_macro2::Ident::new(&format!("f{i}"), proc_macro2::Span::call_site());
							Some(quote! { graphene_hash::CacheHash::cache_hash(#ident, state); })
						});
						(quote! { (#(#bindings,)*) }, quote! { #(#hash_stmts)* })
					}
					Fields::Named(fields) => {
						let names: Vec<_> = fields.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
						let hash_stmts = fields.named.iter().filter_map(|field| {
							if has_skip_attr(&field.attrs) {
								return None;
							}
							let ident = field.ident.as_ref().unwrap();
							Some(quote! { graphene_hash::CacheHash::cache_hash(#ident, state); })
						});
						(quote! { { #(#names,)* } }, quote! { #(#hash_stmts)* })
					}
				};
				quote! {
					Self::#variant_name #pattern => { #hash_body }
				}
			});
			quote! {
				::core::hash::Hash::hash(&::core::mem::discriminant(self), state);
				match self {
					#(#arms)*
				}
			}
		}
		Data::Union(_) => return syn::Error::new(ast.ident.span(), "CacheHash cannot be derived for unions").to_compile_error().into(),
	};

	quote! {
		#[allow(clippy::derived_hash_with_manual_eq)]
		impl #impl_generics graphene_hash::CacheHash for #name #ty_generics #where_clause {
			fn cache_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
				#body
			}
		}
	}
	.into()
}

fn hash_fields(fields: &Fields, self_expr: TokenStream2) -> TokenStream2 {
	match fields {
		Fields::Unit => quote! {},
		Fields::Unnamed(fields) => {
			let stmts = fields.unnamed.iter().enumerate().filter_map(|(i, field)| {
				if has_skip_attr(&field.attrs) {
					return None;
				}
				let index = syn::Index::from(i);
				Some(quote! { graphene_hash::CacheHash::cache_hash(&#self_expr.#index, state); })
			});
			quote! { #(#stmts)* }
		}
		Fields::Named(fields) => {
			let stmts = fields.named.iter().filter_map(|field| {
				if has_skip_attr(&field.attrs) {
					return None;
				}
				let ident = field.ident.as_ref().unwrap();
				Some(quote! { graphene_hash::CacheHash::cache_hash(&#self_expr.#ident, state); })
			});
			quote! { #(#stmts)* }
		}
	}
}

fn has_skip_attr(attrs: &[syn::Attribute]) -> bool {
	attrs.iter().any(|attr| {
		if !attr.path().is_ident("cache_hash") {
			return false;
		}
		attr.parse_args::<syn::Ident>().map(|id| id == "skip").unwrap_or(false)
	})
}
