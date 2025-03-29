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

	let variant_prints = data.variants.iter().enumerate().map(|(index, variant)| {
		let variant_type = &variant.ident;
		let is_last = index == data.variants.len() - 1;
		let tree_symbol = if is_last { "└──" } else { "├──" };

		let has_child = variant
			.attrs
			.iter()
			.any(|attr| attr.path().get_ident().map_or(false, |ident| ident == "sub_discriminant" || ident == "child"));

		if has_child {
			if let Fields::Unnamed(fields) = &variant.fields {
				let field_type = &fields.unnamed.first().unwrap().ty;
				quote! {
					tree.push(format!("{}{}{}", "│   ".repeat(depth), #tree_symbol, stringify!(#variant_type)));
					<#field_type>::generate_enum_variants(depth + 1, tree);
				}
			} else {
				quote! {
					tree.push(format!("{}{}{}", "│   ".repeat(depth), #tree_symbol, stringify!(#variant_type)));
				}
			}
		} else {
			quote! {
				tree.push(format!("{}{}{}", "│   ".repeat(depth), #tree_symbol, stringify!(#variant_type)));
			}
		}
	});

	let res = quote! {
		impl HierarchicalTree for #input_type {
			fn generate_hierarchical_tree() -> Vec<String> {
				let mut hierarchical_tree = Vec::new();
				hierarchical_tree.push(format!("{}", stringify!(#input_type)));
				Self::generate_enum_variants(0, &mut hierarchical_tree);
				hierarchical_tree
			}

			fn generate_enum_variants(depth: usize, tree: &mut Vec<String>) {
				#(#variant_prints)*
			}
		}
	};

	Ok(res)
}
