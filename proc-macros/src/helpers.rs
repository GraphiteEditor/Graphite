use proc_macro2::{Ident, Span};
use syn::punctuated::Punctuated;
use syn::{Path, PathArguments, PathSegment, Token};

/// Returns `Ok(Vec<T>)` if all items are `Ok(T)`, else returns a combination of every error encountered (not just the first one)
pub fn fold_error_iter<T>(iter: impl Iterator<Item = syn::Result<T>>) -> syn::Result<Vec<T>> {
	iter.fold(Ok(vec![]), |acc, x| match acc {
		Ok(mut v) => x.map(|x| {
			v.push(x);
			v
		}),
		Err(mut e) => match x {
			Ok(_) => Err(e),
			Err(e2) => {
				e.combine(e2);
				Err(e)
			}
		},
	})
}

/// Creates an ident at the call site
pub fn call_site_ident<S: AsRef<str>>(s: S) -> Ident {
	Ident::new(s.as_ref(), Span::call_site())
}

/// Creates the path `left::right` from the identifiers `left` and `right`
pub fn two_segment_path(left_ident: Ident, right_ident: Ident) -> Path {
	let mut segments: Punctuated<PathSegment, Token![::]> = Punctuated::new();
	segments.push(PathSegment {
		ident: left_ident,
		arguments: PathArguments::None,
	});
	segments.push(PathSegment {
		ident: right_ident,
		arguments: PathArguments::None,
	});

	Path { leading_colon: None, segments }
}

#[cfg(test)]
mod tests {
	use super::*;

	use quote::ToTokens;
	use syn::spanned::Spanned;

	#[test]
	fn test_fold_error_iter() {
		let res = fold_error_iter(vec![Ok(()), Ok(())].into_iter());
		assert!(res.is_ok());

		let _span = quote::quote! { "" }.span();
		let res = fold_error_iter(vec![Ok(()), Err(syn::Error::new(_span, "err1")), Err(syn::Error::new(_span, "err2"))].into_iter());
		assert!(res.is_err());
		let err = res.unwrap_err();
		let mut check_err = syn::Error::new(_span, "err1");
		check_err.combine(syn::Error::new(_span, "err2"));
		assert_eq!(err.to_compile_error().to_string(), check_err.to_compile_error().to_string());
	}

	#[test]
	fn test_two_path() {
		let _span = quote::quote! { "" }.span();
		assert_eq!(two_segment_path(Ident::new("a", _span), Ident::new("b", _span)).to_token_stream().to_string(), "a :: b");
	}
}
