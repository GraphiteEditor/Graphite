use crate::crate_ident::CrateIdent;
use crate::parsing::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::visit::Visit;
use syn::{GenericArgument, GenericParam, Ident, Lifetime, PathArguments, Type, TypeParam, TypeParamBound};

pub(crate) struct GNodeTokens {
	pub(crate) in_mod: TokenStream2,
	pub(crate) top_level: TokenStream2,
}

pub(crate) fn generate_gnode_code(crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<GNodeTokens> {
	let core_types = crate_ident.gcore()?;

	let ctx_param = context_param(parsed);
	let ctx_ident = match ctx_param {
		Some(ctx_param) => ctx_param.ident.clone(),
		None => format_ident!("__Ctx"),
	};
	let async_fn = parsed.is_async;
	let future_kernel = is_source_kernel(&parsed.output_type);
	let async_source = async_fn || future_kernel;
	if async_fn && parsed.fields.iter().any(|field| matches!(field.ty, ParsedFieldType::Node(_))) {
		return Ok(GNodeTokens {
			in_mod: quote!(),
			top_level: quote!(),
		});
	}
	let snapshot_ctx = async_fn && matches!(&parsed.input.ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "CtxSnapshot"));

	let mut ctx_bounds: Vec<TokenStream2> = match ctx_param {
		Some(ctx_param) => ctx_param
			.bounds
			.iter()
			.filter_map(|bound| match bound {
				TypeParamBound::Lifetime(_) => None,
				bound => Some(desugar_extract_lifetime(bound, core_types)),
			})
			.collect(),
		None => vec![quote!(#core_types::Ctx)],
	};
	if async_source {
		ctx_bounds.push(quote!(#core_types::CacheHash));
	}
	if snapshot_ctx {
		ctx_bounds.extend([
			quote!(#core_types::context::DeriveCtx),
			quote!(#core_types::context::ExtractFootprint),
			quote!(#core_types::context::ExtractRealTime),
			quote!(#core_types::context::ExtractAnimationTime),
			quote!(#core_types::context::ExtractPointerPosition),
			quote!(#core_types::context::ExtractIndex),
			quote!(#core_types::context::ExtractPosition),
		]);
	}

	let derives = ctx_param.is_some_and(|ctx_param| {
		ctx_param.bounds.iter().any(|bound| match bound {
			TypeParamBound::Trait(trait_bound) => trait_bound.path.segments.last().is_some_and(|segment| segment.ident == "DeriveCtx"),
			_ => false,
		})
	});

	let ctx_generic = match ctx_bounds.is_empty() {
		true => quote!(#ctx_ident),
		false => quote!(#ctx_ident: #(#ctx_bounds)+*),
	};
	let mut generics: Vec<TokenStream2> = parsed
		.fn_generics
		.iter()
		.map(|param| match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) == ctx_param.map(|ctx_param| &ctx_param.ident) => ctx_generic.clone(),
			param => quote!(#param),
		})
		.collect();
	if ctx_param.is_none() {
		generics.push(ctx_generic);
	}

	let fn_name = &parsed.fn_name;
	let mod_name = format_ident!("_{}_mod", parsed.mod_name);
	let struct_name = format_ident!("{}Node", parsed.struct_name);
	let output_type = &parsed.output_type;
	let trait_output = slot_value_type(&parsed.output_type);
	let raw_lazy = matches!(kernel_kind(&parsed.output_type), KernelKind::Poll(_));
	let injected_name = |ident: &Ident| async_source && (ident == "_runtime" || ident == "_source");
	let where_predicates: Vec<TokenStream2> = parsed.where_clause.iter().flat_map(|clause| clause.predicates.iter()).map(|predicate| quote!(#predicate)).collect();

	let (data_fields, regular_fields): (Vec<_>, Vec<_>) = parsed.fields.iter().partition(|field| field.is_data_field);

	let data_field_generic_idents: Vec<Ident> = parsed
		.fn_generics
		.iter()
		.filter_map(|generic| match generic {
			GenericParam::Type(type_param) => Some(type_param.ident.clone()),
			_ => None,
		})
		.filter(|ident| {
			data_fields.iter().any(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => crate::codegen::type_contains_ident(ty, ident),
				_ => false,
			})
		})
		.collect();

	let node_generics: Vec<Ident> = regular_fields.iter().enumerate().map(|(index, _)| format_ident!("Node{}", index)).collect();
	let struct_type_params: Vec<Ident> = data_field_generic_idents.iter().cloned().chain(node_generics.iter().cloned()).collect();

	let data_names: Vec<&Ident> = data_fields.iter().map(|field| &field.pat_ident.ident).collect();
	let data_params = data_fields.iter().map(|field| {
		let pat = &field.pat_ident;
		let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
			unreachable!("data fields are regular types");
		};
		quote!(#pat: &#ty)
	});

	let lazy_bound = |output_type: &Type| match derives {
		true => quote!(for<'__derived> #core_types::gnode::GNode<#core_types::context::Derived<'__derived, #ctx_ident>, Output = #output_type>),
		false => quote!(#core_types::gnode::GNode<#ctx_ident, Output = #output_type>),
	};

	let kernel_params = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).map(|field| {
		let pat = &field.pat_ident;
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#pat: #ty),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) if raw_lazy => {
				let bound = lazy_bound(output_type);
				quote!(#pat: &impl #bound)
			}
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
				let bound = lazy_bound(output_type);
				quote!(#pat: #core_types::gnode::LazyInput<'_, impl #bound>)
			}
		}
	});

	let node_bounds = regular_fields.iter().zip(&node_generics).map(|(field, node_generic)| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#node_generic: #core_types::gnode::GNode<#ctx_ident, Output = #ty>),
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
			let bound = lazy_bound(output_type);
			quote!(#node_generic: #bound)
		}
	});

	let async_bounds = match (async_fn, future_kernel) {
		(false, false) => Vec::new(),
		(false, true) => vec![quote!(#trait_output: Clone)],
		(true, _) => {
			let output_clone = std::iter::once(quote!(#trait_output: Clone));
			let value_clones = regular_fields.iter().filter_map(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some(quote!(#ty: Clone)),
				_ => None,
			});
			let data_clones = data_fields.iter().filter_map(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some(quote!(#ty: Clone)),
				_ => None,
			});
			output_clone.chain(value_clones).chain(data_clones).collect()
		}
	};

	let clampable_bounds = regular_fields.iter().filter_map(|field| {
		let ParsedFieldType::Regular(RegularParsedField { ty, number_hard_min, number_hard_max, .. }) = &field.ty else {
			return None;
		};
		(number_hard_min.is_some() || number_hard_max.is_some()).then(|| quote!(#ty: #core_types::misc::Clampable))
	});

	let eval_values = regular_fields.iter().enumerate().map(|(index, field)| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			ParsedFieldType::Regular(_) => quote! {
				let #name = match __cell.eval_input(#index, &self.#name, __input) {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				};
			},
			ParsedFieldType::Node(_) if raw_lazy => quote!(),
			ParsedFieldType::Node(_) => quote! {
				let #name = #core_types::gnode::LazyInput::new(&self.#name, &__cell, #index);
			},
		}
	});

	let clamps = regular_fields.iter().filter_map(|field| {
		let ParsedFieldType::Regular(RegularParsedField { number_hard_min, number_hard_max, .. }) = &field.ty else {
			return None;
		};
		let name = &field.pat_ident.ident;
		let mut tokens = quote!();
		if let Some(min) = number_hard_min {
			tokens.extend(quote!(let #name = #core_types::misc::Clampable::clamp_hard_min(#name, #min);));
		}
		if let Some(max) = number_hard_max {
			tokens.extend(quote!(let #name = #core_types::misc::Clampable::clamp_hard_max(#name, #max);));
		}
		(!tokens.is_empty()).then_some(tokens)
	});

	let call_args = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).map(|field| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			ParsedFieldType::Node(_) if raw_lazy => quote!(&self.#name),
			_ => quote!(#name),
		}
	});

	let value_field_names: Vec<&Ident> = regular_fields
		.iter()
		.filter(|field| matches!(field.ty, ParsedFieldType::Regular(_)))
		.map(|field| &field.pat_ident.ident)
		.collect();

	let extent_impl = match &parsed.attributes.extent {
		Some(path) => quote! {
			fn extent(&self, __input: &#ctx_ident) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent> {
				#path(self, __input)
			}
		},
		None if value_field_names.is_empty() => quote!(),
		None => {
			let first = value_field_names[0];
			let mut meet = quote!(self.#first.extent(__input));
			for name in &value_field_names[1..] {
				meet = quote!(#core_types::gpoll::Extent::meet(#meet, self.#name.extent(__input)));
			}
			quote! {
				fn extent(&self, __input: &#ctx_ident) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent> {
					#meet
				}
			}
		}
	};

	let batch_impl = match &parsed.attributes.batch {
		Some(path) => quote! {
			fn eval_batch<'__batch>(
				&self,
				__input: &'__batch #ctx_ident,
				__range: ::std::ops::Range<u64>,
				__scratch: Option<&'__batch mut [::std::mem::MaybeUninit<Self::Output>]>,
			) -> #core_types::gnode::BatchStatus<'__batch, Self::Output>
			where
				#ctx_ident: #core_types::context::InjectIndex + Copy,
			{
				#path(self, __input, __range, __scratch)
			}
		},
		None => quote!(),
	};

	let ctx_pat = &parsed.input.pat_ident;
	let fn_where = &parsed.where_clause;
	let body = &parsed.body;
	let vis = &parsed.vis;
	let kernel_fields: Vec<&&ParsedField> = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).collect();
	let kernel = match async_fn {
		false => quote! {
			#[allow(clippy::too_many_arguments)]
			#vis fn #fn_name<#(#generics,)*>(#ctx_pat: &#ctx_ident #(, #data_params)* #(, #kernel_params)*) -> #output_type #fn_where #body
		},
		true => {
			let kernel_generics = parsed.fn_generics.iter().filter(|param| match param {
				GenericParam::Type(type_param) => Some(&type_param.ident) != ctx_param.map(|ctx_param| &ctx_param.ident),
				_ => true,
			});
			let snapshot_param = snapshot_ctx.then(|| quote!(#ctx_pat: #core_types::context::CtxSnapshot)).into_iter();
			let data_kernel_params = data_fields.iter().map(|field| {
				let pat = &field.pat_ident;
				let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
					unreachable!("data fields are regular types");
				};
				quote!(#pat: #ty)
			});
			let value_kernel_params = kernel_fields.iter().map(|field| {
				let pat = &field.pat_ident;
				let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
					unreachable!("async source fields are eager values");
				};
				quote!(#pat: #ty)
			});
			let params = snapshot_param.chain(data_kernel_params).chain(value_kernel_params);
			quote! {
				#[allow(clippy::too_many_arguments)]
				#vis async fn #fn_name<#(#kernel_generics,)*>(#(#params),*) -> #output_type #fn_where #body
			}
		}
	};
	let cell_constructor = match parsed.attributes.no_partial {
		true => quote!(#core_types::gnode::StatusCell::no_partial()),
		false => quote!(#core_types::gnode::StatusCell::new()),
	};
	let kernel_call = quote!(self::#fn_name(__input #(, &self.#data_names)* #(, #call_args)*));
	let lift = match kernel_kind(&parsed.output_type) {
		KernelKind::Interrupt(_) => quote! {
			match #kernel_call {
				Ok(value) => __cell.finish(value),
				Err(interrupt) => interrupt.into(),
			}
		},
		KernelKind::Poll(_) => quote!(__cell.merge(#kernel_call)),
		_ => quote!(__cell.finish(#kernel_call)),
	};

	let placeholder_value_names: Vec<&Ident> = kernel_fields
		.iter()
		.filter(|field| matches!(field.ty, ParsedFieldType::Regular(_)))
		.map(|field| &field.pat_ident.ident)
		.collect();
	let inflight = match &parsed.attributes.placeholder {
		Some(path) => quote!(__cell.merge(#core_types::gpoll::GPoll::Partial(#path(#(&#placeholder_value_names),*)))),
		None => quote!(#core_types::gpoll::GPoll::Pending),
	};
	let slot_check = quote! {
		let __key = #core_types::registry::cache_key(__input);
		{
			let __entries = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
			if let Some(__state) = __entries.get(&__key) {
				return match __state {
					Some(value) => __cell.merge(value.clone()),
					None => #inflight,
				};
			}
		}
	};
	let future_completion = |payload: &Type| match kernel_kind(payload) {
		KernelKind::Poll(_) => quote!(__future.await),
		KernelKind::Interrupt(_) => quote! {
			match __future.await {
				Ok(value) => #core_types::gpoll::GPoll::Final(value),
				Err(interrupt) => interrupt.into(),
			}
		},
		_ => quote!(#core_types::gpoll::GPoll::Final(__future.await)),
	};
	let eval_tail = match (async_fn, future_kernel) {
		(false, false) => lift,
		(true, _) => {
			let kernel_value_names: Vec<&Ident> = kernel_fields.iter().map(|field| &field.pat_ident.ident).collect();
			let snapshot_binding = snapshot_ctx.then(|| quote!(let __snapshot = #core_types::context::CtxSnapshot::capture(__input);)).into_iter();
			let snapshot_arg = snapshot_ctx.then(|| quote!(__snapshot)).into_iter();
			let future_args = snapshot_arg
				.chain(data_names.iter().map(|name| quote!(self.#name.clone())))
				.chain(kernel_value_names.iter().map(|name| quote!(#name.clone())));
			let completion = future_completion(&parsed.output_type);
			quote! {
				#slot_check
				self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, None);
				let __slot = std::sync::Arc::clone(&self.slot);
				#(#snapshot_binding)*
				let __future = self::#fn_name(#(#future_args),*);
				_runtime.0.spawn(_source, Box::pin(async move {
					let __value = #completion;
					__slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, Some(__value));
				}));
				#inflight
			}
		}
		(false, true) => {
			let (placeholder_binding, spawn_return) = match &parsed.attributes.placeholder {
				Some(path) => (
					quote!(let __placeholder = #path(#(&#placeholder_value_names),*);),
					quote!(__cell.merge(#core_types::gpoll::GPoll::Partial(__placeholder))),
				),
				None => (quote!(), quote!(#core_types::gpoll::GPoll::Pending)),
			};
			let acquire = match kernel_kind(&parsed.output_type) {
				KernelKind::FutureInterrupt(_) => quote! {
					let __future = match #kernel_call {
						Ok(future) => future,
						Err(interrupt) => return interrupt.into(),
					};
				},
				_ => quote!(let __future = #kernel_call;),
			};
			let payload = match kernel_kind(&parsed.output_type) {
				KernelKind::Future(payload) | KernelKind::FutureInterrupt(payload) => payload,
				_ => unreachable!("guarded by future_kernel"),
			};
			let completion = future_completion(&payload);
			quote! {
				#slot_check
				#placeholder_binding
				#acquire
				self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, None);
				let __slot = std::sync::Arc::clone(&self.slot);
				_runtime.0.spawn(_source, Box::pin(async move {
					let __value = #completion;
					__slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, Some(__value));
				}));
				#spawn_return
			}
		}
	};

	let entries = entries_tokens(parsed, &struct_name, &data_field_generic_idents, &regular_fields);
	let cfg = crate::shader_nodes::modify_cfg(&parsed.attributes);
	let entries_reexport = match entries.is_empty() {
		true => quote!(),
		false => {
			let entries_name = format_ident!("{}_entries", fn_name);
			quote! {
				#cfg
				#[doc(hidden)]
				pub use #mod_name::#entries_name;
			}
		}
	};

	let top_level = quote! {
		#entries_reexport

		#cfg
		#[automatically_derived]
		impl<#(#generics,)* #(#node_generics,)*> #core_types::gnode::GNode<#ctx_ident> for #mod_name::#struct_name<#(#struct_type_params,)*>
		where
			#(#node_bounds,)*
			#(#clampable_bounds,)*
			#(#async_bounds,)*
			#(#where_predicates,)*
		{
			type Output = #trait_output;

			fn eval(&self, __input: &#ctx_ident) -> #core_types::gpoll::GPoll<Self::Output> {
				let __cell = #cell_constructor;
				#(#eval_values)*
				#(#clamps)*
				#eval_tail
			}

			#extent_impl

			#batch_impl
		}
	};

	Ok(GNodeTokens {
		in_mod: entries,
		top_level: quote! {
			#kernel

			#top_level
		},
	})
}

pub(crate) fn slot_value_type(output: &Type) -> Type {
	match kernel_kind(output) {
		KernelKind::Plain => output.clone(),
		KernelKind::Poll(inner) | KernelKind::Interrupt(inner) => inner,
		KernelKind::Future(payload) | KernelKind::FutureInterrupt(payload) => match kernel_kind(&payload) {
			KernelKind::Poll(inner) | KernelKind::Interrupt(inner) => inner,
			_ => payload,
		},
	}
}

pub(crate) fn is_source_kernel(output: &Type) -> bool {
	matches!(kernel_kind(output), KernelKind::Future(_) | KernelKind::FutureInterrupt(_))
}

enum KernelKind {
	Plain,
	Interrupt(Type),
	Poll(Type),
	Future(Type),
	FutureInterrupt(Type),
}

fn source_future_payload(segment: &syn::PathSegment) -> Type {
	let PathArguments::AngleBracketed(args) = &segment.arguments else {
		return syn::parse_quote!(());
	};
	args.args
		.iter()
		.find_map(|argument| match argument {
			GenericArgument::Type(ty) => Some(ty.clone()),
			_ => None,
		})
		.unwrap_or_else(|| syn::parse_quote!(()))
}

fn kernel_kind(output: &Type) -> KernelKind {
	let plain = || KernelKind::Plain;
	let Type::Path(path) = output else { return plain() };
	let Some(segment) = path.path.segments.last() else { return plain() };
	match segment.ident.to_string().as_str() {
		"GPoll" => {
			let PathArguments::AngleBracketed(args) = &segment.arguments else { return plain() };
			let inner = args.args.iter().find_map(|argument| match argument {
				GenericArgument::Type(ty) => Some(ty.clone()),
				_ => None,
			});
			inner.map(KernelKind::Poll).unwrap_or_else(plain)
		}
		"SourceFuture" => KernelKind::Future(source_future_payload(segment)),
		"Result" => {
			let PathArguments::AngleBracketed(args) = &segment.arguments else { return plain() };
			let mut types = args.args.iter().filter_map(|argument| match argument {
				GenericArgument::Type(ty) => Some(ty),
				_ => None,
			});
			let (Some(inner), Some(Type::Path(error_path))) = (types.next(), types.next()) else {
				return plain();
			};
			if !error_path.path.segments.last().is_some_and(|segment| segment.ident == "Interrupt") {
				return plain();
			}
			if let Type::Path(inner_path) = inner {
				if let Some(inner_segment) = inner_path.path.segments.last() {
					if inner_segment.ident == "SourceFuture" {
						return KernelKind::FutureInterrupt(source_future_payload(inner_segment));
					}
				}
			}
			KernelKind::Interrupt(inner.clone())
		}
		_ => plain(),
	}
}

fn context_param<'a>(parsed: &'a ParsedNodeFn) -> Option<&'a TypeParam> {
	let Type::Path(path) = &parsed.input.ty else {
		return None;
	};
	let ident = path.path.get_ident()?;
	parsed.fn_generics.iter().find_map(|param| match param {
		GenericParam::Type(type_param) if &type_param.ident == ident => Some(type_param),
		_ => None,
	})
}

fn type_disqualifies(ty: &Type) -> bool {
	struct Disqualifier {
		found: bool,
	}

	impl<'ast> Visit<'ast> for Disqualifier {
		fn visit_type_reference(&mut self, _: &'ast syn::TypeReference) {
			self.found = true;
		}

		fn visit_type_impl_trait(&mut self, _: &'ast syn::TypeImplTrait) {
			self.found = true;
		}

		fn visit_lifetime(&mut self, _: &'ast Lifetime) {
			self.found = true;
		}
	}

	let mut visitor = Disqualifier { found: false };
	visitor.visit_type(ty);
	visitor.found
}

fn desugar_extract_lifetime(bound: &TypeParamBound, core_types: &TokenStream2) -> TokenStream2 {
	let TypeParamBound::Trait(trait_bound) = bound else {
		return quote!(#bound);
	};
	let Some(segment) = trait_bound.path.segments.last() else {
		return quote!(#bound);
	};
	if segment.ident != "ExtractArena" {
		return quote!(#bound);
	}
	let PathArguments::AngleBracketed(args) = &segment.arguments else {
		return quote!(#bound);
	};
	if args.args.len() != 1 {
		return quote!(#bound);
	}
	let Some(GenericArgument::Lifetime(lifetime)) = args.args.first() else {
		return quote!(#bound);
	};
	quote!(#core_types::context::ExtractArena<ArenaRef = &#lifetime #core_types::arena::Arena>)
}

fn entries_tokens(parsed: &ParsedNodeFn, struct_name: &Ident, data_field_generic_idents: &[Ident], regular_fields: &[&ParsedField]) -> TokenStream2 {
	if !data_field_generic_idents.is_empty() {
		return quote!();
	}
	let Some(rows) = implementation_rows(parsed, regular_fields) else {
		return quote!();
	};
	let rows: Vec<&Vec<Type>> = rows.iter().filter(|row| row.iter().all(|ty| !type_disqualifies(ty))).collect();
	if rows.is_empty() {
		return quote!();
	}

	let fn_name = &parsed.fn_name;
	let entries_name = format_ident!("{}_entries", fn_name);
	let arity = regular_fields.len();
	let names: Vec<&Ident> = regular_fields.iter().map(|field| &field.pat_ident.ident).collect();

	let entries = rows.iter().map(|row| {
		let types = row.iter();
		let boxed_types = row.iter().map(|ty| quote!(::std::boxed::Box<gcore::registry::ErasedGNode<#ty>>));
		let output = quote!(<#struct_name<#(#boxed_types),*> as gcore::gnode::GNode<gcore::context::ContextImpl<'static>>>::Output);
		let downcasts = names.iter().zip(row.iter()).map(|(name, ty)| {
			quote!(let #name = inputs.next().unwrap().downcast::<#ty>()?;)
		});
		quote! {
			gcore::registry::RegistryEntry {
				io: gcore::registry::NodeIoRecord {
					inputs: vec![#(gcore::concrete!(#types)),*],
					output: gcore::concrete!(#output),
				},
				constructor: |inputs| {
					if inputs.len() != #arity {
						return Err(gcore::registry::ConstructionError::Arity { expected: #arity, got: inputs.len() });
					}
					let mut inputs = inputs.into_iter();
					#(#downcasts)*
					Ok(gcore::registry::EdgeHandle::new(::std::boxed::Box::new(#struct_name::new(#(#names),*)) as ::std::boxed::Box<gcore::registry::ErasedGNode<#output>>))
				},
			}
		}
	});

	quote! {
		pub fn #entries_name() -> ::std::vec::Vec<gcore::registry::RegistryEntry> {
			vec![#(#entries),*]
		}
	}
}

fn implementation_rows(parsed: &ParsedNodeFn, regular_fields: &[&ParsedField]) -> Option<Vec<Vec<Type>>> {
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	let open_generics: Vec<&Ident> = parsed
		.fn_generics
		.iter()
		.filter_map(|param| match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref() => Some(&type_param.ident),
			_ => None,
		})
		.collect();

	let candidates: Vec<Vec<Type>> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. }) => match implementations.is_empty() {
				false => Some(implementations.iter().cloned().collect()),
				true => open_generics.iter().all(|generic| !crate::codegen::type_contains_ident(ty, generic)).then(|| vec![ty.clone()]),
			},
			ParsedFieldType::Node(NodeParsedField { output_type, implementations, .. }) => match implementations.is_empty() {
				false => Some(implementations.iter().map(|implementation| implementation.output.clone()).collect()),
				true => open_generics.iter().all(|generic| !crate::codegen::type_contains_ident(output_type, generic)).then(|| vec![output_type.clone()]),
			},
		})
		.collect::<Option<_>>()?;

	let row_count = candidates.iter().map(|types| types.len()).max().unwrap_or(1).max(1);
	Some(
		(0..row_count)
			.map(|row| candidates.iter().map(|types| types[row.min(types.len() - 1)].clone()).collect())
			.collect(),
	)
}
