use crate::parsing::{Implementation, NodeParsedField, ParsedFieldType, ParsedNodeFn, RegularParsedField};
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
		validate_ranked_inputs,
	];

	for validator in validators {
		validator(parsed);
	}

	Ok(())
}

fn validate_no_item_parameters(parsed: &ParsedNodeFn) {
	if parsed.attributes.skip_impl {
		return;
	}

	// An `Item` primary shares its element-wise frame with ranked parameters; a `List`/`ListDyn` aggregation primary accepts
	// them as fixed ranked inputs; a `()` generator has no primary and draws its frame from the ranked parameters themselves.
	let ranked = |ty: &Type| outer_wrapper_is(ty, "Item") || outer_wrapper_is(ty, "List") || outer_wrapper_is(ty, "ListDyn");
	let primary = parsed.primary_input_field();
	let primary_permits_item_params = match primary.map(|(_, field)| &field.ty) {
		Some(ParsedFieldType::Node(NodeParsedField { output_type, implementations, .. })) => ranked(output_type) || implementations.iter().any(|implementation| ranked(&implementation.output)),
		Some(value) => {
			let regular = value.regular().expect("a non-node primary is a value field");
			is_unit_type(&regular.ty) || ranked(&regular.ty) || regular.implementations.iter().any(ranked)
		}
		None => false,
	};
	let primary_index = primary.map(|(index, _)| index);

	for (index, field) in parsed.fields.iter().enumerate() {
		if Some(index) == primary_index || field.is_environment() {
			continue;
		}
		let Some(RegularParsedField { ty, implementations, .. }) = field.ty.regular() else {
			continue;
		};
		let pat_ident = &field.pat_ident;

		// A ranked parameter requires a ranked primary: `Item<T>` (element-wise frame) or `List<T>`/`ListDyn` (aggregation)
		if outer_wrapper_is(ty, "Item") && !primary_permits_item_params {
			emit_error!(
				pat_ident.span(),
				"The `Item<T>` parameter `{}` requires the primary input to be ranked (`Item<T>`, `List<T>`, or `ListDyn`)",
				pat_ident.ident
			);
		}

		if outer_wrapper_is(ty, "Item")
			&& implementations
				.iter()
				.any(|ty| outer_wrapper_is(ty, "Item") || outer_wrapper_is(ty, "List") || outer_wrapper_is(ty, "ListDyn"))
		{
			emit_error!(pat_ident.span(), "The #[implementations(...)] of the ranked parameter `{}` must be bare element types", pat_ident.ident);
		}
	}
}

fn validate_element_wise(parsed: &ParsedNodeFn) {
	if parsed.attributes.skip_impl {
		return;
	}

	let Some((_, primary)) = parsed.primary_input_field() else { return };
	let Some(RegularParsedField { ty, implementations, .. }) = primary.ty.regular() else {
		return;
	};

	if !outer_wrapper_is(ty, "Item") {
		// A non-`Item` primary may still emit a rank-0 `Item<T>`: a `()` generator has no input, and a `List<T>` or
		// `ListDyn` aggregation (declared directly or via a generic primary's implementation rows) reduces a whole list down to a single item.
		let primary_reduces_or_generates = is_unit_type(ty)
			|| primary.ty.list_element().is_some()
			|| outer_wrapper_is(ty, "ListDyn")
			|| implementations.iter().any(|ty| outer_wrapper_is(ty, "List") || outer_wrapper_is(ty, "ListDyn"));
		if outer_wrapper_is(&parsed.output_type, "Item") && !primary_reduces_or_generates {
			emit_error!(
				parsed.output_type.span(),
				"Returning `Item<T>` requires the primary input to be `Item<T>` (element-wise), `List<T>`/`ListDyn` (aggregation), or `()` (generator)"
			);
		}
		return;
	}

	if implementations
		.iter()
		.any(|ty| outer_wrapper_is(ty, "List") || outer_wrapper_is(ty, "Item") || outer_wrapper_is(ty, "ListDyn"))
	{
		emit_error!(
			primary.pat_ident.span(),
			"The #[implementations(...)] of `{}` must be bare element types; the macro derives the Item and List wire forms",
			primary.pat_ident.ident
		);
	}

	if !outer_wrapper_is(&parsed.output_type, "Item") && !outer_wrapper_is(&parsed.output_type, "List") {
		emit_error!(
			parsed.output_type.span(),
			"An element-wise node (declared by its `Item<T>` primary input) must return `Item<U>`, or `List<U>` for an expander"
		);
	}
}

/// Every input must ride a ranked wire: `Item<T>`, `List<T>`, or `ListDyn`, declared directly or substituted per
/// implementations row when the field's type is a bare generic. The () type stays legal as the no-primary-input
/// sentinel for generator nodes, and `#[data]` fields are exempt as internal state rather than wires.
fn validate_ranked_inputs(parsed: &ParsedNodeFn) {
	for (span, message) in ranked_input_violations(parsed) {
		emit_error!(span, "{}", message);
	}
}

fn ranked_input_violations(parsed: &ParsedNodeFn) -> Vec<(proc_macro2::Span, String)> {
	let mut violations = Vec::new();
	if parsed.attributes.skip_impl {
		return violations;
	}

	let ranked = |ty: &Type| outer_wrapper_is(ty, "Item") || outer_wrapper_is(ty, "List") || outer_wrapper_is(ty, "ListDyn");
	let primary_index = parsed.primary_input_field().map(|(index, _)| index);

	for (index, field) in parsed.fields.iter().enumerate() {
		if field.is_data_field {
			continue;
		}
		let pat_ident = &field.pat_ident;

		match &field.ty {
			ParsedFieldType::Node(NodeParsedField { output_type, implementations, .. }) => {
				if ranked(output_type) {
					continue;
				}

				if is_bare_generic(output_type, &parsed.fn_generics) {
					for row in implementations {
						let output = &row.output;
						if !ranked(output) {
							violations.push((
								output.span(),
								format!(
									"Implementations row output `{ty}` of the lazy input `{name}` must be ranked: produce `Item<{ty}>` for one cell or `List<{ty}>` for a whole list",
									name = pat_ident.ident,
									ty = quote!(#output)
								),
							));
						}
					}
					continue;
				}

				violations.push((
					pat_ident.span(),
					format!(
						"Lazy input `{name}` with output type `{ty}` must be ranked: declare its `Output` as `Item<{ty}>` for one cell or `List<{ty}>` for a whole list",
						name = pat_ident.ident,
						ty = quote!(#output_type)
					),
				));
			}
			value => {
				let RegularParsedField { ty, implementations, .. } = value.regular().expect("a non-node field is a value field");

				if is_unit_type(ty) {
					if Some(index) != primary_index {
						violations.push((
							pat_ident.span(),
							format!(
								"Parameter `{}` cannot be typed `()`: the unit type is only the no-primary-input sentinel for generator nodes",
								pat_ident.ident
							),
						));
					}
					continue;
				}

				if ranked(ty) {
					continue;
				}

				// A bare-generic field takes each implementations row as its whole wire type, so the rows carry the rank
				if is_bare_generic(ty, &parsed.fn_generics) {
					for row in implementations {
						if !ranked(row) && !is_bare_generic(row, &parsed.fn_generics) {
							violations.push((
								row.span(),
								format!(
									"Implementations row `{row}` of `{name}` must be ranked: wrap it as `Item<{row}>` to consume one cell or `List<{row}>` to consume a whole list",
									name = pat_ident.ident,
									row = quote!(#row)
								),
							));
						}
					}
					continue;
				}

				violations.push((
					pat_ident.span(),
					format!(
						"Parameter `{name}` of type `{ty}` must be ranked: wrap it as `Item<{ty}>` to consume one cell or `List<{ty}>` to consume a whole list",
						name = pat_ident.ident,
						ty = quote!(#ty)
					),
				));
			}
		}
	}

	violations
}

/// Returns whether the type is exactly one of the function's generic parameters, like `T`.
fn is_bare_generic(ty: &Type, fn_generics: &[GenericParam]) -> bool {
	let Type::Path(type_path) = ty else { return false };
	let Some(ident) = type_path.path.get_ident() else { return false };
	fn_generics.iter().any(|param| matches!(param, GenericParam::Type(type_param) if type_param.ident == *ident))
}

/// Returns whether the type's outermost path segment is the given wrapper name, like `Item` in `Item<T>`.
fn outer_wrapper_is(ty: &Type, wrapper: &str) -> bool {
	let Type::Path(type_path) = ty else { return false };
	type_path.path.segments.last().is_some_and(|segment| segment.ident == wrapper)
}

/// Returns whether the type is the unit type `()`, which marks a generator with no primary input.
fn is_unit_type(ty: &Type) -> bool {
	matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn validate_min_max(parsed: &ParsedNodeFn) {
	for field in &parsed.fields {
		if let Some(RegularParsedField {
			number_hard_max,
			number_hard_min,
			number_soft_max,
			number_soft_min,
			..
		}) = field.ty.regular()
		{
			let pat_ident = &field.pat_ident;
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
		if let Some(RegularParsedField {
			number_mode_range: true,
			number_soft_min,
			number_soft_max,
			number_hard_min,
			number_hard_max,
			..
		}) = field.ty.regular()
		{
			let pat_ident = &field.pat_ident;
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
	if let Some(field) = parsed.fields.first()
		&& let Some(RegularParsedField { exposed: true, .. }) = field.ty.regular()
	{
		let pat_ident = &field.pat_ident;
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
				value => {
					let RegularParsedField { ty, implementations, .. } = value.regular().expect("a non-node field is a value field");
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::parsing::parse_node_fn;
	use proc_macro2::TokenStream;

	fn violations(attr: TokenStream, input: TokenStream) -> Vec<String> {
		let parsed = parse_node_fn(attr, input).expect("The test node fn should parse");
		ranked_input_violations(&parsed).into_iter().map(|(_, message)| message).collect()
	}

	#[test]
	fn a_bare_concrete_parameter_is_rejected() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn scale(_: impl Ctx, content: Item<Vector>, factor: f64) -> Item<Vector> {
					content
				}
			),
		);
		assert_eq!(messages.len(), 1, "{messages:?}");
		assert!(messages[0].contains("of type `f64` must be ranked"), "{messages:?}");
	}

	#[test]
	fn ranked_parameters_and_the_unit_primary_sentinel_pass() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn circle(_: impl Ctx, _primary: (), radius: Item<f64>, points: List<DVec2>, erased: ListDyn) -> Item<Vector> {
					Item::default()
				}
			),
		);
		assert_eq!(messages, Vec::<String>::new());
	}

	#[test]
	fn a_unit_typed_non_primary_parameter_is_rejected() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn weird(_: impl Ctx, content: Item<Vector>, marker: ()) -> Item<Vector> {
					content
				}
			),
		);
		assert_eq!(messages.len(), 1, "{messages:?}");
		assert!(messages[0].contains("Parameter `marker` cannot be typed `()`"), "{messages:?}");
	}

	#[test]
	fn a_bare_generic_parameter_with_ranked_rows_passes() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn to_thing<T>(_: impl Ctx, #[implementations(List<Graphic>, List<Vector>, ListDyn)] content: T) -> Item<Graphic> {
					Item::default()
				}
			),
		);
		assert_eq!(messages, Vec::<String>::new());
	}

	#[test]
	fn a_bare_generic_parameter_with_a_bare_row_is_rejected() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn to_thing<T>(_: impl Ctx, #[implementations(List<Graphic>, f64)] content: T) -> Item<Graphic> {
					Item::default()
				}
			),
		);
		assert_eq!(messages.len(), 1, "{messages:?}");
		assert!(messages[0].contains("row `f64`"), "{messages:?}");
	}

	#[test]
	fn an_item_declared_parameter_with_bare_element_rows_passes() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn blend<T>(_: impl Ctx, #[implementations(Graphic, Vector)] content: Item<T>, mode: Item<f64>) -> Item<T> {
					content
				}
			),
		);
		assert_eq!(messages, Vec::<String>::new());
	}

	#[test]
	fn a_lazy_input_with_a_bare_output_is_rejected() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn lazy_thing(_: impl Ctx, content: Item<Vector>, source: impl Node<Context, Output = f64>) -> Item<Vector> {
					content
				}
			),
		);
		assert_eq!(messages.len(), 1, "{messages:?}");
		assert!(messages[0].contains("Lazy input `source` with output type `f64` must be ranked"), "{messages:?}");
	}

	#[test]
	fn a_lazy_generic_output_with_ranked_rows_passes() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn cache<T>(_: impl Ctx, #[implementations(Context -> Item<f64>, Context -> ListDyn)] value: impl Node<Context, Output = T>) -> T {
					T::default()
				}
			),
		);
		assert_eq!(messages, Vec::<String>::new());
	}

	#[test]
	fn a_lazy_unit_row_is_rejected() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn cache<T>(_: impl Ctx, #[implementations(Context -> (), Context -> Item<f64>)] value: impl Node<Context, Output = T>) -> T {
					T::default()
				}
			),
		);
		assert_eq!(messages.len(), 1, "{messages:?}");
		assert!(messages[0].contains("row output `()` of the lazy input `value` must be ranked"), "{messages:?}");
	}

	#[test]
	fn a_lazy_generic_output_with_a_bare_row_is_rejected() {
		let messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn cache<T>(_: impl Ctx, #[implementations(Context -> Item<f64>, Context -> f64)] value: impl Node<Context, Output = T>) -> T {
					T::default()
				}
			),
		);
		assert_eq!(messages.len(), 1, "{messages:?}");
		assert!(messages[0].contains("row output `f64` of the lazy input `value` must be ranked"), "{messages:?}");
	}

	#[test]
	fn skip_impl_nodes_and_data_fields_are_exempt() {
		let skip_impl_messages = violations(
			quote::quote!(category(""), skip_impl),
			quote::quote!(
				fn passthrough<T>(_: impl Ctx, content: T) -> T {
					content
				}
			),
		);
		assert_eq!(skip_impl_messages, Vec::<String>::new());

		let data_field_messages = violations(
			quote::quote!(category("Test")),
			quote::quote!(
				fn stateful(_: impl Ctx, content: Item<Vector>, #[data] cache: f64) -> Item<Vector> {
					content
				}
			),
		);
		assert_eq!(data_field_messages, Vec::<String>::new());
	}
}
