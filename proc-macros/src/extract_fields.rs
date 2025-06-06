use crate::helpers::clean_rust_type_syntax;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{Data, DeriveInput, Fields, Type, parse2};

pub fn derive_extract_field_impl(input: TokenStream) -> syn::Result<TokenStream> {
	let input = parse2::<DeriveInput>(input)?;
	let struct_name = &input.ident;
	let generics = &input.generics;
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let fields = match &input.data {
		Data::Struct(data) => match &data.fields {
			Fields::Named(fields) => &fields.named,
			_ => return Err(syn::Error::new(Span::call_site(), "ExtractField only works on structs with named fields")),
		},
		_ => return Err(syn::Error::new(Span::call_site(), "ExtractField only works on structs")),
	};

	let mut field_line = Vec::new();
	// Extract field names and types as strings at compile time
	let field_info = fields
		.iter()
		.map(|field| {
			let ident = field.ident.as_ref().unwrap();
			let name = ident.to_string();
			let ty = clean_rust_type_syntax(field.ty.to_token_stream().to_string());
			let line = ident.span().start().line;
			field_line.push(line);
			(name, ty)
		})
		.collect::<Vec<_>>();

	let field_str = field_info.into_iter().map(|(name, ty)| (format!("{}: {}", name, ty)));

	let res = quote! {
		impl #impl_generics #struct_name #ty_generics #where_clause {
			pub fn field_types() -> Vec<(String, usize)> {
				vec![
					#((String::from(#field_str), #field_line)),*
				]
			}

			pub fn print_field_types() {
				for (field, line) in Self::field_types() {
					println!("{} at line {}", field, line);
				}
			}

			pub fn path() -> &'static str {
				file!()
			}
		}
	};

	Ok(res)
}
