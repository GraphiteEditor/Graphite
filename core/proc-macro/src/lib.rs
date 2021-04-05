mod helpers;
mod structs;

use crate::helpers::{fold_error_iter, two_path};
use crate::structs::{AttrInnerKeyStringMap, AttrInnerSingleString};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use syn::{parse_macro_input, Attribute, Data, DeriveInput, LitStr, Variant};

fn parse_hint_helper_attrs(attrs: &[Attribute]) -> syn::Result<(Vec<LitStr>, Vec<LitStr>)> {
	fold_error_iter(
		attrs
			.iter()
			.filter(|a| a.path.get_ident().map_or(false, |i| i == "hint"))
			.map(|attr| syn::parse2::<AttrInnerKeyStringMap>(attr.tokens.clone())),
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
				fold_error_iter(after_first.map(|lit| Err(syn::Error::new(lit.span(), format!("value for key {} was already given", k))))).map(|_: Vec<()>| unreachable!())
			}
		}))
	})
	.map(|v| v.into_iter().unzip())
}

fn derive_hint_impl(input_item: TokenStream2) -> syn::Result<TokenStream2> {
	let input = syn::parse2::<DeriveInput>(input_item)?;

	let ident = input.ident;

	match input.data {
		Data::Enum(data) => {
			let variants = data.variants.iter().map(|var: &Variant| two_path(ident.clone(), var.ident.clone())).collect::<Vec<_>>();

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

/// Derive the `Hint` trait
///
/// # Example
/// ```
/// # use graphite_proc_macros::Hint;
/// # use editor_core::hint::Hint;
///
/// #[derive(Hint)]
/// pub enum StateMachine {
///     #[hint(rmb = "foo", lmb = "bar")]
///     Ready,
///     #[hint(alt = "baz")]
///     RMBDown,
///     // no hint (also ok)
///     LMBDown
/// }
/// ```
#[proc_macro_derive(Hint, attributes(hint))]
pub fn derive_hint(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_hint_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

/// The `edge` proc macro does nothing, it is intended for use with an external tool
///
/// # Example
/// ```ignore
/// match (example_tool_state, event) {
///     (ToolState::Ready, Event::MouseDown(mouse_state)) if *mouse_state == MouseState::Left => {
///         #[edge("LMB Down")]
///         ToolState::Pending
///     }
///     (SelectToolState::Pending, Event::MouseUp(mouse_state)) if *mouse_state == MouseState::Left => {
///         #[edge("LMB Up: Select Object")]
///         SelectToolState::Ready
///     }
///     (SelectToolState::Pending, Event::MouseMove(x,y)) => {
///         #[edge("Mouse Move")]
///         SelectToolState::TransformSelected
///     }
///     (SelectToolState::TransformSelected, Event::MouseMove(x,y)) => {
///         #[egde("Mouse Move")]
///         SelectToolState::TransformSelected
///     }
///     (SelectToolState::TransformSelected, Event::MouseUp(mouse_state)) if *mouse_state == MouseState::Left =>  {
///         #[edge("LMB Up")]
///         SelectToolState::Ready
///     }
///     (state, _) => {
///         // Do nothing
///         state
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn edge(attr: TokenStream, item: TokenStream) -> TokenStream {
	// to make sure that only `#[edge("string")]` is allowed
	let _verify = parse_macro_input!(attr as AttrInnerSingleString);

	item
}

#[cfg(test)]
mod tests {
	use super::*;

	fn ts_assert_eq(l: TokenStream2, r: TokenStream2) {
		// not sure if this is the best way of doing things but if two TokenStreams are equal, their `to_string` is also equal
		// so there are at least no false negatives
		assert_eq!(l.to_string(), r.to_string());
	}

	#[test]
	fn test_derive_hint() {
		let res = derive_hint_impl(quote::quote! {
			#[hint(key1="val1",key2="val2",)]
			struct S { a: u8, b: String, c: bool }
		});
		assert!(res.is_ok());
		ts_assert_eq(
			res.unwrap(),
			quote::quote! {
				impl Hint for S {
					fn hints(&self) -> ::std::collections::HashMap<String, String> {
						let mut hm = ::std::collections::HashMap::with_capacity(2usize);
						hm.insert("key1".to_string(), "val1".to_string());
						hm.insert("key2".to_string(), "val2".to_string());
						hm
					}
				}
			},
		);

		let res = derive_hint_impl(quote::quote! {
			enum E {
				#[hint(key1="val1",key2="val2",)]
				S { a: u8, b: String, c: bool },
				#[hint(key3="val3")]
				X,
				Y
			}
		});
		assert!(res.is_ok());
		ts_assert_eq(
			res.unwrap(),
			quote::quote! {
				impl Hint for E {
					fn hints(&self) -> ::std::collections::HashMap<String, String> {
						match self {
							E::S { .. } => {
								let mut hm = ::std::collections::HashMap::with_capacity(2usize);
								hm.insert("key1".to_string(), "val1".to_string());
								hm.insert("key2".to_string(), "val2".to_string());
								hm
							}
							E::X { .. } => {
								let mut hm = ::std::collections::HashMap::with_capacity(1usize);
								hm.insert("key3".to_string(), "val3".to_string());
								hm
							}
							E::Y { .. } => {
								let mut hm = ::std::collections::HashMap::with_capacity(0usize);
								hm
							}
						}
					}
				}
			},
		);

		let res = derive_hint_impl(quote::quote! {
			union NoHint {}
		});
		assert!(res.is_ok());
		ts_assert_eq(
			res.unwrap(),
			quote::quote! {
				impl Hint for NoHint {
					fn hints(&self) -> ::std::collections::HashMap<String, String> {
						let mut hm = ::std::collections::HashMap::with_capacity(0usize);
						hm
					}
				}
			},
		);

		let res = derive_hint_impl(quote::quote! {
			#[hint(a="1", a="2")]
			struct S;
		});
		assert!(res.is_err());

		let res = derive_hint_impl(quote::quote! {
			#[hint(a="1")]
			#[hint(b="2")]
			struct S;
		});
		assert!(res.is_ok());
		ts_assert_eq(
			res.unwrap(),
			quote::quote! {
				impl Hint for S {
					fn hints(&self) -> ::std::collections::HashMap<String, String> {
						let mut hm = ::std::collections::HashMap::with_capacity(2usize);
						hm.insert("a".to_string(), "1".to_string());
						hm.insert("b".to_string(), "2".to_string());
						hm
					}
				}
			},
		)
	}

	// note: edge needs no testing since AttrInnerSingleString has testing and that's all you'd need to test with edge
}
