use crate::parsing::{Implementation, NodeParsedField, ParsedField, ParsedFieldType, ParsedNodeFn, RegularParsedField, attr_marker, record_writes, remove_attr_marker};
use proc_macro_error2::emit_error;
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{GenericParam, Type};

pub fn validate_node_fn(parsed: &ParsedNodeFn) -> syn::Result<()> {
	let validators: &[fn(&ParsedNodeFn)] = &[
		// Add more validators here as needed
		validate_implementations_for_generics,
		validate_primary_input_expose,
		validate_min_max,
		validate_range_slider_bounds,
		validate_async_source,
		validate_lend_fields,
		validate_record_io,
		validate_lazy_reads,
	];

	for validator in validators {
		validator(parsed);
	}

	Ok(())
}

fn validate_record_io(parsed: &ParsedNodeFn) {
	let value = crate::codegen::slot_value_type(&parsed.output_type);
	if let Type::Tuple(tuple) = &value {
		let has_marker_slot = tuple.elems.iter().any(|slot| attr_marker(slot).is_some() || remove_attr_marker(slot).is_some());
		if has_marker_slot && record_writes(&value).is_none() {
			emit_error!(
				parsed.output_type.span(),
				"a record return tuple is the element first, then only `Attr<..>` writes and `RemoveAttr<..>` deletions"
			);
		}
	} else if attr_marker(&value).is_some() || remove_attr_marker(&value).is_some() {
		emit_error!(parsed.output_type.span(), "an attribute write needs an element in the first tuple slot, e.g. `(T, Attr<..>)`");
	}

	let writes = record_writes(&value);
	let has_reads = parsed.fields.iter().any(|field| !field.attribute_reads.is_empty() && matches!(field.ty, ParsedFieldType::Regular(_)));
	if !has_reads && writes.is_none() {
		return;
	}

	let async_source = crate::codegen::classify::is_async_source(parsed);
	if async_source && has_reads {
		emit_error!(parsed.fn_name.span(), "attribute reads are not supported on async source kernels, only writes");
	}
	if crate::codegen::is_poll_kernel(&parsed.output_type) {
		emit_error!(parsed.output_type.span(), "attribute io needs a plain or `Result<_, Interrupt>` kernel, not a `GPoll` one");
	}

	for field in parsed.fields.iter().skip(1) {
		if let ParsedFieldType::Node(NodeParsedField { output_type, .. }) = &field.ty {
			// Lazy secondaries are consumed as plain elements through the input.
			if crate::codegen::classify::is_served(output_type) || crate::codegen::ir::strip_ilist(output_type).1 > 0 {
				emit_error!(field.pat_ident.span(), "a record node's lazy inputs consume plain elements, not record or ranked wires");
			}
			if !field.attribute_reads.is_empty() {
				emit_error!(field.pat_ident.span(), "attribute reads on a record node's lazy inputs are not supported yet");
			}
		}
	}
	for (index, field) in parsed.fields.iter().enumerate() {
		if field.attribute_reads.is_empty() {
			continue;
		}
		if field.is_data_field {
			emit_error!(field.pat_ident.span(), "a `#[data]` field has no wire to read attributes from");
			continue;
		}
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }) => {
				emit_error!(field.pat_ident.span(), "attribute reads need an owned value; take `T` instead of `&T`");
			}
			ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. }) => {
				if matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty()) {
					emit_error!(field.pat_ident.span(), "attribute-only inputs are not supported yet; the value component cannot be `()`");
				}
				let is_token_carrier = index == 0 && implementations.is_empty() && crate::codegen::unbounded_generic(parsed, ty).is_some();
				if !is_token_carrier && crate::codegen::contains_open_generic(parsed, ty) {
					emit_error!(
						field.pat_ident.span(),
						"a reading input's value is monomorphic for now; use a concrete type or an unbounded passthrough generic in the primary input"
					);
				}
			}
			// Lazy-input reads are validated by `validate_lazy_reads`; a
			// record-io node already rejects lazy inputs above.
			ParsedFieldType::Node(_) => {}
		}
	}

	let Some(carrier) = parsed.fields.first() else {
		emit_error!(parsed.fn_name.span(), "attribute io needs a primary input as the first parameter after the context (`_: ()` for none)");
		return;
	};
	let lazy_carrier = matches!(&crate::codegen::record_shape(parsed), Some(shape) if matches!(shape.carrier, crate::codegen::RecordCarrier::LazyToken));
	let carrier_ty = match &carrier.ty {
		ParsedFieldType::Regular(RegularParsedField { ty, lend: None, .. }) if !carrier.is_data_field => Some(ty),
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) if lazy_carrier => Some(output_type),
		_ => None,
	};
	let Some(carrier_ty) = carrier_ty else {
		emit_error!(
			carrier.pat_ident.span(),
			"a record node's primary input is an owned element, an unbounded passthrough generic, a lazy passthrough source, or `_: ()`; not `#[data]` or `&T`"
		);
		return;
	};
	let node = crate::codegen::ir::build(parsed);
	if lazy_carrier && !node.derives {
		emit_error!(parsed.input.pat_ident.span(), "a lazy record carrier evaluates at derived contexts; spell `impl Ctx + DeriveCtx`");
		return;
	}
	if node.output.gathers && crate::codegen::ir::gathered_subject(&node).is_none() {
		emit_error!(parsed.output_type.span(), "a `Lane` output gathers a materialized subject; give the primary input an `IList` type");
		return;
	}

	let no_carrier = matches!(carrier_ty, Type::Tuple(tuple) if tuple.elems.is_empty());
	let token = match (no_carrier, &carrier.ty) {
		(false, ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. })) if implementations.is_empty() => crate::codegen::unbounded_generic(parsed, ty),
		(false, ParsedFieldType::Node(NodeParsedField { output_type, .. })) if lazy_carrier => crate::codegen::unbounded_generic(parsed, output_type),
		_ => None,
	};
	if async_source && token.is_some() {
		emit_error!(carrier.pat_ident.span(), "an async source's element crosses the future boundary as a value; a passthrough generic element has none");
	}
	let element = writes.as_ref().map(|writes| &writes.element).unwrap_or(&value);
	match &token {
		Some(token) => {
			if !matches!(crate::codegen::bare_ident(element), Some(ident) if ident == token) {
				emit_error!(parsed.output_type.span(), "a generic element passes through unchanged: return `{}` in the first tuple position", token);
			}
		}
		None => {
			if let Some(ident) = crate::codegen::unbounded_generic(parsed, element) {
				emit_error!(parsed.output_type.span(), "the returned generic element `{}` has no matching input", ident);
			} else if !no_carrier && !node.output.gathers && crate::codegen::contains_open_generic(parsed, carrier_ty) {
				emit_error!(
					carrier.pat_ident.span(),
					"record element reads are monomorphic for now; use a concrete element type or an unbounded passthrough generic"
				);
			} else if !node.output.gathers && crate::codegen::contains_open_generic(parsed, element) {
				emit_error!(parsed.output_type.span(), "a written element must be a concrete type");
			}
		}
	}

	for field in &parsed.fields {
		let mut seen_reads: Vec<String> = Vec::new();
		for read in &field.attribute_reads {
			let marker = read.marker.to_token_stream().to_string();
			if seen_reads.contains(&marker) {
				emit_error!(read.pat_ident.span(), "attribute `{}` is read twice from `{}`", marker, field.pat_ident.ident);
			}
			seen_reads.push(marker);
		}
	}
	if let Some(writes) = &writes {
		let mut seen_writes: Vec<String> = Vec::new();
		for write in &writes.markers {
			if write.owned && !async_source {
				emit_error!(parsed.output_type.span(), "an owned attribute crossing belongs to an async source; a synchronous write parks its value in the kernel");
			}
			let written = write.marker.to_token_stream().to_string();
			if seen_writes.contains(&written) {
				emit_error!(parsed.output_type.span(), "attribute `{}` is written twice", written);
			}
			seen_writes.push(written);
		}
		let mut seen_removes: Vec<String> = Vec::new();
		for marker in &writes.removes {
			let removed = marker.to_token_stream().to_string();
			if seen_removes.contains(&removed) {
				emit_error!(parsed.output_type.span(), "attribute `{}` is removed twice", removed);
			}
			if seen_writes.contains(&removed) {
				emit_error!(parsed.output_type.span(), "attribute `{}` is both written and removed", removed);
			}
			seen_removes.push(removed);
		}
		if no_carrier && !writes.removes.is_empty() {
			emit_error!(parsed.output_type.span(), "a node without a primary input writes a fresh record; there is nothing to remove");
		}
	}
}

fn validate_lazy_reads(parsed: &ParsedNodeFn) {
	if !crate::codegen::has_lazy_reads(parsed) {
		return;
	}
	let lazy_carrier = matches!(&crate::codegen::record_shape(parsed), Some(shape) if matches!(shape.carrier, crate::codegen::RecordCarrier::LazyToken));
	if !crate::codegen::record_flip(parsed) && !lazy_carrier {
		emit_error!(
			parsed.fn_name.span(),
			"attribute reads on a lazy input need the record lowering; routing, `plain`, shader, batch, and non-row-assignable generic nodes keep the plain one"
		);
	}
	for (index, field) in parsed.fields.iter().enumerate() {
		let ParsedFieldType::Node(NodeParsedField { output_type, .. }) = &field.ty else {
			continue;
		};
		if field.attribute_reads.is_empty() {
			continue;
		}
		// The lazy carrier forwards its token AND reads: the reads resolve
		// against its wired layout, not the row type.
		if crate::codegen::unbounded_generic(parsed, output_type).is_some() && !(lazy_carrier && index == 0) {
			emit_error!(
				field.pat_ident.span(),
				"an unbounded generic source forwards its whole record; attribute reads need a concrete output type"
			);
		}
		let mut seen: Vec<String> = Vec::new();
		for read in &field.attribute_reads {
			let marker = read.marker.to_token_stream().to_string();
			if seen.contains(&marker) {
				emit_error!(read.marker.span(), "attribute `{}` is read twice from `{}`", marker, field.pat_ident.ident);
			}
			seen.push(marker);
		}
	}
}

fn validate_async_source(parsed: &ParsedNodeFn) {
	let snapshot_ctx = matches!(&parsed.input.ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "CtxSnapshot"));
	let future_kernel = crate::codegen::is_source_kernel(&parsed.output_type);
	if let Some(placeholder) = &parsed.attributes.placeholder
		&& !parsed.is_async
		&& !future_kernel
	{
		emit_error!(
			placeholder.span(),
			"`placeholder` applies only to async and source kernels; a synchronous node never reports `Partial`, so the stand-in is unused"
		);
	}
	if parsed.is_async && future_kernel {
		emit_error!(
			parsed.output_type.span(),
			"an `async fn` kernel already is the async part; returning `SourceFuture` is the sync-prologue form, so drop the `async` keyword or return the value directly"
		);
		return;
	}
	if !parsed.is_async {
		if snapshot_ctx {
			emit_error!(
				parsed.input.pat_ident.span(),
				"`CtxSnapshot` is the async source context; synchronous nodes take `impl Ctx` and read through extract bounds"
			);
		}
		if !future_kernel {
			return;
		}
	}
	if parsed.is_async {
		for field in &parsed.fields {
			if matches!(field.ty, ParsedFieldType::Node(_)) {
				emit_error!(
					field.pat_ident.span(),
					"`async fn` source nodes cannot take `impl Node` inputs: the spawned future outlives any borrow of the graph, so it cannot evaluate other nodes; use the sync-prologue form (return `SourceFuture`) to evaluate lazy inputs before spawning"
				);
			}
		}
	}
}

fn validate_lend_fields(parsed: &ParsedNodeFn) {
	let future_kernel = crate::codegen::is_source_kernel(&parsed.output_type);
	for field in &parsed.fields {
		let ParsedFieldType::Regular(RegularParsedField { lend: Some(reference), .. }) = &field.ty else {
			continue;
		};
		if let Some(mutability) = &reference.mutability {
			emit_error!(mutability.span(), "reference parameters are read-only lends; `&mut` is not supported");
		}
		if let Some(lifetime) = &reference.lifetime {
			emit_error!(lifetime.span(), "reference parameters use the eval lifetime implicitly; write a bare `&T`");
		}
		if field.is_data_field {
			emit_error!(field.pat_ident.span(), "`#[data]` fields are node-resident state and cannot be references");
		}
		if parsed.is_async || future_kernel {
			emit_error!(
				field.pat_ident.span(),
				"source kernels move their inputs into the spawned task, so they cannot take reference parameters"
			);
		}
	}
}

fn validate_min_max(parsed: &ParsedNodeFn) {
	for field in &parsed.fields {
		if let ParsedField {
			ty: ParsedFieldType::Regular(RegularParsedField {
				number_hard_max,
				number_hard_min,
				number_soft_max,
				number_soft_min,
				..
			}),
			pat_ident,
			..
		} = field
		{
			if let (Some(soft_min), Some(hard_min)) = (number_soft_min, number_hard_min) {
				let soft_min_value: f64 = soft_min.to_f64();
				let hard_min_value: f64 = hard_min.to_f64();
				if soft_min_value == hard_min_value {
					emit_error!(
						pat_ident.span(),
						"Redundant lower bound on `{}`: the #[soft] and #[hard] lower bounds are equal.",
						pat_ident.ident;
						help = "Drop the lower bound from #[soft] and let the slider fall back to #[hard].";
						note = "A soft bound only matters when it sits inside the corresponding hard bound.",
					);
				} else if soft_min_value < hard_min_value {
					emit_error!(
						pat_ident.span(),
						"The #[soft] lower bound on `{}` is below the #[hard] lower bound.",
						pat_ident.ident;
						help = "The soft (slider) range must stay within the hard (clamped) range.";
						note = "Letting the slider range precede #[hard]'s lower bound doesn't make sense.",
					);
				}
			}

			if let (Some(soft_max), Some(hard_max)) = (number_soft_max, number_hard_max) {
				let soft_max_value: f64 = soft_max.to_f64();
				let hard_max_value: f64 = hard_max.to_f64();
				if soft_max_value == hard_max_value {
					emit_error!(
						pat_ident.span(),
						"Redundant upper bound on `{}`: the #[soft] and #[hard] upper bounds are equal.",
						pat_ident.ident;
						help = "Drop the upper bound from #[soft] and let the slider fall back to #[hard].";
						note = "A soft bound only matters when it sits inside the corresponding hard bound.",
					);
				} else if soft_max_value > hard_max_value {
					emit_error!(
						pat_ident.span(),
						"The #[soft] upper bound on `{}` is above the #[hard] upper bound.",
						pat_ident.ident;
						help = "The soft (slider) range must stay within the hard (clamped) range.";
						note = "Letting the slider range exceed #[hard]'s upper bound doesn't make sense.",
					);
				}
			}
		}
	}
}

/// A `#[range]` slider needs a defined extent on both ends. The extent comes from `#[soft]` when present,
/// otherwise it falls back to `#[hard]`, so each end must be covered by at least one of the two attributes.
fn validate_range_slider_bounds(parsed: &ParsedNodeFn) {
	for field in &parsed.fields {
		if let ParsedField {
			ty: ParsedFieldType::Regular(RegularParsedField {
				number_mode_range: true,
				number_soft_min,
				number_soft_max,
				number_hard_min,
				number_hard_max,
				..
			}),
			pat_ident,
			..
		} = field
		{
			let min_bounded = number_soft_min.is_some() || number_hard_min.is_some();
			let max_bounded = number_soft_max.is_some() || number_hard_max.is_some();

			let missing = match (min_bounded, max_bounded) {
				(true, true) => continue,
				(false, false) => "lower and upper bounds",
				(false, true) => "a lower bound",
				(true, false) => "an upper bound",
			};

			emit_error!(
				pat_ident.span(),
				"The #[range] slider on `{}` is missing {}.",
				pat_ident.ident, missing;
				help = "A slider needs both ends defined; add the missing bound via #[soft(..)] or #[hard(..)], e.g. #[soft(0..100)].";
				note = "The slider's extent comes from #[soft] if present, otherwise #[hard].",
			);
		}
	}
}

fn validate_primary_input_expose(parsed: &ParsedNodeFn) {
	if let Some(ParsedField {
		ty: ParsedFieldType::Regular(RegularParsedField { exposed: true, .. }),
		pat_ident,
		..
	}) = parsed.fields.first()
	{
		emit_error!(
			pat_ident.span(),
			"Unnecessary #[expose] attribute on primary input `{}`. Primary inputs are always exposed.",
			pat_ident.ident;
			help = "You can safely remove the #[expose] attribute from this field.";
			note = "The function's second argument, `{}`, is the node's primary input and it's always exposed by default", pat_ident.ident
		);
	}
}

fn validate_implementations_for_generics(parsed: &ParsedNodeFn) {
	let has_skip_impl = parsed.attributes.skip_impl;
	let routing = crate::codegen::routing_io(parsed);
	let node = crate::codegen::ir::build(parsed);
	let record_token = match (crate::codegen::ir::node_kind(&node), &node.output.shape.element) {
		(crate::codegen::ir::NodeKind::RecordIo, crate::codegen::ir::Element::Generic(ident)) => Some(ident.clone()),
		_ => None,
	};
	let opaque_record_generic = |ty: &Type| {
		let (stripped, _) = crate::codegen::ir::strip_ilist(ty);
		let ident = match &stripped {
			Type::Path(path) => path.path.get_ident(),
			_ => None,
		};
		ident.is_some() && (ident == routing.as_ref().map(|routing| &routing.generic) || ident == record_token.as_ref())
	};

	if !has_skip_impl && !parsed.fn_generics.is_empty() {
		for field in &parsed.fields {
			// Skip validation for data fields - they're internal state and can be generic
			if field.is_data_field {
				continue;
			}

			let pat_ident = &field.pat_ident;
			match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. }) => {
					if opaque_record_generic(ty) {
						continue;
					}
					if contains_generic_param(ty, &parsed.fn_generics) && implementations.is_empty() {
						emit_error!(
							ty.span(),
							"Generic type `{}` in field `{}` requires an #[implementations(...)] attribute",
							quote!(#ty),
							pat_ident.ident;
							help = "Add #[implementations(ConcreteType1, ConcreteType2)] to field '{}'", pat_ident.ident;
							help = "Or use #[node_macro::node(category(...), skip_impl)] if you want to manually implement the node"
						);
					}
				}
				ParsedFieldType::Node(NodeParsedField {
					input_type,
					output_type,
					implementations,
					..
				}) => {
					if opaque_record_generic(output_type) {
						continue;
					}
					if (contains_generic_param(input_type, &parsed.fn_generics) || contains_generic_param(output_type, &parsed.fn_generics)) && implementations.is_empty() {
						emit_error!(
							pat_ident.span(),
							"Generic types in Node field `{}` require an #[implementations(...)] attribute",
							pat_ident.ident;
							help = "Add #[implementations(InputType1 -> OutputType1, InputType2 -> OutputType2)] to field '{}'", pat_ident.ident;
							help = "Or use #[node_macro::node(category(...), skip_impl)] if you want to manually implement the node"
						);
					}
					// Additional check for Node implementations
					for impl_ in implementations {
						validate_node_implementation(impl_, input_type, output_type, &parsed.fn_generics);
					}
				}
			}
		}
	}
}

fn validate_node_implementation(impl_: &Implementation, input_type: &Type, output_type: &Type, fn_generics: &[GenericParam]) {
	if contains_generic_param(&impl_.input, fn_generics) || contains_generic_param(&impl_.output, fn_generics) {
		emit_error!(
			impl_.input.span(),
			"Implementation types `{}` and `{}` must be concrete, not generic",
			quote!(#input_type), quote!(#output_type);
			help = "Replace generic types with concrete types in the implementation"
		);
	}
}

fn contains_generic_param(ty: &Type, fn_generics: &[GenericParam]) -> bool {
	struct GenericParamChecker<'a> {
		fn_generics: &'a [GenericParam],
		found: bool,
	}

	impl<'a> syn::visit::Visit<'a> for GenericParamChecker<'a> {
		fn visit_ident(&mut self, ident: &'a syn::Ident) {
			if self
				.fn_generics
				.iter()
				.any(|param| if let GenericParam::Type(type_param) = param { type_param.ident == *ident } else { false })
			{
				self.found = true;
			}
		}
	}

	let mut checker = GenericParamChecker { fn_generics, found: false };
	syn::visit::visit_type(&mut checker, ty);
	checker.found
}
