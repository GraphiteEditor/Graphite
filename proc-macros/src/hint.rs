use crate::helper_structs::AttrInnerKeyStringMap;
use crate::helpers::{fold_error_iter, two_segment_path};
use proc_macro2::{Span, TokenStream as TokenStream2};
use syn::{Attribute, Data, DeriveInput, LitStr, Variant};

fn parse_hint_helper_attrs(attrs: &[Attribute]) -> syn::Result<(Vec<LitStr>, Vec<LitStr>)> {
	fold_error_iter(
		attrs
			.iter()
			.filter(|a| a.path().get_ident().map_or(false, |i| i == "hint"))
			.map(|attr| attr.parse_args::<AttrInnerKeyStringMap>()),
	)
	.and_then(|v: Vec<AttrInnerKeyStringMap>| {
		fold_error_iter(AttrInnerKeyStringMap::multi_into_iter(v).map(|(k, mut v)| match v.len() {
			0 => panic!("internal error: a key without values was somehow inserted into the hashmap"),
			1 => {
				let single_val = v.pop().unwrap();
				Ok((LitStr::new(&k.to_string(), Span::call_site()), single_val))
			}
			_ => {
				// the first value is ok, the other ones should error
				let after_first = v.into_iter().skip(1);
				// this call to fold_error_iter will always return Err with a combined error
				fold_error_iter(after_first.map(|lit| Err(syn::Error::new(lit.span(), format!("value for key {k} was already given"))))).map(|_: Vec<()>| unreachable!())
			}
		}))
	})
	.map(|v| v.into_iter().unzip())
}

pub fn derive_hint_impl(input_item: TokenStream2) -> syn::Result<TokenStream2> {
	let input = syn::parse2::<DeriveInput>(input_item)?;

	let ident = input.ident;

	match input.data {
		Data::Enum(data) => {
			let variants = data.variants.iter().map(|var: &Variant| two_segment_path(ident.clone(), var.ident.clone())).collect::<Vec<_>>();

			let hint_result = fold_error_iter(data.variants.into_iter().map(|var: Variant| parse_hint_helper_attrs(&var.attrs)));

			hint_result.map(|hints: Vec<(Vec<LitStr>, Vec<LitStr>)>| {
				let (keys, values): (Vec<Vec<LitStr>>, Vec<Vec<LitStr>>) = hints.into_iter().unzip();
				let cap: Vec<usize> = keys.iter().map(|v| v.len()).collect();

				quote::quote! {
					impl Hint for #ident {
						fn hints(&self) -> ::std::collections::HashMap<String, String> {
							match self {
								#(
									#variants { .. } => {
										let mut hm = ::std::collections::HashMap::with_capacity(#cap);
										#(
											hm.insert(#keys.to_string(), #values.to_string());
										)*
										hm
									}
								)*
							}
						}
					}
				}
			})
		}
		Data::Struct(_) | Data::Union(_) => {
			let hint_result = parse_hint_helper_attrs(&input.attrs);

			hint_result.map(|(keys, values)| {
				let cap = keys.len();

				quote::quote! {
					impl Hint for #ident {
						fn hints(&self) -> ::std::collections::HashMap<String, String> {
							let mut hm = ::std::collections::HashMap::with_capacity(#cap);
							#(
								hm.insert(#keys.to_string(), #values.to_string());
							)*
							hm
						}
					}
				}
			})
		}
	}
}
