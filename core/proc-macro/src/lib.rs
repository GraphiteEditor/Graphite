mod helpers;
mod structs;

use crate::helpers::{fold_error_iter, two_path};
use crate::structs::{AttrInnerKeyStringMap, AttrInnerSingleString};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, LitStr, Variant};

fn parse_hint_helper_attrs(attrs: &[Attribute], whole_span: Span, item_type: &str) -> syn::Result<(Vec<LitStr>, Vec<LitStr>)> {
	let mut v = attrs.iter().filter(|a| a.path.get_ident().map_or(false, |i| i == "hint")).collect::<Vec<_>>();
	match v.len() {
		0 => {
			// no hint attribute -> no hints
			Ok((Vec::new(), Vec::new()))
		}
		1 => {
			let attr = v.pop().unwrap();
			let tokens_span = attr.tokens.span();

			let parsed = syn::parse2::<AttrInnerKeyStringMap>(attr.tokens.clone())?;
			let v: Vec<(LitStr, LitStr)> = fold_error_iter(parsed.into_iter().map(|(k, mut v)| match v.len() {
				0 => panic!("internal error: a key without values was somehow inserted into the hashmap"),
				1 => {
					let single_val = v.pop().unwrap();
					Ok((LitStr::new(&k.to_string(), Span::call_site()), single_val))
				}
				n => Err(syn::Error::new(tokens_span, format!("multiple hints for the same key ({} hints for {:?})", n, k))),
			}))?;

			Ok(v.into_iter().unzip())
		}
		// TODO: just join multiple attrs together
		n => Err(syn::Error::new(whole_span, format!("too many `hint` attributes for {} (expected 1, got {})", item_type, n))),
	}
}

fn derive_hint_impl(input_item: TokenStream2) -> syn::Result<TokenStream2> {
	let input = syn::parse2::<DeriveInput>(input_item)?;

	let span = input.span();
	let ident = input.ident;

	match input.data {
		Data::Enum(data) => {
			let variants = data.variants.iter().map(|var: &Variant| two_path(ident.clone(), var.ident.clone())).collect::<Vec<_>>();

			let hint_result = fold_error_iter(data.variants.into_iter().map(|var: Variant| parse_hint_helper_attrs(&var.attrs, var.span(), "variant")));

			hint_result.map(|hints: Vec<(Vec<LitStr>, Vec<LitStr>)>| {
				let (keys, values): (Vec<Vec<LitStr>>, Vec<Vec<LitStr>>) = hints.into_iter().unzip();

				quote::quote! {
					impl Hint for #ident {
						fn hints(&self) -> ::std::collections::HashMap<String, String> {
							let mut hm = ::std::collections::HashMap::new();
							match self {
								#(
									#variants { .. } => {
										#(
											hm.insert(#keys.to_string(), #values.to_string());
										)*
									}
								)*
							}
							hm
						}
					}
				}
			})
		}
		Data::Struct(_) | Data::Union(_) => {
			let hint_result = parse_hint_helper_attrs(&input.attrs, span, "struct");

			hint_result.map(|(keys, values)| {
				quote::quote! {
					impl Hint for #ident {
						fn hints(&self) -> ::std::collections::HashMap<String, String> {
							let mut hm = ::std::collections::HashMap::new();
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
						let mut hm = ::std::collections::HashMap::new();
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
						let mut hm = ::std::collections::HashMap::new();
						match self {
							E::S { .. } => {
								hm.insert("key1".to_string(), "val1".to_string());
								hm.insert("key2".to_string(), "val2".to_string());
							}
							E::X { .. } => {
								hm.insert("key3".to_string(), "val3".to_string());
							}
							E::Y { .. } => {

							}
						}
						hm
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
						let mut hm = ::std::collections::HashMap::new();
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

		// TODO: change this when that is no longer an error
		let res = derive_hint_impl(quote::quote! {
			#[hint(a="1")]
			#[hint(b="2")]
			struct S;
		});
		assert!(res.is_err());
	}

	// note: edge needs no testing since AttrInnerSingleString has testing and that's all you'd need to test with edge
}
