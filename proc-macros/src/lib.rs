mod as_message;
mod combined_message_attrs;
mod discriminant;
mod helper_structs;
mod helpers;
mod hint;
mod transitive_child;
mod widget_builder;

use crate::as_message::derive_as_message_impl;
use crate::combined_message_attrs::combined_message_attrs_impl;
use crate::discriminant::derive_discriminant_impl;
use crate::helper_structs::AttrInnerSingleString;
use crate::hint::derive_hint_impl;
use crate::transitive_child::derive_transitive_child_impl;
use crate::widget_builder::derive_widget_builder_impl;

use proc_macro::TokenStream;

/// Derive the `ToDiscriminant` trait and create a `<Type Name>Discriminant` enum
///
/// This derive macro is enum-only.
///
/// The discriminant enum is a copy of the input enum with all fields of every variant removed.
/// The exception to that rule is the `#[child]` attribute.
///
/// # Helper attributes
/// - `#[sub_discriminant]`: only usable on variants with a single field; instead of no fields, the discriminant of the single field will be included in the discriminant,
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
/// # use editor::utility_traits::ToDiscriminant;
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
/// # use editor::utility_traits::TransitiveChild;
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

/// Derive the `AsMessage` trait
///
/// # Helper Attributes
/// - `#[child]`: only on tuple variants with a single field; Denote that the message path should continue inside the variant
///
/// # Example
/// See also [`TransitiveChild`]
/// ```
/// # use graphite_proc_macros::{TransitiveChild, AsMessage};
/// # use editor::utility_traits::TransitiveChild;
/// # use editor::messages::prelude::*;
///
/// #[derive(AsMessage)]
/// pub enum TopMessage {
///     A(u8),
///     B(u16),
///     #[child]
///     C(MessageC),
///     #[child]
///     D(MessageD)
/// }
///
/// impl TransitiveChild for TopMessage {
///     type Parent = Self;
///     type TopParent = Self;
/// }
///
/// #[derive(TransitiveChild, AsMessage, Copy, Clone)]
/// #[parent(TopMessage, TopMessage::C)]
/// #[parent_is_top]
/// pub enum MessageC {
///     X1,
///     X2
/// }
///
/// #[derive(TransitiveChild, AsMessage, Copy, Clone)]
/// #[parent(TopMessage, TopMessage::D)]
/// #[parent_is_top]
/// pub enum MessageD {
///     Y1,
///     #[child]
///     Y2(MessageE)
/// }
///
/// #[derive(TransitiveChild, AsMessage, Copy, Clone)]
/// #[parent(MessageD, MessageD::Y2)]
/// pub enum MessageE {
///     Alpha,
///     Beta
/// }
///
/// let c = MessageC::X1;
/// assert_eq!(c.local_name(), "X1");
/// assert_eq!(c.global_name(), "C.X1");
/// let d = MessageD::Y2(MessageE::Alpha);
/// assert_eq!(d.local_name(), "Y2.Alpha");
/// assert_eq!(d.global_name(), "D.Y2.Alpha");
/// let e = MessageE::Beta;
/// assert_eq!(e.local_name(), "Beta");
/// assert_eq!(e.global_name(), "D.Y2.Beta");
/// ```
#[proc_macro_derive(AsMessage, attributes(child))]
pub fn derive_message(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_as_message_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

/// This macro is basically an abbreviation for the usual [ToDiscriminant], [TransitiveChild] and [AsMessage] invocations.
///
/// This macro is enum-only.
///
/// Also note that all three of those derives have to be in scope.
///
/// # Usage
/// There are three possible argument syntaxes you can use:
/// 1. no arguments: this is for the top-level message enum. It derives `ToDiscriminant`, `AsMessage` on the discriminant, and implements `TransitiveChild` on both
///     (the parent and top parent being the respective types themselves).
///     It also derives the following `std` traits on the discriminant: `Debug, Copy, Clone, PartialEq, Eq, Hash`.
/// 2. two arguments: this is for message enums whose direct parent is the top level message enum. The syntax is `#[impl_message(<Type>, <Ident>)]`,
///     where `<Type>` is the parent message type and `<Ident>` is the identifier of the variant used to construct this child.
///     It derives `ToDiscriminant`, `AsMessage` on the discriminant, and `TransitiveChild` on both (adding `#[parent_is_top]` to both).
///     It also derives the following `std` traits on the discriminant: `Debug, Copy, Clone, PartialEq, Eq, Hash`.
/// 3. three arguments: this is for all other message enums that are transitive children of the top level message enum. The syntax is
///     `#[impl_message(<Type>, <Type>, <Ident>)]`, where the first `<Type>` is the top parent message type, the second `<Type>` is the parent message type
///     and `<Ident>` is the identifier of the variant used to construct this child.
///     It derives `ToDiscriminant`, `AsMessage` on the discriminant, and `TransitiveChild` on both.
///     It also derives the following `std` traits on the discriminant: `Debug, Copy, Clone, PartialEq, Eq, Hash`.
///     **This third option will likely change in the future**
#[proc_macro_attribute]
pub fn impl_message(attr: TokenStream, input_item: TokenStream) -> TokenStream {
	TokenStream::from(combined_message_attrs_impl(attr.into(), input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

/// Derive the `Hint` trait
///
/// # Example
/// ```
/// # use graphite_proc_macros::Hint;
/// # use editor::utility_traits::Hint;
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
///     (ToolState::Ready, Event::PointerDown(mouse_state)) if *mouse_state == MouseState::Left => {
///         #[edge("LMB Down")]
///         ToolState::Pending
///     }
///     (SelectToolState::Pending, Event::PointerUp(mouse_state)) if *mouse_state == MouseState::Left => {
///         #[edge("LMB Up: Select Object")]
///         SelectToolState::Ready
///     }
///     (SelectToolState::Pending, Event::PointerMove(x,y)) => {
///         #[edge("Mouse Move")]
///         SelectToolState::TransformSelected
///     }
///     (SelectToolState::TransformSelected, Event::PointerMove(x,y)) => {
///         #[edge("Mouse Move")]
///         SelectToolState::TransformSelected
///     }
///     (SelectToolState::TransformSelected, Event::PointerUp(mouse_state)) if *mouse_state == MouseState::Left => {
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
	let _verify = syn::parse_macro_input!(attr as AttrInnerSingleString);

	item
}

#[proc_macro_derive(WidgetBuilder, attributes(widget_builder))]
pub fn derive_widget_builder(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_widget_builder_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
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
