use proc_macro2::Ident;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Paren;
use syn::{parenthesized, LitStr, Token};

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
	_paren_token: Paren,
	parts: Punctuated<KeyEqString, Token![,]>,
}

impl Parse for AttrInnerKeyStringMap {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let content;
		let _paren_token = parenthesized!(content in input);
		Ok(Self {
			_paren_token,
			parts: Punctuated::parse_terminated(&content)?,
		})
	}
}

impl AttrInnerKeyStringMap {
	pub fn into_hashmap(self) -> HashMap<Ident, Vec<LitStr>> {
		let mut res = HashMap::<_, Vec<_>>::new();

		for part in self.parts {
			res.entry(part.key).or_default().push(part.lit);
		}

		res
	}
}
