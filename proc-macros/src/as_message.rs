use proc_macro2::{Span, TokenStream};
use syn::{Data, DeriveInput};

pub fn derive_as_message_impl(input_item: TokenStream) -> syn::Result<TokenStream> {
	let input = syn::parse2::<DeriveInput>(input_item).unwrap();

	let data = match input.data {
		Data::Enum(data) => data,
		_ => return Err(syn::Error::new(Span::call_site(), "Tried to derive AsMessage for non-enum")),
	};

	let input_type = input.ident;

	let (globs, names) = data
		.variants
		.iter()
		.map(|var| {
			let var_name = &var.ident;
			let var_name_s = var.ident.to_string();
			if var.attrs.iter().any(|a| a.path().is_ident("child")) {
				(
					quote::quote! {
						#input_type::#var_name(child)
					},
					quote::quote! {
						format!("{}.{}", #var_name_s, child.local_name())
					},
				)
			} else {
				(
					quote::quote! {
						#input_type::#var_name { .. }
					},
					quote::quote! {
						#var_name_s.to_string()
					},
				)
			}
		})
		.unzip::<_, _, Vec<_>, Vec<_>>();

	let res = quote::quote! {
		impl AsMessage for #input_type {
			fn local_name(self) -> String {
				match self {
					#(
						#globs => #names
					),*
				}
			}
		}
	};

	Ok(res)
}
