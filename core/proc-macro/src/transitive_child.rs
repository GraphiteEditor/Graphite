use crate::helper_structs::Pair;
use proc_macro2::{Ident, Span, TokenStream};
use syn::{Attribute, DeriveInput, Type};

pub fn derive_transitive_child_impl(input_item: TokenStream) -> syn::Result<TokenStream> {
	let input = syn::parse2::<DeriveInput>(input_item).unwrap();

	let Attribute { tokens, .. } = input
		.attrs
		.iter()
		.find(|a| a.path.is_ident("parent"))
		.ok_or_else(|| syn::Error::new(Span::call_site(), format!("tried to derive TransitiveChild without a #[parent] attribute (on {})", input.ident)))?;

	let parent_is_top = input.attrs.iter().any(|a| a.path.is_ident("parent_is_top"));

	let Pair {
		first: parent_type, second: variant, ..
	} = syn::parse2::<Pair<Type, Ident>>(tokens.clone())?;

	let top_parent_type: Type = syn::parse_quote! { <#parent_type as TransitiveChild>::TopParent };

	let input_type = &input.ident;

	let trait_impl = quote::quote! {
		impl TransitiveChild for #input_type {
			type Parent = #parent_type;
			type TopParent = #top_parent_type;
		}
	};

	let from_parent = quote::quote! {
		impl From<#input_type> for #parent_type {
			fn from(x: #input_type) -> #parent_type {
				#parent_type::#variant(x)
			}
		}
	};

	let from_top = quote::quote! {
		impl From<#input_type> for #top_parent_type {
			fn from(x: #input_type) -> #top_parent_type {
				#top_parent_type::from(#parent_type::#variant(x))
			}
		}
	};

	Ok(if parent_is_top {
		quote::quote! { #trait_impl #from_parent }
	} else {
		quote::quote! { #trait_impl #from_parent #from_top }
	})
}
