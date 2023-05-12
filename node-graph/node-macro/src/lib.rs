use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, ToTokens};
use syn::{
	parse_macro_input, punctuated::Punctuated, token::Comma, FnArg, GenericParam, Ident, ItemFn, Lifetime, Pat, PatIdent, PathArguments, PredicateType, ReturnType, Token, TraitBound, Type, TypeParam,
	TypeParamBound, WhereClause, WherePredicate,
};

#[proc_macro_attribute]
pub fn node_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
	//let node_name = parse_macro_input!(attr as Ident);
	let node = parse_macro_input!(attr as syn::PathSegment);

	let function = parse_macro_input!(item as ItemFn);

	let node = &node;
	let node_name = &node.ident;
	let mut args = match node.arguments.clone() {
		PathArguments::AngleBracketed(args) => args
			.args
			.into_iter()
			.map(|arg| match arg {
				syn::GenericArgument::Type(ty) => ty,
				_ => panic!("Only types are allowed as arguments"),
			})
			.collect::<Vec<_>>(),
		_ => Default::default(),
	};

	let asyncness = function.sig.asyncness.is_some();

	let arg_idents = args
		.iter()
		.filter(|x| x.to_token_stream().to_string().starts_with('_'))
		.map(|arg| Ident::new(arg.to_token_stream().to_string().to_lowercase().as_str(), Span::call_site()))
		.collect::<Vec<_>>();

	let mut function_inputs = function.sig.inputs.iter().filter_map(|arg| if let FnArg::Typed(typed_arg) = arg { Some(typed_arg) } else { None });

	let mut type_generics = function.sig.generics.params.clone();
	let mut where_clause = function.sig.generics.where_clause.clone().unwrap_or(WhereClause {
		where_token: Token![where](Span::call_site()),
		predicates: Default::default(),
	});

	// Extract primary input as first argument
	let primary_input = function_inputs.next().expect("Primary input required - set to `()` if not needed.");
	let Pat::Ident(PatIdent{ident: primary_input_ident, mutability: primary_input_mutability,..} ) =&*primary_input.pat else {
		panic!("Expected ident as primary input.");
	};
	let primary_input_ty = &primary_input.ty;
	let aux_type_generics = type_generics
		.iter()
		.filter(|gen| {
			if let GenericParam::Type(ty) = gen {
				!function.sig.inputs.iter().take(1).any(|param_ty| match param_ty {
					FnArg::Typed(pat_ty) => ty.ident == pat_ty.ty.to_token_stream().to_string(),
					_ => false,
				})
			} else {
				false
			}
		})
		.cloned()
		.collect::<Vec<_>>();

	let body = function.block;

	// Extract secondary inputs as all other arguments
	let parameter_inputs = function_inputs.collect::<Vec<_>>();
	let parameter_pat_ident_patterns = parameter_inputs
		.iter()
		.map(|input| {
			let Pat::Ident(pat_ident) = &*input.pat else { panic!("Expected ident for secondary input."); };
			pat_ident
		})
		.collect::<Vec<_>>();
	let parameter_idents = parameter_pat_ident_patterns.iter().map(|pat_ident| &pat_ident.ident).collect::<Vec<_>>();
	let parameter_mutability = parameter_pat_ident_patterns.iter().map(|pat_ident| &pat_ident.mutability);

	// Extract the output type of the entire node - `()` by default
	let output = if let ReturnType::Type(_, ty) = &function.sig.output {
		ty.to_token_stream()
	} else {
		quote::quote!(())
	};

	let struct_generics = (0..parameter_inputs.len())
		.map(|x| {
			let ident = format_ident!("S{x}");
			ident
		})
		.collect::<Punctuated<_, Comma>>();
	let struct_generics_iter = struct_generics.iter();

	for ident in struct_generics.iter() {
		args.push(Type::Verbatim(quote::quote!(#ident)));
	}

	// Generics are simply `S0` through to `Sn-1` where n is the number of secondary inputs
	let node_generics = struct_generics
		.iter()
		.cloned()
		.map(|ident| {
			GenericParam::Type(TypeParam {
				attrs: vec![],
				ident,
				colon_token: Some(Default::default()),
				bounds: Punctuated::from_iter([TypeParamBound::Lifetime(Lifetime::new("'input", Span::call_site()))].iter().cloned()),
				eq_token: None,
				default: None,
			})
		})
		.collect::<Punctuated<_, Comma>>();
	type_generics.iter_mut().for_each(|x| {
		if let GenericParam::Type(t) = x {
			t.bounds.insert(0, TypeParamBound::Lifetime(Lifetime::new("'input", Span::call_site())));
		}
	});
	let generics = type_generics.into_iter().chain(node_generics.iter().cloned()).collect::<Punctuated<_, Comma>>();
	let new_fn_generics = aux_type_generics.into_iter().chain(node_generics.iter().cloned()).collect::<Punctuated<_, Comma>>();
	// Bindings for all of the above generics to a node with an input of `()` and an output of the type in the function
	let extra_where_clause = parameter_inputs
		.iter()
		.zip(&node_generics)
		.map(|(ty, name)| {
			let ty = &ty.ty;
			let GenericParam::Type(generic_ty) = name else { panic!("Expected type generic."); };
			let ident = &generic_ty.ident;
			WherePredicate::Type(PredicateType {
				lifetimes: None,
				bounded_ty: Type::Verbatim(ident.to_token_stream()),
				colon_token: Default::default(),
				bounds: Punctuated::from_iter([TypeParamBound::Trait(TraitBound {
					paren_token: None,
					modifier: syn::TraitBoundModifier::None,
					lifetimes: syn::parse_quote!(for<'any_input>),
					path: syn::parse_quote!(Node<'any_input, (), Output = #ty>),
				})]),
			})
		})
		.collect::<Vec<_>>();
	where_clause.predicates.extend(extra_where_clause.clone());

	let input_lifetime = if generics.is_empty() { quote::quote!() } else { quote::quote!('input,) };

	let node_impl = if asyncness {
		quote::quote! {

			impl <'input, #generics> Node<'input, #primary_input_ty> for #node_name<#(#args),*>
				#where_clause
			{
				type Output = core::pin::Pin<Box<dyn core::future::Future< Output = #output> + 'input>>;
				#[inline]
				fn eval(&'input self, #primary_input_mutability #primary_input_ident: #primary_input_ty) -> Self::Output {
					#(
						let #parameter_mutability #parameter_idents = self.#parameter_idents.eval(());
					)*

					Box::pin(async move {#body})
				}
			}
		}
	} else {
		quote::quote! {

			impl <'input, #generics> Node<'input, #primary_input_ty> for #node_name<#(#args),*>
				#where_clause
			{
				type Output = #output;
				#[inline]
				fn eval(&'input self, #primary_input_mutability #primary_input_ident: #primary_input_ty) -> Self::Output {
					#(
						let #parameter_mutability #parameter_idents = self.#parameter_idents.eval(());
					)*

					#body
				}
			}
		}
	};

	let new_fn = quote::quote! {

		impl <#input_lifetime #new_fn_generics> #node_name<#(#args),*>
			where #(#extra_where_clause),*
		{
			pub const fn new(#(#parameter_idents: #struct_generics_iter),*) -> Self{
				Self{
					#(#parameter_idents,)*
					#(#arg_idents: core::marker::PhantomData,)*
				}
			}
		}

	};
	quote::quote! {
		#node_impl
		#new_fn
	}
	.into()
}
