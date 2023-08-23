use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use syn::{
	parse_macro_input, punctuated::Punctuated, token::Comma, AngleBracketedGenericArguments, AssocType, FnArg, GenericArgument, GenericParam, Ident, ItemFn, Lifetime, Pat, PatIdent, PathArguments,
	PredicateType, ReturnType, Token, TraitBound, Type, TypeImplTrait, TypeParam, TypeParamBound, TypeTuple, WhereClause, WherePredicate,
};

#[proc_macro_attribute]
pub fn node_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
	let mut imp = node_impl_proxy(attr.clone(), item.clone());
	let new = node_new_impl(attr, item);
	imp.extend(new);
	imp
}
#[proc_macro_attribute]
pub fn node_new(attr: TokenStream, item: TokenStream) -> TokenStream {
	node_new_impl(attr, item)
}

#[proc_macro_attribute]
pub fn node_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
	node_impl_proxy(attr, item)
}

fn node_new_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
	let node = parse_macro_input!(attr as syn::PathSegment);

	let function = parse_macro_input!(item as ItemFn);

	let node = &node;
	let node_name = &node.ident;
	let mut args = node_args(node);

	let arg_idents = args
		.iter()
		.filter(|x| x.to_token_stream().to_string().starts_with('_'))
		.map(|arg| Ident::new(arg.to_token_stream().to_string().to_lowercase().as_str(), Span::call_site()))
		.collect::<Vec<_>>();

	let (_, _, parameter_pat_ident_patterns) = parse_inputs(&function, false);
	let parameter_idents = parameter_pat_ident_patterns.iter().map(|pat_ident| &pat_ident.ident).collect::<Vec<_>>();

	// Extract the output type of the entire node - `()` by default
	let struct_generics = (0..parameter_pat_ident_patterns.len())
		.map(|x| {
			let ident = format_ident!("S{x}");
			ident
		})
		.collect::<Punctuated<_, Comma>>();

	for ident in struct_generics.iter() {
		args.push(Type::Verbatim(quote::quote!(#ident)));
	}

	let struct_generics_iter = struct_generics.iter();
	quote::quote! {
		#[automatically_derived]
		impl <#(#args),*> #node_name<#(#args),*>
		{
			pub const fn new(#(#parameter_idents: #struct_generics_iter),*) -> Self{
				Self{
					#(#parameter_idents,)*
					#(#arg_idents: core::marker::PhantomData,)*
				}
			}
		}
	}
	.into()
}

fn node_args(node: &syn::PathSegment) -> Vec<Type> {
	match node.arguments.clone() {
		PathArguments::AngleBracketed(args) => args
			.args
			.into_iter()
			.map(|arg| match arg {
				syn::GenericArgument::Type(ty) => ty,
				_ => panic!("Only types are allowed as arguments"),
			})
			.collect::<Vec<_>>(),
		_ => Default::default(),
	}
}

fn node_impl_proxy(attr: TokenStream, item: TokenStream) -> TokenStream {
	let fn_item = item.clone();
	let function = parse_macro_input!(fn_item as ItemFn);
	if function.sig.asyncness.is_some() {
		node_impl_impl(attr, item, Asyncness::AllAsync)
	} else {
		node_impl_impl(attr, item, Asyncness::Sync)
	}
}
enum Asyncness {
	Sync,
	AllAsync,
}

fn node_impl_impl(attr: TokenStream, item: TokenStream, asyncness: Asyncness) -> TokenStream {
	//let node_name = parse_macro_input!(attr as Ident);
	let node = parse_macro_input!(attr as syn::PathSegment);

	let function = parse_macro_input!(item as ItemFn);

	let node = &node;
	let node_name = &node.ident;
	let mut args = node_args(node);

	let async_out = match asyncness {
		Asyncness::Sync => false,
		Asyncness::AllAsync => true,
	};
	let async_in = matches!(asyncness, Asyncness::AllAsync);

	let body = &function.block;
	let mut type_generics = function.sig.generics.params.clone();
	let mut where_clause = function.sig.generics.where_clause.clone().unwrap_or(WhereClause {
		where_token: Token![where](Span::call_site()),
		predicates: Default::default(),
	});

	type_generics.iter_mut().for_each(|x| {
		if let GenericParam::Type(t) = x {
			t.bounds.insert(0, TypeParamBound::Lifetime(Lifetime::new("'input", Span::call_site())));
		}
	});

	let (primary_input, parameter_inputs, parameter_pat_ident_patterns) = parse_inputs(&function, true);
	let primary_input_ty = &primary_input.ty;
	let Pat::Ident(PatIdent {
		ident: primary_input_ident,
		mutability: primary_input_mutability,
		..
	}) = &*primary_input.pat
	else {
		panic!("Expected ident as primary input.");
	};

	// Extract the output type of the entire node - `()` by default
	let output = if let ReturnType::Type(_, ty) = &function.sig.output {
		ty.to_token_stream()
	} else {
		quote::quote!(())
	};

	let num_inputs = parameter_inputs.len();
	let struct_generics = (0..num_inputs).map(|x| format_ident!("S{x}")).collect::<Vec<_>>();
	let future_generics = (0..num_inputs).map(|x| format_ident!("F{x}")).collect::<Vec<_>>();
	let parameter_types = parameter_inputs.iter().map(|x| *x.ty.clone()).collect::<Vec<Type>>();
	let future_types = future_generics
		.iter()
		.enumerate()
		.map(|(i, x)| match parameter_types[i].clone() {
			Type::ImplTrait(x) => Type::ImplTrait(x),
			_ => Type::Verbatim(x.to_token_stream()),
		})
		.collect::<Vec<_>>();

	for ident in struct_generics.iter() {
		args.push(Type::Verbatim(quote::quote!(#ident)));
	}

	// Generics are simply `S0` through to `Sn-1` where n is the number of secondary inputs
	let node_generics = construct_node_generics(&struct_generics);
	let future_generic_params = construct_node_generics(&future_generics);
	let (future_parameter_types, future_generic_params): (Vec<_>, Vec<_>) = parameter_types.iter().cloned().zip(future_generic_params).filter(|(ty, _)| !matches!(ty, Type::ImplTrait(_))).unzip();

	let generics = if async_in {
		type_generics
			.into_iter()
			.chain(node_generics.iter().cloned())
			.chain(future_generic_params.iter().cloned())
			.collect::<Punctuated<_, Comma>>()
	} else {
		type_generics.into_iter().chain(node_generics.iter().cloned()).collect::<Punctuated<_, Comma>>()
	};

	// Bindings for all of the above generics to a node with an input of `()` and an output of the type in the function
	let node_bounds = if async_in {
		let mut node_bounds = input_node_bounds(future_types, node_generics, |lifetime, in_ty, out_ty| quote! {Node<#lifetime, #in_ty, Output = #out_ty>});
		let future_bounds = input_node_bounds(future_parameter_types, future_generic_params, |_, _, out_ty| quote! { core::future::Future<Output = #out_ty>});
		node_bounds.extend(future_bounds);
		node_bounds
	} else {
		input_node_bounds(parameter_types, node_generics, |lifetime, in_ty, out_ty| quote! {Node<#lifetime, #in_ty, Output = #out_ty>})
	};
	where_clause.predicates.extend(node_bounds);

	let output = if async_out {
		quote::quote!(core::pin::Pin<Box<dyn core::future::Future< Output = #output> + 'input>>)
	} else {
		quote::quote!(#output)
	};

	let parameter_idents = parameter_pat_ident_patterns.iter().map(|pat_ident| &pat_ident.ident).collect::<Vec<_>>();
	let parameter_mutability = parameter_pat_ident_patterns.iter().map(|pat_ident| &pat_ident.mutability);

	let parameters = if matches!(asyncness, Asyncness::AllAsync) {
		quote::quote!(#(let #parameter_mutability #parameter_idents = self.#parameter_idents.eval(()).await;)*)
	} else {
		quote::quote!(#(let #parameter_mutability #parameter_idents = self.#parameter_idents.eval(());)*)
	};
	let mut body_with_inputs = quote::quote!(
			#parameters
			{#body}
	);
	if async_out {
		body_with_inputs = quote::quote!(Box::pin(async move { #body_with_inputs }));
	}

	quote::quote! {
		#[automatically_derived]
		impl <'input, #generics> Node<'input, #primary_input_ty> for #node_name<#(#args),*>
			#where_clause
		{
			type Output = #output;
			#[inline]
			fn eval(&'input self, #primary_input_mutability #primary_input_ident: #primary_input_ty) -> Self::Output {
				#body_with_inputs
			}
		}
	}
	.into()
}

fn parse_inputs(function: &ItemFn, remove_impl_node: bool) -> (&syn::PatType, Vec<&syn::PatType>, Vec<&PatIdent>) {
	let mut function_inputs = function.sig.inputs.iter().filter_map(|arg| if let FnArg::Typed(typed_arg) = arg { Some(typed_arg) } else { None });

	// Extract primary input as first argument
	let primary_input = function_inputs.next().expect("Primary input required - set to `()` if not needed.");

	// Extract secondary inputs as all other arguments
	let parameter_inputs = function_inputs.collect::<Vec<_>>();

	let parameter_pat_ident_patterns = parameter_inputs
		.iter()
		.filter(|input| !matches!(&*input.ty, Type::ImplTrait(_)) || !remove_impl_node)
		.map(|input| {
			let Pat::Ident(pat_ident) = &*input.pat else {
				panic!("Expected ident for secondary input.");
			};
			pat_ident
		})
		.collect::<Vec<_>>();
	(primary_input, parameter_inputs, parameter_pat_ident_patterns)
}

fn construct_node_generics(struct_generics: &[Ident]) -> Vec<GenericParam> {
	struct_generics
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
		.collect()
}

fn input_node_bounds(parameter_inputs: Vec<Type>, node_generics: Vec<GenericParam>, trait_bound: impl Fn(Lifetime, Type, Type) -> proc_macro2::TokenStream) -> Vec<WherePredicate> {
	parameter_inputs
		.iter()
		.zip(&node_generics)
		.map(|(ty, name)| {
			let GenericParam::Type(generic_ty) = name else {
				panic!("Expected type generic.");
			};
			let ident = &generic_ty.ident;
			let (lifetime, in_ty, out_ty) = match ty.clone() {
				Type::ImplTrait(TypeImplTrait { bounds, .. }) if bounds.len() == 1 => {
					let TypeParamBound::Trait(TraitBound { ref path, .. }) = bounds[0] else {
						panic!("impl Traits other then Node are not supported")
					};
					let node_segment = path.segments.last().expect("Found an empty path in the impl Trait arg");
					assert_eq!(node_segment.ident.to_string(), "Node", "Only impl Node is supported as an argument");
					let PathArguments::AngleBracketed(AngleBracketedGenericArguments { ref args, .. }) = node_segment.arguments else {
						panic!("Node must have generic arguments")
					};
					let mut args_iter = args.iter();
					let lifetime = if args.len() == 2 {
						Lifetime::new("'input", Span::call_site())
					} else if let Some(GenericArgument::Lifetime(node_lifetime)) = args_iter.next() {
						node_lifetime.clone()
					} else {
						panic!("Invalid arguments for Node trait")
					};

					let Some(GenericArgument::Type(in_ty)) = args_iter.next() else {
						panic!("Expected type argument in Node<> declaration")
					};
					let Some(GenericArgument::AssocType(AssocType { ty: out_ty, .. })) = args_iter.next() else {
						panic!("Expected Output = in Node declaration")
					};
					(lifetime, in_ty.clone(), out_ty.clone())
				}
				ty => (
					Lifetime::new("'input", Span::call_site()),
					Type::Tuple(TypeTuple {
						paren_token: syn::token::Paren(Span::call_site()),
						elems: Punctuated::new(),
					}),
					ty,
				),
			};

			let bound = trait_bound(lifetime, in_ty, out_ty);
			WherePredicate::Type(PredicateType {
				lifetimes: None,
				bounded_ty: Type::Verbatim(ident.to_token_stream()),
				colon_token: Default::default(),
				bounds: Punctuated::from_iter([TypeParamBound::Trait(TraitBound {
					paren_token: None,
					modifier: syn::TraitBoundModifier::None,
					lifetimes: None, //syn::parse_quote!(for<'any_input>),
					path: syn::parse_quote!(#bound),
				})]),
			})
		})
		.collect()
}
