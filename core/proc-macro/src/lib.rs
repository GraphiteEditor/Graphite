mod as_message;
mod combined_message_attrs;
mod discriminant;
mod helper_structs;
mod helpers;
mod hint;
mod transitive_child;

use crate::as_message::derive_as_message_impl;
use crate::combined_message_attrs::combined_message_attrs_impl;
use crate::discriminant::derive_discriminant_impl;
use crate::helper_structs::AttrInnerSingleString;
use crate::hint::derive_hint_impl;
use crate::transitive_child::derive_transitive_child_impl;
use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(ToDiscriminant, attributes(child, discriminant_derive, discriminant_attr))]
pub fn derive_discriminant(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_discriminant_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

// todo: revert so that parent takes an expr as second arg again
#[proc_macro_derive(TransitiveChild, attributes(parent, parent_is_top))]
pub fn derive_transitive_child(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_transitive_child_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

#[proc_macro_derive(AsMessage, attributes(child))]
pub fn derive_message(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_as_message_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

#[proc_macro_attribute]
pub fn impl_message(attr: TokenStream, input_item: TokenStream) -> TokenStream {
	TokenStream::from(combined_message_attrs_impl(attr.into(), input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
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

	/*#[test]
	fn foo() {
		let res = derive_message_impl(quote::quote! {
			enum E {
				S { a: u8, b: String, c: bool },
				X,
				Y
			}
		}
		panic!("{:?}", res)
	}*/

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
