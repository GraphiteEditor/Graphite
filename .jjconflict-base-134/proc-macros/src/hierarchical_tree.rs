use crate::helpers::clean_rust_type_syntax;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Fields, Type, parse2};

pub fn generate_hierarchical_tree(input: TokenStream) -> syn::Result<TokenStream> {
	let input = parse2::<DeriveInput>(input)?;
	let input_type = &input.ident;

	let line_number = input_type.span().start().line;

	let data = match &input.data {
		Data::Enum(data) => data,
		_ => return Err(syn::Error::new(Span::call_site(), "Tried to derive HierarchicalTree for non-enum")),
	};

	let build_message_tree: Result<Vec<_>, syn::Error> = data
		.variants
		.iter()
		.map(|variant| {
			let variant_type = &variant.ident;

			let has_child = variant
				.attrs
				.iter()
				.any(|attr| attr.path().get_ident().is_some_and(|ident| ident == "sub_discriminant" || ident == "child"));

			match &variant.fields {
				Fields::Unit => Ok(quote! {
					message_tree.add_variant(DebugMessageTree::new(stringify!(#variant_type)));
				}),
				Fields::Unnamed(fields) => {
					if has_child {
						let field_type = &fields.unnamed.first().unwrap().ty;
						Ok(quote! {
							{
								let mut variant_tree = DebugMessageTree::new(stringify!(#variant_type));
								let field_name = stringify!(#field_type);
								const MESSAGE_SUFFIX: &str = "Message";
								if MESSAGE_SUFFIX == &field_name[field_name.len().saturating_sub(MESSAGE_SUFFIX.len())..] {
									// The field is a Message type, recursively build its tree
									let sub_tree = #field_type::build_message_tree();
									variant_tree.add_variant(sub_tree);
								} else {
									variant_tree.add_fields(vec![format!("{field_name}")]);
								}
								message_tree.add_variant(variant_tree);
							}
						})
					} else {
						let error_msg = match fields.unnamed.len() {
							0 => format!("Remove the unnecessary `()` from the `{variant_type}` message enum variant."),
							1 => {
								let field_type = &fields.unnamed.first().unwrap().ty;
								format!(
									"The `{variant_type}` message should be defined as a struct-style (not tuple-style) enum variant to maintain consistent formatting across all editor messages.\n\
									Replace `{}` with a named field using {{curly braces}} instead of a positional field using (parentheses).",
									field_type.to_token_stream()
								)
							}
							_ => {
								let field_types = fields.unnamed.iter().map(|f| f.ty.to_token_stream().to_string()).collect::<Vec<_>>().join(", ");
								format!(
									"The `{variant_type}` message should be defined as a struct-style (not tuple-style) enum variant to maintain consistent formatting across all editor messages.\n\
									Replace `{field_types}` with named fields using {{curly braces}} instead of positional fields using (parentheses)."
								)
							}
						};
						Err(syn::Error::new(Span::call_site(), error_msg))
					}
				}
				Fields::Named(fields) => {
					let names = fields.named.iter().map(|f| f.ident.as_ref().unwrap());
					let ty = fields.named.iter().map(|f| clean_rust_type_syntax(f.ty.to_token_stream().to_string()));
					Ok(quote! {
						{
							let mut field_names = Vec::new();
							#(field_names.push(format!("{}: {}",stringify!(#names), #ty));)*
							let mut variant_tree = DebugMessageTree::new(stringify!(#variant_type));
							variant_tree.add_fields(field_names);
							message_tree.add_variant(variant_tree);
						}
					})
				}
			}
		})
		.collect();
	let build_message_tree = build_message_tree?;

	let res = quote! {
		impl HierarchicalTree for #input_type {
			fn build_message_tree() -> DebugMessageTree {
				let mut message_tree = DebugMessageTree::new(stringify!(#input_type));
				#(#build_message_tree)*

				let message_handler_str = #input_type::message_handler_str();
				message_tree.add_message_handler_field(message_handler_str);

				let message_handler_data_str = #input_type::message_handler_data_str();
				if message_handler_data_str.fields().len() > 0 {
					message_tree.add_message_handler_data_field(message_handler_data_str);
				}

				message_tree.set_path(file!());

				message_tree.set_line_number(#line_number);

				message_tree
			}
		}
	};

	Ok(res)
}
