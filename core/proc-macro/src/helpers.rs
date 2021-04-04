use proc_macro2::Ident;
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

/// Creates the path `left::right` from the idents `left` and `right`
pub fn two_path(left_ident: Ident, right_ident: Ident) -> Path {
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
