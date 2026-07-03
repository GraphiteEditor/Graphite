use crate::parsing::{Implementation, NodeParsedField, ParsedField, ParsedFieldType, ParsedNodeFn, RegularParsedField};
use proc_macro_error2::emit_error;
use quote::quote;
use syn::spanned::Spanned;
use syn::{GenericParam, Type};

pub fn validate_node_fn(parsed: &ParsedNodeFn) -> syn::Result<()> {
	let validators: &[fn(&ParsedNodeFn)] = &[
		// Add more validators here as needed
		validate_implementations_for_generics,
		validate_primary_input_expose,
		validate_min_max,
		validate_range_slider_bounds,
		validate_no_item_parameters,
		validate_element_wise,
	];

	for validator in validators {
		validator(parsed);
	}

	Ok(())
}

fn validate_no_item_parameters(parsed: &ParsedNodeFn) {
	// The primary input (index 0) may be Item<T>, which declares the node as an element-wise kernel
	for field in parsed.fields.iter().skip(1) {
		let ParsedField {
			ty: ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. }),
			pat_ident,
			..
		} = field
		else {
			continue;
		};

		if outer_wrapper_is(ty, "Item") || implementations.iter().any(|ty| outer_wrapper_is(ty, "Item")) {
			emit_error!(
				pat_ident.span(),
				"Parameter `{}` must not be typed `Item<T>`.",
				pat_ident.ident;
				help = "Only the primary input may be typed `Item<T>`, which declares the node as an element-wise kernel.";
			);
		}
	}
}

fn validate_element_wise(parsed: &ParsedNodeFn) {
	let Some(primary) = parsed.fields.first() else { return };
	let ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. }) = &primary.ty else {
		return;
	};

	if !outer_wrapper_is(ty, "Item") {
		if outer_wrapper_is(&parsed.output_type, "Item") {
			emit_error!(
				parsed.output_type.span(),
				"Returning `Item<T>` requires the primary input to also be typed `Item<T>`, declaring an element-wise node"
			);
		}
		return;
	}

	if primary.is_data_field {
		emit_error!(primary.pat_ident.span(), "The `Item<T>` primary input `{}` cannot be a #[data] field", primary.pat_ident.ident);
	}

	if parsed.attributes.shader_node.is_some() {
		emit_error!(parsed.fn_name.span(), "An element-wise `Item<T>` primary input cannot be combined with `shader_node`");
	}

	if implementations.iter().any(|ty| outer_wrapper_is(ty, "List") || outer_wrapper_is(ty, "Item")) {
		emit_error!(
			primary.pat_ident.span(),
			"The #[implementations(...)] of `{}` must be bare element types; the macro derives the Item and List wire forms",
			primary.pat_ident.ident
		);
	}

	if !outer_wrapper_is(&parsed.output_type, "Item") {
		emit_error!(parsed.output_type.span(), "An element-wise node (declared by its `Item<T>` primary input) must return `Item<U>`");
	}
}

/// Returns whether the type's outermost path segment is the given wrapper name, like `Item` in `Item<T>`.
fn outer_wrapper_is(ty: &Type, wrapper: &str) -> bool {
	let Type::Path(type_path) = ty else { return false };
	type_path.path.segments.last().is_some_and(|segment| segment.ident == wrapper)
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

	if !has_skip_impl && !parsed.fn_generics.is_empty() {
		for field in &parsed.fields {
			// Skip validation for data fields - they're internal state and can be generic
			if field.is_data_field {
				continue;
			}

			let pat_ident = &field.pat_ident;
			match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, implementations, .. }) => {
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
