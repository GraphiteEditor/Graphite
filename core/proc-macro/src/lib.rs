mod helpers;
mod structs;

use crate::helpers::{fold_error_iter, two_path};
use crate::structs::{AttrInnerKeyStringMap, AttrInnerSingleString};
use proc_macro::TokenStream;
use proc_macro2::Span;
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
			let v: Vec<(LitStr, LitStr)> = fold_error_iter(parsed.into_hashmap().into_iter().map(|(k, mut v)| match v.len() {
				0 => panic!("internal error: a key without values was somehow inserted into the hashmap"),
				1 => {
					let single_val = v.pop().unwrap();
					Ok((LitStr::new(&k.to_string(), Span::call_site()), single_val))
				}
				n => Err(syn::Error::new(tokens_span, format!("multiple hints for the same key ({} hints for {:?})", n, k))),
			}))?;

			Ok(v.into_iter().unzip())
		}
		n => Err(syn::Error::new(whole_span, format!("too many `hint` attributes for {} (expected 1, got {})", item_type, n))),
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
	let input = parse_macro_input!(input_item as DeriveInput);

	let span = input.span();
	let ident = input.ident;

	let output_result = match input.data {
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
						fn hints(&self) -> HashMap<String, String> {
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
	};

	let output = output_result.unwrap_or_else(|err| err.to_compile_error());

	TokenStream::from(output)
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
