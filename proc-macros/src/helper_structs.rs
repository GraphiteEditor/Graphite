use proc_macro2::{Ident, TokenStream};
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Paren;
use syn::{parenthesized, LitStr, Token};

pub struct IdentList {
	pub parts: Punctuated<Ident, Token![,]>,
}

impl Parse for IdentList {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let content;
		let _paren_token = parenthesized!(content in input);
		Ok(Self {
			parts: Punctuated::parse_terminated(&content)?,
		})
	}
}

/// Parses `("some text")`
pub struct AttrInnerSingleString {
	_paren_token: Paren,
	pub content: LitStr,
}

impl Parse for AttrInnerSingleString {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let content;
		let _paren_token = parenthesized!(content in input);
		Ok(Self {
			_paren_token,
			content: content.parse()?,
		})
	}
}

/// Parses `key="value"`
pub struct KeyEqString {
	key: Ident,
	_eq_token: Token![=],
	lit: LitStr,
}

impl Parse for KeyEqString {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			key: input.parse()?,
			_eq_token: input.parse()?,
			lit: input.parse()?,
		})
	}
}

/// Parses `(key="value", key="value", …)`
pub struct AttrInnerKeyStringMap {
	parts: Punctuated<KeyEqString, Token![,]>,
}

impl Parse for AttrInnerKeyStringMap {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			parts: Punctuated::parse_terminated(input)?,
		})
	}
}

impl AttrInnerKeyStringMap {
	pub fn multi_into_iter(iter: impl IntoIterator<Item = Self>) -> impl Iterator<Item = (Ident, Vec<LitStr>)> {
		use std::collections::hash_map::Entry;

		let mut res = Vec::<(Ident, Vec<LitStr>)>::new();
		let mut idx = HashMap::<Ident, usize>::new();

		for part in iter.into_iter().flat_map(|x: Self| x.parts) {
			match idx.entry(part.key) {
				Entry::Occupied(occ) => {
					res[*occ.get()].1.push(part.lit);
				}
				Entry::Vacant(vac) => {
					let ident = vac.key().clone();
					vac.insert(res.len());
					res.push((ident, vec![part.lit]));
				}
			}
		}

		res.into_iter()
	}
}

/// Parses `(left, right)`
pub struct Pair<F, S> {
	pub first: F,
	pub sep: Token![,],
	pub second: S,
}

impl<F, S> Parse for Pair<F, S>
where
	F: Parse,
	S: Parse,
{
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			first: input.parse()?,
			sep: input.parse()?,
			second: input.parse()?,
		})
	}
}

/// parses `(...)`
pub struct ParenthesizedTokens {
	pub paren: Paren,
	pub tokens: TokenStream,
}

impl Parse for ParenthesizedTokens {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let content;
		let paren = parenthesized!(content in input);
		Ok(Self { paren, tokens: content.parse()? })
	}
}

/// parses a comma-delimeted list of `T`s with optional trailing comma
pub struct SimpleCommaDelimeted<T>(pub Vec<T>);

impl<T: Parse> Parse for SimpleCommaDelimeted<T> {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let punctuated = Punctuated::<T, Token![,]>::parse_terminated(input)?;
		Ok(Self(punctuated.into_iter().collect()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn attr_inner_single_string() {
		let res = syn::parse2::<AttrInnerSingleString>(quote::quote! {
			("a string literal")
		});
		assert!(res.is_ok());
		assert_eq!(res.ok().unwrap().content.value(), "a string literal");

		let res = syn::parse2::<AttrInnerSingleString>(quote::quote! {
			wrong, "stuff"
		});
		assert!(res.is_err());
	}

	#[test]
	fn key_eq_string() {
		let res = syn::parse2::<KeyEqString>(quote::quote! {
			key="value"
		});
		assert!(res.is_ok());
		let res = res.ok().unwrap();
		assert_eq!(res.key, "key");
		assert_eq!(res.lit.value(), "value");

		let res = syn::parse2::<KeyEqString>(quote::quote! {
			wrong, "stuff"
		});
		assert!(res.is_err());
	}

	#[test]
	fn attr_inner_key_string_map() {
		let res = syn::parse2::<AttrInnerKeyStringMap>(quote::quote! {
			key="value", key2="value2"
		});
		assert!(res.is_ok());
		let res = res.ok().unwrap();
		for (item, (k, v)) in res.parts.into_iter().zip(vec![("key", "value"), ("key2", "value2")]) {
			assert_eq!(item.key, k);
			assert_eq!(item.lit.value(), v);
		}

		let res = syn::parse2::<AttrInnerKeyStringMap>(quote::quote! {
			key="value", key2="value2",
		});
		assert!(res.is_ok());
		let res = res.ok().unwrap();
		for (item, (k, v)) in res.parts.into_iter().zip(vec![("key", "value"), ("key2", "value2")]) {
			assert_eq!(item.key, k);
			assert_eq!(item.lit.value(), v);
		}

		let res = syn::parse2::<AttrInnerKeyStringMap>(quote::quote! {
			wrong, "stuff"
		});
		assert!(res.is_err());
	}
}
