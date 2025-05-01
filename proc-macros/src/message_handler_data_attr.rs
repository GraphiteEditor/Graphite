use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{ItemImpl, Type, parse2, spanned::Spanned};

pub fn message_handler_data_attr_impl(attr: TokenStream, input_item: TokenStream) -> syn::Result<TokenStream> {
	// Parse the input as an impl block
	let impl_block = parse2::<ItemImpl>(input_item.clone())?;

	let self_ty = &impl_block.self_ty;

	let path = match &**self_ty {
		Type::Path(path) => &path.path,
		_ => return Err(syn::Error::new(Span::call_site(), "Expected impl implementation")),
	};

	let input_type = path.segments.last().map(|s| &s.ident).unwrap();

	// Extract the message type from the trait path
	let trait_path = match &impl_block.trait_ {
		Some((_, path, _)) => path,
		None => return Err(syn::Error::new(Span::call_site(), "Expected trait implementation")),
	};

	// Get the trait generics (should be MessageHandler<M, D>)
	if let Some(segment) = trait_path.segments.last() {
		if segment.ident != "MessageHandler" {
			return Err(syn::Error::new(segment.ident.span(), "Expected MessageHandler trait"));
		}
		if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
			if args.args.len() >= 2 {
				// Extract the message type (M) and data type (D) from the trait params
				let message_type = &args.args[0];
				let data_type = &args.args[1];

				// Check if the attribute is "CustomData"
				let is_custom_data = attr.to_string().contains("CustomData");

				let impl_item = match data_type {
					syn::GenericArgument::Type(t) => {
						match t {
							syn::Type::Path(type_path) if !type_path.path.segments.is_empty() => {
								// Get just the base identifier (ToolMessageData) without generics
								let type_name = &type_path.path.segments.first().unwrap().ident;

								if is_custom_data {
									quote! {
										#input_item
										impl #message_type {
											pub fn message_handler_data_str() -> Vec<String> {
												custom_data()
											}
											pub fn message_handler_str() -> Vec<String> {
												#input_type::field_types()
											}
										}
									}
								} else {
									quote! {
										#input_item
										impl #message_type {
											pub fn message_handler_data_str() -> Vec<String> {
												#type_name::field_types()
											}
											pub fn message_handler_str() -> Vec<String> {
												#input_type::field_types()
											}
										}
									}
								}
							}
							syn::Type::Tuple(_) => quote! {
								#input_item
								impl #message_type {
										pub fn message_handler_str() -> Vec<String> {
											#input_type::field_types()
										}
									}
							},
							_ => return Err(syn::Error::new(t.span(), "Unsupported type format")),
						}
					}

					_ => quote! {
						#input_item
					},
				};
				return Ok(impl_item);
			}
		}
	}
	Ok(input_item)
}
