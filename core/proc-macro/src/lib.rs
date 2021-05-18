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

/// Derive the `ToDiscriminant` trait and create a `<Type Name>Discriminant` enum
///
/// This derive macro is enum-only.
///
/// The discriminant enum is a copy of the input enum with all fields of every* variant removed.\
/// *) The exception to that rule is the `#[child]` attribute
///
/// # Helper attributes
/// - `#[sub_discriminant]`: only usable on tuple variants with a single field; instead of no fields, the discriminant of the single field will be included in the discriminant,
///     acting as a sub-discriminant.
/// - `#[discriminant_attr(…)]`: usable on the enum itself or on any variant; applies `#[…]` in its place on the discriminant.
///
/// # Attributes on the Discriminant
/// All attributes on variants and the type itself are cleared when constructing the discriminant.
/// If the discriminant is supposed to also have an attribute, you must double it with `#[discriminant_attr(…)]`
///
/// # Example
/// ```
/// # use graphite_proc_macros::ToDiscriminant;
/// # use editor_core::derivable_custom_traits::ToDiscriminant;
/// # use std::ffi::OsString;
///
/// #[derive(ToDiscriminant)]
/// #[discriminant_attr(derive(Debug, Eq, PartialEq))]
/// pub enum EnumA {
///     A(u8),
///     #[sub_discriminant]
///     B(EnumB)
/// }
///
/// #[derive(ToDiscriminant)]
/// #[discriminant_attr(derive(Debug, Eq, PartialEq))]
/// #[discriminant_attr(repr(u8))]
/// pub enum EnumB {
///     Foo(u8),
///     Bar(String),
///     #[cfg(feature = "some-feature")]
///     #[discriminant_attr(cfg(feature = "some-feature"))]
///     WindowsBar(OsString)
/// }
///
/// let a = EnumA::A(1);
/// assert_eq!(a.to_discriminant(), EnumADiscriminant::A);
/// let b = EnumA::B(EnumB::Bar("bar".to_string()));
/// assert_eq!(b.to_discriminant(), EnumADiscriminant::B(EnumBDiscriminant::Bar));
/// ```
#[proc_macro_derive(ToDiscriminant, attributes(sub_discriminant, discriminant_attr))]
pub fn derive_discriminant(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_discriminant_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

/// Derive the `TransitiveChild` trait and generate `From` impls to convert into the parent, as well as the top parent type
///
/// This macro cannot be invoked on the top parent (which has no parent but itself). Instead, implement `TransitiveChild` manually
/// like in the example.
///
/// # Helper Attributes
/// - `#[parent(<Type>, <Expr>)]` (**required**): declare the parent type (`<Type>`)
///     and a function (`<Expr>`, has to evaluate to a single arg function) for converting a value of this type to the parent type
/// - `#[parent_is_top]`: Denote that the parent type has no further parent type (this is required because otherwise the `From` impls for parent and top parent would overlap)
///
/// # Example
/// ```
/// # use graphite_proc_macros::TransitiveChild;
/// # use editor_core::derivable_custom_traits::TransitiveChild;
///
/// #[derive(Debug, Eq, PartialEq)]
/// struct A { u: u8, b: B };
///
/// impl A {
///     pub fn from_b(b: B) -> Self {
///         Self { u: 7, b }
///     }
/// }
///
/// impl TransitiveChild for A {
///     type Parent = Self;
///     type TopParent = Self;
/// }
///
/// #[derive(TransitiveChild, Debug, Eq, PartialEq)]
/// #[parent(A, A::from_b)]
/// #[parent_is_top]
/// enum B {
///     Foo,
///     Bar,
///     Child(C)
/// }
///
/// #[derive(TransitiveChild, Debug, Eq, PartialEq)]
/// #[parent(B, B::Child)]
/// struct C(D);
///
/// #[derive(TransitiveChild, Debug, Eq, PartialEq)]
/// #[parent(C, C)]
/// struct D;
///
/// let d = D;
/// assert_eq!(A::from(d), A { u: 7, b: B::Child(C(D)) });
/// ```
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
/// # use editor_core::derivable_custom_traits::Hint;
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
	use proc_macro2::TokenStream as TokenStream2;

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
