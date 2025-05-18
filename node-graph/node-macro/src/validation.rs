use crate::parsing::{Implementation, ParsedField, ParsedNodeFn};
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
	];

	for validator in validators {
		validator(parsed);
	}

	Ok(())
}

fn validate_min_max(parsed: &ParsedNodeFn) {
	for field in &parsed.fields {
		if let ParsedField::Regular {
			number_hard_max,
			number_hard_min,
			number_soft_max,
			number_soft_min,
			pat_ident,
			..
		} = field
		{
			if let (Some(soft_min), Some(hard_min)) = (number_soft_min, number_hard_min) {
				let soft_min_value: f64 = soft_min.base10_parse().unwrap_or_default();
				let hard_min_value: f64 = hard_min.base10_parse().unwrap_or_default();
				if soft_min_value == hard_min_value {
					emit_error!(
						pat_ident.span(),
						"Unnecessary #[soft_min] attribute on `{}`, as #[hard_min] has the same value.",
						pat_ident.ident;
						help = "You can safely remove the #[soft_min] attribute from this field.";
						note = "#[soft_min] is redundant when it equals #[hard_min].",
					);
				} else if soft_min_value < hard_min_value {
					emit_error!(
						pat_ident.span(),
						"The #[soft_min] attribute on `{}` is incorrectly greater than #[hard_min].",
						pat_ident.ident;
						help = "You probably meant to reverse the two attribute values.";
						note = "Allowing the possible slider range to preceed #[hard_min] doesn't make sense.",
					);
				}
			}

			if let (Some(soft_max), Some(hard_max)) = (number_soft_max, number_hard_max) {
				let soft_max_value: f64 = soft_max.base10_parse().unwrap_or_default();
				let hard_max_value: f64 = hard_max.base10_parse().unwrap_or_default();
				if soft_max_value == hard_max_value {
					emit_error!(
						pat_ident.span(),
						"Unnecessary #[soft_max] attribute on `{}`, as #[hard_max] has the same value.",
						pat_ident.ident;
						help = "You can safely remove the #[soft_max] attribute from this field.";
						note = "#[soft_max] is redundant when it equals #[hard_max].",
					);
				} else if soft_max_value < hard_max_value {
					emit_error!(
						pat_ident.span(),
						"The #[soft_max] attribute on `{}` is incorrectly greater than #[hard_max].",
						pat_ident.ident;
						help = "You probably meant to reverse the two attribute values.";
						note = "Allowing the possible slider range to exceed #[hard_max] doesn't make sense.",
					);
				}
			}
		}
	}
}

fn validate_primary_input_expose(parsed: &ParsedNodeFn) {
	if let Some(ParsedField::Regular { exposed: true, pat_ident, .. }) = parsed.fields.first() {
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
			match field {
				ParsedField::Regular { ty, implementations, pat_ident, .. } => {
					if contains_generic_param(ty, &parsed.fn_generics) && implementations.is_empty() {
						emit_error!(
							ty.span(),
							"Generic type `{}` in field `{}` requires an #[implementations(...)] attribute",
							quote!(#ty),
							pat_ident.ident;
							help = "Add #[implementations(ConcreteType1, ConcreteType2)] to field '{}'", pat_ident.ident;
							help = "Or use #[skip_impl] if you want to manually implement the node"
						);
					}
				}
				ParsedField::Node {
					input_type,
					output_type,
					implementations,
					pat_ident,
					..
				} => {
					if (contains_generic_param(input_type, &parsed.fn_generics) || contains_generic_param(output_type, &parsed.fn_generics)) && implementations.is_empty() {
						emit_error!(
							pat_ident.span(),
							"Generic types in Node field `{}` require an #[implementations(...)] attribute",
							pat_ident.ident;
							help = "Add #[implementations(InputType1 -> OutputType1, InputType2 -> OutputType2)] to field '{}'", pat_ident.ident;
							help = "Or use #[skip_impl] if you want to manually implement the node"
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
