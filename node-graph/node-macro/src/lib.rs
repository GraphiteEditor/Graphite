use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote, ToTokens};
use syn::{
	parse_macro_input, punctuated::Punctuated, token::Comma, AngleBracketedGenericArguments, AssocType, FnArg, GenericArgument, GenericParam, Ident, ItemFn, Lifetime, Pat, PatIdent, PathArguments,
	PredicateType, ReturnType, Token, TraitBound, Type, TypeImplTrait, TypeParam, TypeParamBound, TypeTuple, WhereClause, WherePredicate,
};

/// A macro used to construct a proto node implementation from the given struct and the decorated function.
///
/// This works by generating two `impl` blocks for the given struct:
///
/// - `impl TheGivenStruct`:
///   Attaches a `new` constructor method to the struct.
/// - `impl Node for TheGivenStruct`:
///   Implements the [`Node`] trait for the struct, with the `eval` method inside which is a modified version of the decorated function. See below for how the function is modified.
///
/// # Usage of this and similar macros
///
/// You'll use this macro most commonly when writing proto nodes. It's a convenient combination of the [`node_new`] and [`node_impl`] proc macros, which handles both of the bullet points above, respectively. There can only be one constructor method, but additional functions decorated by the [`node_impl`] macro can be added to implement different functionality across multiple type signatures.
///
/// # Useful hint
///
/// It can be helpful to run the "rust-analyzer: Expand macro recursively at carat" command from the VS Code command palette (or your editor's equivalent) to see the generated code of the macro to understand how the translation magic works.
///
/// # How generics and type signatures are handled
///
/// The given struct has various fields, each of them generic. These correspond with the node's parameters (the secondary inputs, but not the primary input). We can implement multiple functions with different type signatures, each each of these are converted by the [`node_impl`] macro into separate `impl` blocks for different `Node` traits.
///
/// ## Type signature translation
///
/// The conversion into an `impl Node` corresponding with the decorated function's type signature involves:
///
/// - Mapping the type of the function's first argument (the node's primary input) to the impl'd `Node`'s generic type, e.g.:
///   
///   ```
///   Node<'input, Color>
///   ```
///   
///   for a `Color` primary input type.
/// - Mapping the type of the function's remaining arguments (the node's secondary inputs) to the given struct fields' generic types, e.g.:
///   
///   ```
///   TheGivenStruct<S0, S1>
///       where S0: Node<'input, (), Output = f64>,
///       where S1: Node<'input, (), Output = f64>,
///   ```
///   
///   for two `f64` parameter (secondary input) types. Since Graphene works by having each function evaluate its upstream node as a lambda that returns output data, these secondary inputs are not directly `f64` values but rather `Node`s that output `f64` values when evaluated (in this case, with an empty input of `()`).
/// - Mapping the function's return type to the impl'd `Node` trait's associated type, e.g.:
///   
///   ```
///   Output = Color
///   ```
///   
///   for a `Color` return (secondary output) type.
///
/// ## `eval()` method generation
///
/// The conversion of the decorated function's body into the `eval` method within the `impl Node` block involves the following steps:
///
/// - The function's body gets copied over to the interior of the `eval` method.
/// - The function's argument list only has its first argument (the node's primary input) copied over to the `eval` function signature. The remaining arguments (the node's secondary inputs) are not copied over as `eval` function arguments.
/// - A series of `let` declarations are added before the copied-over function body, one for each secondary input. They look like `let secondaryA: SomeOutputType = self.secondaryA.eval(someInput);`. Each one is calling the `eval()` method on its corresponding struct field, obtaining the evaluated value of that secondary input node that gets used in the function body in the lines below these `let` declarations.
///   
///   This process is necessary because the arguments in the original decorated function don't really exist with the actual values. Instead, they live as fields in the struct and they are `Node`s that output the actual values only once evaluated. So with the magic performed by this macro, the function body can written pretending to be working with the actual secondary input values, but the real types are `impl Node<SomeInputType, Output = SomeOutputType>` and they live in `self` as struct fields.
///   
///   The function body runs with the actual primary input value from the `eval` method's argument and the secondary input values from the `eval` method's `let` declarations. The result looks like this:
///   
///   ```
///   fn eval(&'input self, color: Color) -> Self::Output {
///       let secondaryA = self.secondaryA.eval(());
///       let secondaryB = self.secondaryB.eval(());
///       {
///           Color::from_rgbaf32_unchecked(
///               color.r() / secondaryA,
///               color.g() / secondaryA,
///               color.b() / secondaryA,
///               color.a() * secondaryB,
///           )
///       }
///   }
///   ```
///   
///   There is one exception where a `let` declaration is not added if an opt-out is desired. Any argument given to the decorated function may be of type `impl Node<SomeInputType, Output = SomeOutputType>` which will tell the macro not to add a `let` declaration for that argument. This allows for manually calling `eval` on the struct field in the function body, like `self.secondaryA.eval(())`.
///   
///   When a `let` declaration is generated automatically, this is called **automatic composition**. When opting out, this is called **manual composition**.
#[proc_macro_attribute]
pub fn node_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
	// Performs the `node_impl` macro's functionality of attaching an `impl Node for TheGivenStruct` block to the node struct
	let node_impl = node_impl_proxy(attr.clone(), item.clone());

	// Performs the `node_new` macro's functionality of attaching a `new` constructor method to the node struct
	let mut new_constructor = node_new_impl(attr, item);

	// Combines the two pieces of Rust source code
	new_constructor.extend(node_impl);

	new_constructor
}

/// Attaches an `impl TheGivenStruct` block to the node struct, containing a `new` constructor method. This is almost always called by the combined [`node_fn`] macro instead of using this one, however it can be used separately if needed. See that macro's documentation for more information.
#[proc_macro_attribute]
pub fn node_new(attr: TokenStream, item: TokenStream) -> TokenStream {
	node_new_impl(attr, item)
}

/// Attaches an `impl Node for TheGivenStruct` block to the node struct, containing an implementation of the node's `eval` method for a certain type signature. This can be called with multiple separate functions each having different type signatures. The [`node_fn`] macro calls this macro as well as defining a `new` constructor method on the node struct, which is a necessary part of defining a proto node; therefore you will most likely call that macro on the first decorated function and this macro on any additional decorated functions to provide additional type signatures for the proto node. See that macro's documentation for more information.
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
				Self {
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
	// let node_name = parse_macro_input!(attr as Ident);
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
		#body
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
					lifetimes: None, // syn::parse_quote!(for<'any_input>),
					path: syn::parse_quote!(#bound),
				})]),
			})
		})
		.collect()
}
