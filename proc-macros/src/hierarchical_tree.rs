use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Fields, Type, parse2};

pub fn generate_hierarchical_tree(input: TokenStream) -> syn::Result<TokenStream> {
	let input = parse2::<DeriveInput>(input)?;
	let input_type = &input.ident;

	let data = match &input.data {
		Data::Enum(data) => data,
		_ => return Err(syn::Error::new(Span::call_site(), "Tried to derive HierarchicalTree for non-enum")),
	};

	let build_message_tree = data.variants.iter().map(|variant| {
		let variant_type = &variant.ident;

		let has_child = variant
			.attrs
			.iter()
			.any(|attr| attr.path().get_ident().map_or(false, |ident| ident == "sub_discriminant" || ident == "child"));

		if has_child {
			if let Fields::Unnamed(fields) = &variant.fields {
				let field_type = &fields.unnamed.first().unwrap().ty;
				quote! {
					{
						let mut variant_tree = DebugMessageTree::new(stringify!(#variant_type));
						let field_name = stringify!(#field_type);
						if "Message" == &field_name[field_name.len().saturating_sub(7)..] {
							// The field is a Message type, recursively build its tree
							let sub_tree = #field_type::build_message_tree();
							variant_tree.add_variant(sub_tree);
						}
						message_tree.add_variant(variant_tree);
					}
				}
			} else {
				quote! {
					message_tree.add_variant(DebugMessageTree::new(stringify!(#variant_type)));
				}
			}
		} else {
			quote! {
				message_tree.add_variant(DebugMessageTree::new(stringify!(#variant_type)));
			}
		}
	});

	let res = quote! {
		impl HierarchicalTree for #input_type {
			fn build_message_tree() -> DebugMessageTree {
				let mut message_tree = DebugMessageTree::new(stringify!(#input_type));
				#(#build_message_tree)*
				message_tree
			}
		}
	};

	Ok(res)
}
