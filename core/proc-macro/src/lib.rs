mod helpers;
mod structs;

use std::fmt::Display;

use crate::helpers::{fold_error_iter, to_path};
use crate::structs::{AttrInnerKeyStringMap, AttrInnerSingleString};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::ToTokens;
use structs::IdentList;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Ident, LitStr, Variant};

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

fn parse_message_helper_attrs(attrs: &[Attribute]) -> syn::Result<(Ident, Ident, Ident)> {
	attrs
		.iter()
		.filter(|a| a.path.get_ident().map_or(false, |i| i == "message"))
		.map(|attr| syn::parse2::<IdentList>(attr.tokens.clone()))
		.next()
		.expect("error: #[message… attr not found")
		.and_then(|v: IdentList| match v.parts.len() {
			3 => {
				let mut parts = v.parts.iter();
				Ok((parts.next().cloned().unwrap(), parts.next().cloned().unwrap(), parts.next().cloned().unwrap()))
			}
			_ => panic!("error: #[message… takes 3 arguments, …"),
		})
}

fn derive_hint_impl(input_item: TokenStream2) -> syn::Result<TokenStream2> {
	let input = syn::parse2::<DeriveInput>(input_item)?;

	let ident = input.ident;

	match input.data {
		Data::Enum(data) => {
			let variants = data.variants.iter().map(|var: &Variant| to_path(ident.clone(), var.ident.clone())).collect::<Vec<_>>();

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

#[proc_macro_derive(MessageImpl, attributes(message, child))]
pub fn derive_message(input_item: TokenStream) -> TokenStream {
	TokenStream::from(derive_message_impl(input_item.into()))
	//TokenStream::from(derive_message_impl(input_item.into()).unwrap_or_else(|err| err.to_compile_error()))
}

fn derive_message_impl(input_item: TokenStream2) -> TokenStream2 {
	let input = syn::parse2::<DeriveInput>(input_item).unwrap();

	let ident = input.ident;
	let (super_parent, parent, parent_variant) = parse_message_helper_attrs(input.attrs.as_slice()).unwrap();
	let parent_path = to_path(parent.clone(), parent_variant.clone());
	let discriminant = Ident::new(format!("{}Discriminant", ident).as_str(), Span::call_site());
	let super_discriminant = Ident::new(format!("{}Discriminant", super_parent).as_str(), Span::call_site());
	let parent_discriminant = Ident::new(format!("{}Discriminant", parent).as_str(), Span::call_site());
	let parent_discriminant_path = to_path(parent_discriminant.clone(), parent_variant.clone());

	if let Data::Enum(data) = input.data {
		let variants = data.variants.iter().map(|var: &Variant| to_path(ident.clone(), var.ident.clone())).collect::<Vec<_>>();
		let variant_fields = data.variants.iter().map(|var: &Variant| var.fields.clone()).collect::<Vec<_>>();
		let data_variant_fields: Vec<TokenStream2> = data
			.variants
			.iter()
			.zip(variant_fields.iter())
			.map(|(var, field)| {
				if let Some(syn::Field { ty: syn::Type::Path(path), .. }) = field.iter().next() {
					if var.attrs.iter().any(|name| name.path.to_token_stream().to_string().as_str() == "child") {
						let last = path.path.segments.last().unwrap();
						let new_ident = Ident::new(format!("{}Discriminant", last.ident).as_str(), Span::call_site());
						quote::quote! {
							(#new_ident)
						}
					} else {
						quote::quote! {}
					}
				} else {
					quote::quote! {}
				}
			})
			.collect();
		let convert_variant_fields: Vec<TokenStream2> = data
			.variants
			.iter()
			.zip(variant_fields.iter())
			.map(|(var, field)| {
				let var_path = to_path(ident.clone(), var.ident.clone());
				let dis_path = to_path(discriminant.clone(), var.ident.clone());
				if field.iter().next().is_some() {
					quote::quote! {
						#var_path(x) => #dis_path(x.clone().into()),
					}
				} else {
					quote::quote! {
						#var_path => #dis_path,
					}
				}
			})
			.collect();
		let data_variants: Vec<Ident> = data.variants.iter().map(|v| v.ident.clone()).collect();

		let into_impl = |from, to, path| {
			quote::quote! {
				#[allow(clippy::from_over_into)]
				impl Into<#to> for #from {
					fn into(self) -> #to {
						#path(self)
					}
				}
				#[allow(clippy::from_over_into)]
				impl Into<#to> for &#from {
					fn into(self) -> #to {
						#path(self.clone())
					}
				}
			}
		};
		let into_super_impl = |from, to, path| {
			quote::quote! {
				#[allow(clippy::from_over_into)]
				impl Into<#to> for #from {
					fn into(self) -> #to {
						#path(self).into()
					}
				}
				#[allow(clippy::from_over_into)]
				impl Into<#to> for &#from {
					fn into(self) -> #to {
						#path(self.clone()).into()
					}
				}
			}
		};
		let super_impl = into_super_impl(ident.clone(), super_parent.clone(), parent_path.clone());
		let discriminant_super_impl = into_super_impl(discriminant.clone(), super_discriminant.clone(), parent_discriminant_path.clone());

		let super_impl = if parent == super_parent {
			TokenStream2::new()
		} else {
			quote::quote! {
				#super_impl
				#discriminant_super_impl
				#[allow(clippy::from_over_into)]
				impl Into<#super_discriminant> for &#ident {
					fn into(self) -> #super_discriminant {
						let dis: #discriminant = self.into();
						dis.into()
					}
				}
			}
		};
		let prefix_impl = if ident == super_parent {
			quote::quote! {
				format!("")
			}
		} else {
			quote::quote! {
				format!("{}.{}", #parent::prefix(), stringify!(#parent_variant))
			}
		};
		let into_parent = into_impl(ident.clone(), parent.clone(), parent_path);
		let into_parent_discriminant = into_impl(discriminant.clone(), parent_discriminant.clone(), parent_discriminant_path.clone());
		let super_discriminant_impl = if ident == super_parent {
			quote::quote! {
				impl From<&#ident> for #ident {
					fn from(ident: &#ident) -> Self {
						ident.clone()
					}
				}

			}
		} else {
			quote::quote! {
				#[allow(clippy::from_over_into)]
				impl Into<#parent_discriminant> for &#ident {
					fn into(self) -> #parent_discriminant {
						#parent_discriminant_path(self.into())
					}
				}
				impl PartialEq<#super_parent> for #ident {
					fn eq(&self, other: &#super_parent) -> bool {
						let message: #super_parent =  self.into();
						message == *other
					}
				}
				#into_parent
				#into_parent_discriminant
			}
		};

		let res = quote::quote! {
			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub enum #discriminant {
				#(
					#data_variants #data_variant_fields ,
				)*
			}

			impl AsMessage for #ident {
				fn suffix(&self) -> &'static str {
					match *self {
						#(
							#variants { .. } => {
								stringify!(#data_variants)
							}
						)*
					}
				}
				fn prefix() -> String {
					#prefix_impl
				}
				fn name(&self) -> String {
					format!("{}.{}", Self::prefix(), self.suffix())
				}
				fn get_discriminant(&self) -> #super_discriminant {
					let dis: #discriminant = self.into();
					dis.into()
				}

			}
			#super_impl
			#super_discriminant_impl


			impl From<#ident> for #discriminant {
				fn from(ident: #ident) -> Self {
					match ident {
						#( 	#convert_variant_fields		)*
					}
				}
			}
			impl From<&#ident> for #discriminant {
				fn from(ident: &#ident) -> Self {
					match ident.clone() {
						#( 	#convert_variant_fields		)*
					}
				}
			}
			impl std::fmt::Display for #ident {
				fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
					let message: #super_parent =  self.into();
					write!(f, "{}", message)
				}

			}
		};
		res
	} else {
		panic!("Tried to use derive macro on non enum")
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
