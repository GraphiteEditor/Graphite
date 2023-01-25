use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, ToTokens};
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, FnArg, GenericParam, Ident, ItemFn, Lifetime, Pat, PatIdent, PathArguments, ReturnType, Type, TypeParam, TypeParamBound};

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
	let arg_idents = args
		.iter()
		.map(|arg| Ident::new(format!("_{}", arg.to_token_stream().to_string().to_lowercase()).as_str(), Span::call_site()))
		.collect::<Vec<_>>();

	let mut function_inputs = function.sig.inputs.iter().filter_map(|arg| if let FnArg::Typed(typed_arg) = arg { Some(typed_arg) } else { None });

	let mut type_generics = function.sig.generics.params.clone();
	let where_clause = function.sig.generics.where_clause.clone();

	// Extract primary input as first argument
	let primary_input = function_inputs.next().expect("Primary input required - set to `()` if not needed.");
	let Pat::Ident(PatIdent{..} ) =&*primary_input.pat else {
		panic!("Expected ident as primary input.");
	};
	let primary_input_ty = &primary_input.ty;

	let body = function.block;

	// Extract secondary inputs as all other arguments
	let parameter_inputs = function_inputs.collect::<Vec<_>>();
	let parameter_idents = parameter_inputs
		.iter()
		.map(|input| {
			let Pat::Ident(PatIdent { ident: primary_input_ident,.. }) = &*input.pat else { panic!("Expected ident for secondary input."); };
			primary_input_ident
		})
		.collect::<Vec<_>>();

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
				bounds: Punctuated::from_iter([TypeParamBound::Lifetime(Lifetime::new("'node", Span::call_site()))].iter().cloned()),
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
	// Bindings for all of the above generics to a node with an input of `()` and an output of the type in the function
	let extra_where_clause = parameter_inputs
		.iter()
		.zip(&node_generics)
		.map(|(ty, name)| {
			let ty = &ty.ty;
			quote::quote!(#name: Node<(), Output = #ty>)
		})
		.collect::<Vec<_>>();

	let output = quote::quote! {
		impl <'input, 'node: 'input, #generics> NodeIO<'input, #primary_input_ty> for #node_name<#(#args),*>
			#where_clause
			#(#extra_where_clause),*
		{
			type Output = #output;
		}

		impl <'input, 'node: 'input, #generics> Node<'input, 'node, #primary_input_ty> for #node_name<#(#args),*>
			#where_clause
			#(#extra_where_clause),*
		{
			fn eval(&'node self, input: #primary_input_ty) -> <Self as NodeIO<'input, #primary_input_ty>>::Output {
				#(
					let #parameter_idents = self.#parameter_idents.eval(());
				)*

				#body
			}
		}

		impl <'input, 'node: 'input, #generics> #node_name<#(#args),*>
			#where_clause
			#(#extra_where_clause),*
		{
				pub fn new(#(#parameter_idents: #generics),*) -> Self{
					Self{
						#(#parameter_idents),*
						#(#arg_idents: core::marker::PhantomData),*
					}
				}
		}

	};
	println!("Node generated: {}", output.to_token_stream().to_string());
	output.into()
}
