use crate::crate_ident::CrateIdent;
use crate::parsing::*;
use crate::shader_nodes::{ShaderCodegen, ShaderTokens};
use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use std::sync::atomic::AtomicU64;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;
use syn::{GenericArgument, GenericParam, Ident, Lifetime, PatIdent, PathArguments, Type, TypeParam, TypeParamBound};

pub(crate) mod classify;
mod entries;
pub(crate) mod ir;
mod metadata;
pub(crate) use classify::*;
use entries::entries_tokens;
use ir::{LazyBinding, ValueBinding};
use metadata::generate_node_input_references;

static NODE_ID: AtomicU64 = AtomicU64::new(0);

/// Binds in evaluation order with the lazy wrappers last: a wrapper takes the
/// free space as it stands when it is built, so every input whose record the
/// kernel still holds must have claimed its frame by then.
fn lazy_last<'a>(binds: impl Iterator<Item = (&'a &'a ParsedField, TokenStream2)>, lazy_entry: &TokenStream2) -> Vec<TokenStream2> {
	let mut ordered: Vec<(bool, TokenStream2)> = binds.map(|(field, body)| (matches!(field.ty, ParsedFieldType::Node(_)), body)).collect();
	ordered.sort_by_key(|(lazy, _)| *lazy);
	let mut declared = false;
	ordered
		.into_iter()
		.map(|(lazy, body)| match lazy && !std::mem::replace(&mut declared, true) {
			true => quote!(#lazy_entry #body),
			false => body,
		})
		.collect()
}

/// The regular inputs that materialize whole in the eval prologue, which is
/// where the per-node batch cache slots attach.
fn materialized_indices(regular_fields: &[&ParsedField], node: &ir::Node) -> Vec<usize> {
	regular_fields
		.iter()
		.enumerate()
		.filter(|(index, field)| matches!(field.ty, ParsedFieldType::Regular(_)) && matches!(ir::value_binding(node, *index), ValueBinding::Materialized))
		.map(|(index, _)| index)
		.collect()
}

pub(crate) fn generate_node_code(crate_ident: &CrateIdent, parsed: &ParsedNodeFn) -> syn::Result<TokenStream2> {
	let ParsedNodeFn {
		attributes,
		fn_name,
		struct_name,
		mod_name,
		fn_generics,
		input,
		output_type,
		fields,
		description,
		..
	} = parsed;
	let core_types = crate_ident.gcore()?;

	let category = attributes
		.category
		.as_ref()
		.expect("The 'category' attribute is required and should be checked during parsing, but was not found during codegen");
	let mod_name = format_ident!("_{}_mod", mod_name);

	let display_name = match &attributes.display_name.as_ref() {
		Some(lit) => lit.value(),
		None => struct_name.to_string().to_case(Case::Title),
	};
	let struct_name = format_ident!("{}Node", struct_name);

	// Separate data fields from regular fields
	let (data_fields, regular_fields): (Vec<_>, Vec<_>) = fields.iter().partition(|f| f.is_data_field);

	let model = analyze(parsed);
	let node = crate::codegen::ir::build(parsed);
	let kind = model.as_ref().map(|_| crate::codegen::ir::node_kind(&node));
	let carrier_present = matches!(node.inputs.first(), Some(input) if input.subject && crate::codegen::ir::materialized_levels(&node, 0) == 0);
	let record_io = matches!(kind, Some(crate::codegen::ir::NodeKind::RecordIo));
	let flip = matches!(kind, Some(crate::codegen::ir::NodeKind::Flip));
	let carrier_flip = flip && carrier_present;
	let opaque = matches!(kind, Some(crate::codegen::ir::NodeKind::Opaque));
	let routing_generic = match (kind, &node.output.shape.element) {
		(Some(crate::codegen::ir::NodeKind::Routing), crate::codegen::ir::Element::Generic(ident)) => Some(ident.clone()),
		_ => None,
	};
	// A `_: ()` primary keeps its slot: dropping it would shift every
	// per-index classification against the IR and the document's arity.
	let record_skips_carrier = record_io && !carrier_present;
	// A gather carrier copies the returned lane's frame, so it needs the plan
	// without a carrier layout of its own.
	let gather_carrier = record_io && node.output.gathers;
	let struct_regular_fields: Vec<_> = regular_fields.to_vec();
	let struct_regular_field_names: Vec<_> = struct_regular_fields.iter().map(|f| &f.pat_ident.ident).collect();

	// Extract function generics used by data fields
	let data_field_generics: Vec<_> = fn_generics
		.iter()
		.filter(|generic| {
			let generic_ident = match generic {
				syn::GenericParam::Type(type_param) => &type_param.ident,
				_ => return false,
			};

			// Check if this generic is used in any data field type
			data_fields.iter().any(|field| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => type_contains_ident(ty, generic_ident),
				_ => false,
			})
		})
		.cloned()
		.collect();

	// Node generics for regular fields (Node0, Node1, ...)
	let node_generics: Vec<Ident> = struct_regular_fields.iter().enumerate().map(|(i, _)| format_ident!("Node{}", i)).collect();

	// Extract just the idents from data_field_generics for struct type parameters
	let data_field_generic_idents: Vec<Ident> = data_field_generics
		.iter()
		.filter_map(|gp| match gp {
			syn::GenericParam::Type(tp) => Some(tp.ident.clone()),
			_ => None,
		})
		.collect();

	// Flipped kernels, ranked-input element generics, and the generics a
	// record-io node's secondary inputs name must be carried as struct parameters.
	let ctx_ident_for_flip = context_param(parsed).map(|ctx| ctx.ident.clone());
	let carries_generic = |ident: &Ident| {
		regular_fields.iter().enumerate().any(|(index, field)| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, list_levels, .. }) => (*list_levels > 0 || (record_io && index > 0)) && type_contains_ident(ty, ident),
			_ => false,
		})
	};
	let carried_generics: Vec<&syn::GenericParam> = fn_generics
		.iter()
		.filter(|param| match param {
			syn::GenericParam::Type(tp) => Some(&tp.ident) != ctx_ident_for_flip.as_ref() && !data_field_generic_idents.contains(&tp.ident) && (flip || carries_generic(&tp.ident)),
			_ => false,
		})
		.collect();
	let carried_generic_idents: Vec<Ident> = carried_generics
		.iter()
		.filter_map(|param| match param {
			syn::GenericParam::Type(tp) => Some(tp.ident.clone()),
			_ => None,
		})
		.collect();

	// Combined struct type parameters: data field generic idents (T, U, ...) + node generics (Node0, Node1, ...)
	// For struct type instantiation: MemoizeNode<T, Node0>
	let struct_type_params: Vec<Ident> = data_field_generic_idents
		.iter()
		.cloned()
		.chain(node_generics.iter().cloned())
		.chain(carried_generic_idents.iter().cloned())
		.collect();

	// Combined struct generic parameters with bounds for struct definition
	// struct MemoizeNode<T: Clone, Node0>
	let struct_generic_params: Vec<TokenStream2> = data_field_generics
		.iter()
		.map(|gp| quote!(#gp))
		.chain(node_generics.iter().map(|id| quote!(#id)))
		.chain(carried_generics.iter().map(|gp| quote!(#gp)))
		.collect();
	let context_features = &input.context_features;

	// Regular field idents and names (for function parameters)
	let field_idents: Vec<_> = regular_fields.iter().map(|f| &f.pat_ident).collect();
	let regular_field_names: Vec<_> = regular_fields.iter().map(|f| &f.pat_ident.ident).collect();
	let data_field_names: Vec<_> = data_fields.iter().map(|f| &f.pat_ident.ident).collect();

	// Only regular fields have input names/descriptions (for UI)
	let input_names: Vec<_> = regular_fields
		.iter()
		.map(|f| &f.name)
		.zip(regular_field_names.iter())
		.map(|zipped| match zipped {
			(Some(name), _) => name.value(),
			(_, name) => name.to_string().to_case(Case::Title),
		})
		.collect();

	let input_hidden = regular_field_names.iter().map(|name| name.to_string().starts_with('_')).collect::<Vec<_>>();

	let input_descriptions: Vec<_> = regular_fields.iter().map(|f| &f.description).collect();

	let subject_depth = node.inputs.iter().find(|input| input.subject).map_or(0, |input| input.shape.depth);
	let pushed_levels = (node.output.shape.depth as i8 - subject_depth as i8).max(0) as u8;
	let field_pushed_levels: Vec<u8> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { list_levels, .. }) if *list_levels > 0 => *list_levels as u8,
			ParsedFieldType::Node(_) => pushed_levels,
			_ => 0,
		})
		.collect();

	// Generate struct fields: data fields (concrete types) + regular fields (generic types)
	let data_field_defs = data_fields.iter().map(|field| {
		let name = &field.pat_ident.ident;
		let ty = match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
			_ => unreachable!("Data fields must be Regular types, not Node types"),
		};
		quote! { pub(super) #name: #ty }
	});

	let regular_field_defs = struct_regular_field_names.iter().zip(node_generics.iter()).map(|(name, r#gen)| {
		quote! { pub(super) #name: #r#gen }
	});

	let record_state_fields: Vec<TokenStream2> = if record_io {
		let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout)];
		if !record_skips_carrier {
			state.push(quote!(pub(super) __carrier: gcore::record::Layout));
		}
		if !record_skips_carrier || gather_carrier {
			state.push(quote!(pub(super) __plan: ::std::vec::Vec<(usize, usize, usize)>));
		}
		state.push(quote!(pub(super) __frame_bytes: usize));
		state.push(quote!(pub(super) __lane_invariant: u32));
		state.extend(reading_secondary_indices(&struct_regular_fields, record_skips_carrier).into_iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(pub(super) #slot: gcore::record::Layout)
		}));
		state.extend(crate::codegen::ir::element_lazy_indices(&struct_regular_fields, &node).into_iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(pub(super) #slot: gcore::record::Layout)
		}));
		let total_reads: usize = struct_regular_fields.iter().map(|field| field.attribute_reads.len()).sum();
		state.extend((0..total_reads).map(|index| {
			let slot = format_ident!("__read_{index}");
			quote!(pub(super) #slot: Option<usize>)
		}));
		state.extend((0..node.output.shape.attrs.len()).map(|index| {
			let slot = format_ident!("__write_{index}");
			quote!(pub(super) #slot: usize)
		}));
		state
	} else if routing_generic.is_some() {
		let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout), quote!(pub(super) __lane_invariant: u32)];
		state.extend(
			routing_value_indices(&struct_regular_fields, routing_generic.as_ref().expect("guarded by the arm"))
				.into_iter()
				.map(|index| {
					let slot = format_ident!("__in_{index}");
					quote!(pub(super) #slot: gcore::record::Layout)
				}),
		);
		state
	} else if opaque {
		vec![quote!(pub(super) __layout: gcore::record::Layout)]
	} else if flip {
		let mut state = vec![quote!(pub(super) __layout: gcore::record::Layout), quote!(pub(super) __frame_bytes: usize)];
		if carrier_flip {
			state.push(quote!(pub(super) __plan: ::std::vec::Vec<(usize, usize, usize)>));
		}
		state.extend((0..struct_regular_fields.len()).map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(pub(super) #slot: gcore::record::Layout)
		}));
		state.extend(lazy_read_fields(&struct_regular_fields).into_iter().map(|(index, field)| {
			let slot = format_ident!("__reads_{index}");
			let arity = field.attribute_reads.len();
			quote!(pub(super) #slot: [Option<usize>; #arity])
		}));
		state
	} else {
		Vec::new()
	};
	let mut record_state_fields = record_state_fields;
	if !carried_generic_idents.is_empty() {
		record_state_fields.push(quote!(pub(super) __marker: ::core::marker::PhantomData<fn() -> (#(#carried_generic_idents,)*)>));
	}
	// One slot per materialized input: the batch of the frame that
	// materialized it, keyed by (lane-normalized context, generation), so
	// per-lane evals share one materialization.
	record_state_fields.extend(materialized_indices(&struct_regular_fields, &node).into_iter().map(|index| {
		let slot = format_ident!("__mat_cache_{index}");
		quote!(pub(super) #slot: ::std::sync::Arc<::std::sync::Mutex<::core::option::Option<(u64, #core_types::record::MaterializedSpan)>>>)
	}));

	let async_source = parsed.injects_async_source_fields();
	let slot_value_type = crate::codegen::classify::substitute_lifetimes(&crate::codegen::classify::slot_static_type(output_type), "'static");
	let slot_field = async_source
		.then(|| quote! { pub(super) slot: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<u64, Option<gcore::gpoll::GPoll<#slot_value_type>>>>> })
		.into_iter();
	let struct_fields = data_field_defs.chain(regular_field_defs).chain(record_state_fields.iter().cloned()).chain(slot_field);

	// Only regular fields have UI metadata (data fields are internal state)
	let widget_override: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.widget_override {
			ParsedWidgetOverride::None => quote!(RegistryWidgetOverride::None),
			ParsedWidgetOverride::Hidden => quote!(RegistryWidgetOverride::Hidden),
			ParsedWidgetOverride::String(lit_str) => quote!(RegistryWidgetOverride::String(#lit_str)),
			ParsedWidgetOverride::Custom(lit_str) => quote!(RegistryWidgetOverride::Custom(#lit_str)),
		})
		.collect();

	let value_sources: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { value_source, .. }) => match value_source {
				ParsedValueSource::Default(data) => {
					// Check if the data is a string literal by parsing the token stream
					let data_str = data.to_string();
					if data_str.starts_with('"') && data_str.ends_with('"') && data_str.len() >= 2 {
						quote!(RegistryValueSource::Default(#data))
					} else {
						quote!(RegistryValueSource::Default(stringify!(#data)))
					}
				}
				ParsedValueSource::Scope(data) => {
					if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(_), .. }) = &**data {
						quote!(RegistryValueSource::Scope(#data))
					} else {
						quote!(RegistryValueSource::Scope(#data.as_static_str()))
					}
				}
				ParsedValueSource::SourceId => quote!(RegistryValueSource::SourceId),
				_ => quote!(RegistryValueSource::None),
			},
			_ => quote!(RegistryValueSource::None),
		})
		.collect();

	let default_types: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { implementations, .. }) => match implementations.first() {
				Some(ty) => quote!(Some(concrete!(#ty))),
				_ => quote!(None),
			},
			_ => quote!(None),
		})
		.collect();

	let bound_values = |select: fn(&RegularParsedField) -> &Option<NumberBound>| -> Vec<_> {
		regular_fields
			.iter()
			.map(|field| match &field.ty {
				ParsedFieldType::Regular(regular) => select(regular).as_ref().map_or(quote!(None), |bound| quote!(Some(#bound))),
				_ => quote!(None),
			})
			.collect()
	};
	let number_soft_min_values = bound_values(|field| &field.number_soft_min);
	let number_soft_max_values = bound_values(|field| &field.number_soft_max);
	let number_hard_min_values = bound_values(|field| &field.number_hard_min);
	let number_hard_max_values = bound_values(|field| &field.number_hard_max);
	let number_mode_range_values: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { number_mode_range, .. }) => quote!(#number_mode_range),
			_ => quote!(false),
		})
		.collect();
	let number_display_decimal_places: Vec<_> = regular_fields
		.iter()
		.map(|field| field.number_display_decimal_places.as_ref().map_or(quote!(None), |i| quote!(Some(#i))))
		.collect();
	let number_step: Vec<_> = regular_fields.iter().map(|field| field.number_step.as_ref().map_or(quote!(None), |i| quote!(Some(#i)))).collect();

	let unit_suffix: Vec<_> = regular_fields.iter().map(|field| field.unit.as_ref().map_or(quote!(None), |i| quote!(Some(#i)))).collect();

	let exposed: Vec<_> = regular_fields
		.iter()
		.map(|field| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { exposed, .. }) => quote!(#exposed),
			_ => quote!(true),
		})
		.collect();

	// Only eval regular fields (data fields are accessed directly as self.field_name)
	let all_implementation_types = fields.iter().flat_map(|field| match &field.ty {
		ParsedFieldType::Regular(RegularParsedField { implementations, .. }) => implementations.iter().cloned().collect::<Vec<_>>(),
		ParsedFieldType::Node(NodeParsedField { implementations, .. }) => implementations
			.iter()
			.flat_map(|implementation| [implementation.input.clone(), implementation.output.clone()])
			.collect(),
	});
	let all_implementation_types = all_implementation_types.chain(input.implementations.iter().cloned());

	// Only regular fields are parameters to new()
	let new_args = node_generics.iter().zip(struct_regular_field_names.iter()).map(|(r#gen, name)| {
		quote! { #name: #r#gen }
	});

	// Initialize data fields with Default, regular fields with parameters
	let data_inits = data_field_names.iter().map(|name| {
		quote! { #name: Default::default() }
	});
	let regular_inits = struct_regular_field_names.iter().map(|name| {
		quote! { #name }
	});
	let slot_init = async_source.then(|| quote! { slot: Default::default() }).into_iter();
	let all_field_inits = data_inits.chain(regular_inits).chain(slot_init);

	// Data fields may not implement Copy, PartialEq, etc., so only derive Debug and Clone
	let struct_derives = if record_io || routing_generic.is_some() || flip {
		quote!(#[derive(Debug, Clone)])
	} else if data_fields.is_empty() && !async_source {
		quote!(#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)])
	} else {
		quote!(#[derive(Debug, Clone)])
	};

	let identifier = format_ident!("{}_proto_ident", fn_name);
	let identifier_path = match parsed.attributes.path.as_ref() {
		Some(path) => {
			let path = path.to_token_stream().to_string().replace(' ', "");
			quote!(#path)
		}
		None => quote!(std::module_path!()),
	};

	let registry_name = format_ident!("__node_registry_{}_{}", NODE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst), struct_name);
	let register_node_impl = quote! {
		#[cfg(target_family = "wasm")]
		#[unsafe(no_mangle)]
		extern "C" fn #registry_name() {
			register_metadata();
		}
	};
	// Record nodes construct through the generated `wire` fn, which resolves
	// offsets from the carrier layout; `new` cannot fill that state.
	let routing_layout_param = (routing_generic.is_some() || opaque).then(|| quote!(__layout: &gcore::record::Layout,)).into_iter();
	let routing_layout_init = (routing_generic.is_some() || opaque).then(|| quote!(__layout: __layout.clone(),)).into_iter();
	// The lane-invariance mask arrives with the resolved layout, so `new` starts
	// from the safe empty mask.
	let routing_invariant_init = routing_generic.is_some().then(|| quote!(__lane_invariant: 0,)).into_iter();
	let routing_value_layouts: Vec<usize> = routing_generic.as_ref().map(|generic| routing_value_indices(&struct_regular_fields, generic)).unwrap_or_default();
	let routing_in_params = routing_value_layouts.iter().map(|index| {
		let slot = format_ident!("__in_{index}");
		quote!(#slot: &gcore::record::Layout,)
	});
	let routing_in_inits = routing_value_layouts.iter().map(|index| {
		let slot = format_ident!("__in_{index}");
		quote!(#slot: #slot.clone(),)
	});
	let flip_layout_params = flip
		.then(|| {
			(0..struct_regular_fields.len()).map(|index| {
				let slot = format_ident!("__in_{index}");
				quote!(#slot: &gcore::record::Layout,)
			})
		})
		.into_iter()
		.flatten();
	let flip_layout_inits = flip
		.then(|| {
			(0..struct_regular_fields.len()).map(|index| {
				let slot = format_ident!("__in_{index}");
				quote!(#slot: #slot.clone(),)
			})
		})
		.into_iter()
		.flatten();
	// The output layout, frame size, and copy plan are installed by `set_layout`.
	let flip_read_bindings = flip
		.then(|| {
			lazy_read_fields(&struct_regular_fields).into_iter().map(|(index, field)| {
				let arr = format_ident!("__reads_{index}");
				let slot = format_ident!("__in_{index}");
				let offsets = field.attribute_reads.iter().map(|read| {
					let marker = &read.marker;
					quote!(#slot.offset_of(<#marker as gcore::attribute::Attribute>::NAME, 0))
				});
				quote!(let #arr = [#(#offsets),*];)
			})
		})
		.into_iter()
		.flatten();
	let flip_read_inits = flip
		.then(|| {
			lazy_read_fields(&struct_regular_fields).into_iter().map(|(index, _)| {
				let arr = format_ident!("__reads_{index}");
				quote!(#arr,)
			})
		})
		.into_iter()
		.flatten();
	let flip_output_inits = flip
		.then(|| {
			let plan = carrier_flip.then(|| quote!(__plan: ::std::vec::Vec::new(),));
			quote!(__layout: ::core::default::Default::default(), __frame_bytes: 0, #plan)
		})
		.into_iter();
	let marker_init = (!carried_generic_idents.is_empty()).then(|| quote!(__marker: ::core::marker::PhantomData,)).into_iter();
	let plain_mat_cache_inits: Vec<TokenStream2> = materialized_indices(&struct_regular_fields, &node)
		.into_iter()
		.map(|index| {
			let slot = format_ident!("__mat_cache_{index}");
			quote!(#slot: ::core::default::Default::default(),)
		})
		.collect();
	// `new` carries the bounds the erased glue needs at the output type.
	let new_where = flip
		.then(|| {
			let existing = parsed.where_clause.iter().flat_map(|clause| clause.predicates.iter());
			quote!(where #(#existing,)* #slot_value_type: ::core::clone::Clone + ::core::marker::Send + ::core::marker::Sync + 'static)
		})
		.into_iter();
	let new_impl = match !record_io {
		true => quote! {
			#[automatically_derived]
			impl<'n, #(#struct_generic_params,)*> #struct_name<#(#struct_type_params,)*> #(#new_where)*
			{
				#[allow(clippy::too_many_arguments)]
				pub fn new(#(#new_args,)* #(#routing_layout_param)* #(#routing_in_params)* #(#flip_layout_params)*) -> Self {
					#(#flip_read_bindings)*
					Self {
						#(#all_field_inits,)*
						#(#routing_layout_init)*
						#(#routing_invariant_init)*
						#(#routing_in_inits)*
						#(#flip_layout_inits)*
						#(#flip_read_inits)*
						#(#flip_output_inits)*
						#(#marker_init)*
						#(#plain_mat_cache_inits)*
					}
				}
			}
		},
		false => quote!(),
	};

	let import_name = format_ident!("_IMPORT_STUB_{}", mod_name.to_string().to_case(Case::UpperSnake));
	let mut plan = generate_node_impl(
		crate_ident,
		parsed,
		&model,
		NodeFields {
			data_fields: data_fields.clone(),
			regular_fields: struct_regular_fields.clone(),
			node_generics: node_generics.clone(),
			data_field_generic_idents: data_field_generic_idents.clone(),
			struct_type_params: struct_type_params.clone(),
		},
	)?;
	plan.struct_item = quote! {
		#struct_derives
		pub struct #struct_name<#(#struct_generic_params,)*> {
			#(#struct_fields,)*
		}
	};
	plan.value_ctor = new_impl;
	let NodePlan {
		struct_item,
		value_ctor,
		kernel,
		lazy_read_fns,
		record_ctor,
		node_impl,
		entries,
	} = plan;
	let entries_name = format_ident!("{}_entries", parsed.fn_name);
	let register_entries = match entries.is_empty() {
		true => quote!(),
		false => quote!(gcore::registry::NODE_REGISTRY.lock().unwrap().entry(#identifier()).or_default().extend(#entries_name());),
	};

	let properties = &attributes.properties_string.as_ref().map(|value| quote!(Some(#value))).unwrap_or(quote!(None));
	let memoize_flag = attributes.memoize;
	let inject_scope_flag = attributes.inject_scope;

	let cfg = crate::shader_nodes::modify_cfg(attributes);
	let node_input_accessor = generate_node_input_references(parsed, fn_generics, &field_idents, core_types, &identifier, &cfg);
	let ShaderTokens { shader_entry_point, gpu_node } = attributes.shader_node.as_ref().map(|n| n.codegen(crate_ident, parsed)).unwrap_or(Ok(ShaderTokens::default()))?;

	let display_name_header = format!("# {display_name}");
	let mut description_doc_attrs = vec![quote!(#[doc = #display_name_header]), quote!(#[doc = ""])];
	description_doc_attrs.extend(description.lines().map(|line| quote!(#[doc = #line])));

	// Add parameter list to doc comment
	if !input_names.is_empty() {
		description_doc_attrs.push(quote!(#[doc = ""]));
		description_doc_attrs.push(quote!(#[doc = "## Parameters"]));
		for (name, desc) in input_names.iter().zip(input_descriptions.iter()) {
			if desc.is_empty() {
				let header = format!("- **{name}**");
				description_doc_attrs.push(quote!(#[doc = #header]));
			} else {
				let first_line = desc.lines().next().unwrap_or("");
				let header = format!("- **{name}**: {first_line}");
				description_doc_attrs.push(quote!(#[doc = #header]));
				for line in desc.lines().skip(1) {
					let continuation = format!("  {line}");
					description_doc_attrs.push(quote!(#[doc = #continuation]));
				}
			}
		}
	}

	Ok(quote! {
		#(#description_doc_attrs)*
		#kernel

		#lazy_read_fns

		#record_ctor

		#node_impl

		#cfg
		const fn #identifier() -> #core_types::ProtoNodeIdentifier {
			#core_types::ProtoNodeIdentifier::new(std::concat!(#identifier_path, "::", std::stringify!(#struct_name)))
		}

		#cfg
		#[doc(inline)]
		pub use #mod_name::#struct_name;

		#[doc(hidden)]
		#node_input_accessor

		#cfg
		#[doc(hidden)]
		#[allow(clippy::module_inception)]
		mod #mod_name {
			use super::*;
			use #core_types as gcore;
			use gcore::{ContextFeature, concrete};
			use gcore::registry::{NodeMetadata, FieldMetadata, NODE_METADATA, RegistryValueSource, RegistryWidgetOverride};
			use gcore::ctor::ctor;

			// Use the types specified in the implementation

			static #import_name: core::marker::PhantomData<(#(#all_implementation_types,)*)> = core::marker::PhantomData;

			#struct_item

			#value_ctor

			#entries

			#register_node_impl

			#[cfg_attr(not(target_family = "wasm"), ctor)]
			fn register_metadata() {
				let metadata = NodeMetadata {
					display_name: #display_name,
					category: #category,
					description: #description,
					properties: #properties,
					context_features: vec![#(ContextFeature::#context_features,)*],
					memoize: #memoize_flag,
					inject_scope: #inject_scope_flag,
					async_source_fields: #async_source,
					fields: vec![
						#(
							FieldMetadata {
								name: #input_names,
								widget_override: #widget_override,
								description: #input_descriptions,
								pushed_levels: #field_pushed_levels,
								hidden: #input_hidden,
								exposed: #exposed,
								value_source: #value_sources,
								default_type: #default_types,
								number_soft_min: #number_soft_min_values,
								number_soft_max: #number_soft_max_values,
								number_hard_min: #number_hard_min_values,
								number_hard_max: #number_hard_max_values,
								number_mode_range: #number_mode_range_values,
								number_display_decimal_places: #number_display_decimal_places,
								number_step: #number_step,
								unit: #unit_suffix,
							},
						)*
					],
				};
				NODE_METADATA.lock().unwrap().insert(#identifier(), metadata);
				#register_entries
			}
		}

		#shader_entry_point

		#gpu_node
	})
}

/// Check if a type contains a reference to a specific identifier (e.g., a generic type parameter)
pub(crate) fn type_contains_ident(ty: &Type, ident: &Ident) -> bool {
	struct IdentChecker<'a> {
		target: &'a Ident,
		found: bool,
	}

	impl<'a, 'ast> syn::visit::Visit<'ast> for IdentChecker<'a> {
		fn visit_ident(&mut self, i: &'ast Ident) {
			if i == self.target {
				self.found = true;
			}
		}
	}

	let mut checker = IdentChecker { target: ident, found: false };
	syn::visit::visit_type(&mut checker, ty);
	checker.found
}

/// The generated items of one node, produced uniformly across classes and
/// stitched by the assembling `quote!` at the end of [`generate_node_code`].
/// `struct_item` and `value_ctor` are filled by the caller; the rest come from
/// [`generate_node_impl`].
#[derive(Default)]
pub(crate) struct NodePlan {
	pub(crate) struct_item: TokenStream2,
	pub(crate) value_ctor: TokenStream2,
	pub(crate) kernel: TokenStream2,
	pub(crate) lazy_read_fns: TokenStream2,
	pub(crate) record_ctor: TokenStream2,
	pub(crate) node_impl: TokenStream2,
	pub(crate) entries: TokenStream2,
}

/// The field and generic derivation shared by the struct/metadata side and the
/// impl side, computed once in [`generate_node_code`] and passed to
/// [`generate_node_impl`]. `regular_fields` is the carrier-skipped slice both
/// sides agree on.
pub(crate) struct NodeFields<'a> {
	pub(crate) data_fields: Vec<&'a ParsedField>,
	pub(crate) regular_fields: Vec<&'a ParsedField>,
	pub(crate) node_generics: Vec<Ident>,
	pub(crate) data_field_generic_idents: Vec<Ident>,
	pub(crate) struct_type_params: Vec<Ident>,
}

/// Rewrites a creator kernel's `emit(a, b, ..)` tail into the row tuple `(a, b, ..)`.
/// `emit`'s parentheses double as the tuple constructor, so it is pure sugar.
fn rewrite_emit(body: &TokenStream2) -> TokenStream2 {
	let Ok(mut block) = syn::parse2::<syn::Block>(body.clone()) else {
		return body.clone();
	};
	struct EmitToTuple;
	impl VisitMut for EmitToTuple {
		fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
			if let syn::Expr::Call(call) = expr
				&& matches!(&*call.func, syn::Expr::Path(path) if path.path.is_ident("emit"))
			{
				*expr = syn::Expr::Tuple(syn::ExprTuple {
					attrs: Vec::new(),
					paren_token: Default::default(),
					elems: call.args.clone(),
				});
			}
			syn::visit_mut::visit_expr_mut(self, expr);
		}
	}
	EmitToTuple.visit_block_mut(&mut block);
	block.to_token_stream()
}

pub(crate) fn generate_node_impl(crate_ident: &CrateIdent, parsed: &ParsedNodeFn, model: &Option<Dialect>, fields: NodeFields) -> syn::Result<NodePlan> {
	let core_types = crate_ident.gcore()?;

	let ctx_param = context_param(parsed);
	let ctx_ident = match ctx_param {
		Some(ctx_param) => ctx_param.ident.clone(),
		None => format_ident!("__Ctx"),
	};
	let Some(model) = model.as_ref() else {
		return Ok(NodePlan::default());
	};
	let async_fn = matches!(*model, Dialect::AsyncFn);
	let future_kernel = matches!(*model, Dialect::Future | Dialect::FutureInterrupt);
	let async_source = async_fn || future_kernel;
	let node = crate::codegen::ir::build(parsed);
	let kind = crate::codegen::ir::node_kind(&node);
	let carrier_present = matches!(node.inputs.first(), Some(input) if input.subject && crate::codegen::ir::materialized_levels(&node, 0) == 0);
	let flip = matches!(kind, crate::codegen::ir::NodeKind::Flip);
	let carrier_flip = flip && carrier_present;
	let opaque = matches!(kind, crate::codegen::ir::NodeKind::Opaque);
	let record_io = matches!(kind, crate::codegen::ir::NodeKind::RecordIo);
	let routing_generic = match (kind, &node.output.shape.element) {
		(crate::codegen::ir::NodeKind::Routing, crate::codegen::ir::Element::Generic(ident)) => Some(ident.clone()),
		_ => None,
	};
	let skips_carrier = record_io && !carrier_present;
	// A gather carrier copies the returned lane's frame, so it needs the plan
	// without a carrier layout of its own.
	let gather_carrier = record_io && node.output.gathers;
	// A gathered element is carried by the copy plan, not as a lazy token, so
	// its generic stays a struct parameter.
	let record_token = match (kind, &node.output.shape.element) {
		(crate::codegen::ir::NodeKind::RecordIo, crate::codegen::ir::Element::Generic(ident)) if !gather_carrier => Some(ident.clone()),
		_ => None,
	};
	// The record-io write set, resolved from the output item and carrier input.
	let write_markers: Vec<&Type> = node.output.shape.attrs.iter().map(|attr| &attr.marker).collect();
	let removes: Vec<&Type> = node.output.removes.iter().map(|attr| &attr.marker).collect();
	// A gathered element rides the copy plan, never a write.
	let element_write: Option<&Type> = match &node.output.shape.element {
		crate::codegen::ir::Element::Concrete(ty) if !gather_carrier => Some(ty),
		_ => None,
	};
	let carrier_read_ty: Option<&Type> = node.inputs.first().filter(|input| input.subject).and_then(|input| match &input.shape.element {
		crate::codegen::ir::Element::Concrete(ty) => Some(ty),
		_ => None,
	});
	let subject_depth = node.inputs.iter().find(|input| input.subject).map_or(0, |input| input.shape.depth);
	let level_delta = node.output.shape.depth as i8 - subject_depth as i8;
	let pushed_levels = level_delta.max(0) as u8;
	let output_row = slot_value_type(&parsed.output_type);
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
	if async_source && !snapshot_ctx {
		ctx_bounds.push(quote!(#core_types::context::DeriveCtx));
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

	// The serving lifetime is quantified by each serving method, so the impl
	// never binds the context's arena; the kernel keeps its own bound.
	let extracts_arena = |bound: &TypeParamBound| matches!(bound, TypeParamBound::Trait(trait_bound) if trait_bound.path.segments.last().is_some_and(|segment| segment.ident == "ExtractArena"));
	let mut impl_ctx_bounds: Vec<TokenStream2> = match ctx_param {
		Some(ctx_param) => ctx_param
			.bounds
			.iter()
			.filter(|bound| !matches!(bound, TypeParamBound::Lifetime(_)) && !extracts_arena(bound))
			.map(|bound| quote!(#bound))
			.collect(),
		None => Vec::new(),
	};
	if ctx_param.is_none() {
		impl_ctx_bounds.push(quote!(#core_types::Ctx));
	}
	if async_source && !snapshot_ctx {
		impl_ctx_bounds.push(quote!(#core_types::context::DeriveCtx));
	}
	if snapshot_ctx {
		impl_ctx_bounds.extend([
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
	let derive_routing = derives && routing_generic.is_some();
	// A kernel that holds records (a forwarded routing wire, or a lazy edge it
	// serves itself) names the record lifetime; unless it declared a serving
	// lifetime of its own, the context binds the arena at that lifetime.
	let kernel_lazy = parsed.fields.iter().any(|field| !field.is_data_field && matches!(field.ty, ParsedFieldType::Node(_)));
	let wants_record_lifetime = routing_generic.is_some() || ((record_io || flip) && kernel_lazy);
	let ctx_declares_arena = ctx_param.is_some_and(|ctx_param| ctx_param.bounds.iter().any(extracts_arena));
	let bind_record_arena = wants_record_lifetime && !ctx_declares_arena;
	// The lifetime a lazy wrapper's frame space is named at: the kernel's own
	// arena lifetime, since the records it hands back live in that space.
	let declared_arena_lifetime = ctx_param.and_then(|ctx_param| {
		ctx_param.bounds.iter().find_map(|bound| match bound {
			TypeParamBound::Trait(trait_bound) if extracts_arena(bound) => match &trait_bound.path.segments.last().expect("checked by the predicate").arguments {
				PathArguments::AngleBracketed(args) => args.args.iter().find_map(|arg| match arg {
					GenericArgument::Lifetime(lifetime) => Some(lifetime.clone()),
					_ => None,
				}),
				_ => None,
			},
			_ => None,
		})
	});
	let frames_lifetime = match (&declared_arena_lifetime, wants_record_lifetime) {
		(Some(lifetime), _) => quote!(#lifetime),
		(None, true) => quote!('__record),
		(None, false) => quote!('_),
	};
	if bind_record_arena {
		ctx_bounds.push(quote!(#core_types::context::ExtractArena<ArenaRef = &'__record #core_types::arena::Arena>));
	}

	let ctx_generic = match ctx_bounds.is_empty() {
		true => quote!(#ctx_ident),
		false => quote!(#ctx_ident: #(#ctx_bounds)+*),
	};
	let impl_ctx_generic = match impl_ctx_bounds.is_empty() {
		true => quote!(#ctx_ident),
		false => quote!(#ctx_ident: #(#impl_ctx_bounds)+*),
	};
	let generic_tokens = |param: &GenericParam| match param {
		GenericParam::Type(type_param) if Some(&type_param.ident) == ctx_param.map(|ctx_param| &ctx_param.ident) => ctx_generic.clone(),
		param => quote!(#param),
	};
	let impl_generic_tokens = |param: &GenericParam| match param {
		GenericParam::Type(type_param) if Some(&type_param.ident) == ctx_param.map(|ctx_param| &ctx_param.ident) => impl_ctx_generic.clone(),
		param => quote!(#param),
	};
	let mut generics: Vec<TokenStream2> = parsed
		.fn_generics
		.iter()
		.filter(|param| match param {
			// A routing generic is the record itself, so the kernel names the
			// record value rather than carrying the parameter.
			GenericParam::Type(type_param) => Some(&type_param.ident) != routing_generic.as_ref(),
			_ => true,
		})
		.map(|param| match param {
			// Flipped kernels clone bare-typed elements out of their records,
			// so those generics carry the bound wire values satisfy; a
			// generic only nested in a field's type stays as declared.
			GenericParam::Type(type_param)
				if flip
					&& Some(&type_param.ident) != ctx_param.map(|ctx_param| &ctx_param.ident)
					&& parsed.fields.iter().filter(|field| !field.is_data_field).any(|field| {
						let ty = match &field.ty {
							ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
							ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
						};
						matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.get_ident() == Some(&type_param.ident))
					}) =>
			{
				let mut bounded = type_param.clone();
				bounded.bounds.push(syn::parse_quote!(::core::clone::Clone));
				quote!(#bounded)
			}
			param => generic_tokens(param),
		})
		.collect();
	let mut impl_generics: Vec<TokenStream2> = parsed
		.fn_generics
		.iter()
		.filter(|param| match param {
			GenericParam::Type(type_param) => Some(&type_param.ident) != routing_generic.as_ref() && Some(&type_param.ident) != record_token.as_ref(),
			// A serving lifetime is the serve method's own, so it never rides
			// the impl: an impl-level binding of the arena would contradict
			// the one the method quantifies over.
			GenericParam::Lifetime(_) => false,
			_ => true,
		})
		.map(&impl_generic_tokens)
		.collect();
	if ctx_param.is_none() {
		generics.push(ctx_generic.clone());
		impl_generics.push(impl_ctx_generic.clone());
	}
	let lazy_carrier = record_io && carrier_present && matches!(parsed.fields.iter().find(|field| !field.is_data_field).map(|field| &field.ty), Some(ParsedFieldType::Node(_)));

	let fn_name = &parsed.fn_name;
	let mod_name = format_ident!("_{}_mod", parsed.mod_name);
	let struct_name = format_ident!("{}Node", parsed.struct_name);
	let output_type = &parsed.output_type;
	let raw_lazy = matches!(*model, Dialect::Poll);
	let injected_name = |ident: &Ident| async_source && (ident == "_runtime" || ident == "_source");
	let where_predicates: Vec<TokenStream2> = parsed.where_clause.iter().flat_map(|clause| clause.predicates.iter()).map(|predicate| quote!(#predicate)).collect();

	let NodeFields {
		data_fields,
		regular_fields,
		node_generics,
		data_field_generic_idents,
		struct_type_params,
	} = fields;

	if derive_routing {
		for (index, field) in regular_fields.iter().enumerate() {
			let source_ty = match &field.ty {
				ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
			};
			if routing_generic.as_ref().is_some_and(|generic| crate::codegen::classify::routing_source_output(source_ty, generic)) {
				let source_generic = format_ident!("__Source{index}");
				generics.push(quote! {
					#source_generic: for<'__derived> #core_types::record::DerivedRecordInput<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>
				});
			}
		}
	}
	if lazy_carrier && derives {
		let source_generic = format_ident!("__Source0");
		generics.push(quote! {
			#source_generic: for<'__derived> #core_types::record::DerivedRecordInput<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>
		});
	}
	if flip {
		for (index, field) in regular_fields.iter().enumerate() {
			if matches!(&field.ty, ParsedFieldType::Node(_)) {
				let source_generic = format_ident!("__Source{index}");
				let derived_extra = derives
					.then(|| quote!(+ for<'__derived> #core_types::record::DerivedRecordInput<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>))
					.into_iter();
				generics.push(quote! {
					#source_generic: #core_types::node::Node<#ctx_ident> #(#derived_extra)*
				});
			}
		}
	}
	if record_io {
		for (index, field) in regular_fields.iter().enumerate() {
			if matches!(&field.ty, ParsedFieldType::Node(_)) && matches!(crate::codegen::ir::lazy_binding(&node, index), LazyBinding::Element) {
				let source_generic = format_ident!("__Source{index}");
				let derived_extra = derives
					.then(|| quote!(+ for<'__derived> #core_types::record::DerivedRecordInput<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>))
					.into_iter();
				generics.push(quote! {
					#source_generic: #core_types::node::Node<#ctx_ident> #(#derived_extra)*
				});
			}
		}
	}
	if opaque {
		for (index, field) in regular_fields.iter().enumerate() {
			if matches!(&field.ty, ParsedFieldType::Node(_)) {
				let source_generic = format_ident!("__Source{index}");
				generics.push(quote!(#source_generic: #core_types::node::Node<#ctx_ident>));
			}
		}
	}
	// The record lifetime the kernel's wire types and arena bound name; the
	// impl infers it from the serving lifetime at every call.
	if wants_record_lifetime {
		generics.insert(0, quote!('__record));
	}

	let data_names: Vec<&Ident> = data_fields.iter().map(|field| &field.pat_ident.ident).collect();
	let data_params = data_fields.iter().map(|field| {
		let pat = &field.pat_ident;
		let ParsedFieldType::Regular(RegularParsedField { ty, .. }) = &field.ty else {
			unreachable!("data fields are regular types");
		};
		quote!(#pat: &#ty)
	});

	let derived_edge = quote!(for<'__derived> #core_types::record::DerivedRecordInput<'__derived, #core_types::context::Derived<'__derived, #ctx_ident>>);
	let lazy_bound = || match derives {
		true => {
			let derived_edge = derived_edge.clone();
			quote!(#core_types::node::Node<#ctx_ident> + #derived_edge)
		}
		false => quote!(#core_types::node::Node<#ctx_ident>),
	};

	let routing_source = |ty: &Type| routing_generic.as_ref().is_some_and(|generic| crate::codegen::classify::routing_source_output(ty, generic));

	let lazy_read_out = |field: &ParsedField, output_type: &Type| {
		let attr_tys = field.attribute_reads.iter().map(|read| {
			let marker = &read.marker;
			quote!(#core_types::attribute::Attr<#marker>)
		});
		match field.attribute_reads.is_empty() {
			true => quote!(#output_type),
			false => quote!((#output_type #(, #attr_tys)*)),
		}
	};
	let read_tuple_param = |field: &ParsedField, value_param: TokenStream2, value_ty: TokenStream2| {
		let read_pats = field.attribute_reads.iter().map(|read| &read.pat_ident);
		let read_tys = field.attribute_reads.iter().map(|read| {
			let marker = &read.marker;
			quote!(#core_types::attribute::Attr<#marker>)
		});
		quote!((#value_param #(, #read_pats)*): (#value_ty #(, #read_tys)*))
	};
	let kernel_params = regular_fields.iter().enumerate().filter(|(_, field)| !injected_name(&field.pat_ident.ident)).map(|(index, field)| {
		let pat = &field.pat_ident;
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) if ir::materialized_levels(&node, index) > 0 => {
				// The gathered lane borrows this list, so both take the kernel's
				// own subject lifetime. An element type naming a serving
				// lifetime ties the view to the same region; a fn-declared
				// serving lifetime binds a generic subject's view, so the
				// output can borrow the materialized level.
				let declared = || {
					let mut lifetimes = parsed.fn_generics.iter().filter_map(|param| match param {
						GenericParam::Lifetime(lifetime_param) => Some(lifetime_param.lifetime.clone()),
						_ => None,
					});
					lifetimes
						.next()
						.filter(|_| lifetimes.next().is_none())
						.filter(|lifetime| match &node.output.shape.element {
							ir::Element::Concrete(element) => crate::codegen::classify::named_serving_lifetime(element).as_ref() == Some(lifetime),
							_ => false,
						})
						// An arena-bound lifetime serves the output from the
						// arena, not from the subject's batch view.
						.filter(|lifetime| !ctx_param.is_some_and(|ctx| quote!(#ctx).to_string().contains(&lifetime.to_string())))
				};
				match (crate::codegen::classify::named_serving_lifetime(ty).or_else(declared), ir::gathered_subject(&node) == Some(index)) {
					(Some(lifetime), _) => quote!(#pat: #core_types::node::List<#lifetime, #ty>),
					(None, true) => quote!(#pat: #core_types::node::List<'__lane, #ty>),
					(None, false) => quote!(#pat: #core_types::node::List<'_, #ty>),
				}
			}
			// A routing source is the forwarded record itself, not an element.
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) if routing_source(ty) => quote!(#pat: #core_types::record::RecordValue<'__record>),
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => quote!(#pat: &#ty),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) if !field.attribute_reads.is_empty() => read_tuple_param(field, quote!(#pat), quote!(#ty)),
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(#pat: #ty),
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => {
				let source_generic = format_ident!("__Source{index}");
				match (ir::lazy_binding(&node, index), raw_lazy) {
					(LazyBinding::DeriveRouting, _) => quote!(#pat: #core_types::record::RecordLazyInput<'_, #frames_lifetime, #source_generic>),
					(LazyBinding::DeriveCarrier, _) => {
						let out = lazy_read_out(field, output_type);
						quote!(#pat: #core_types::record::DerivedLazyInput<'_, #frames_lifetime, #out, #source_generic>)
					}
					(LazyBinding::OpaqueRecord, _) => quote!(#pat: &#core_types::record::RecordEdgeInput<'_, #frames_lifetime, #source_generic>),
					(LazyBinding::Element, true) => {
						let out = lazy_read_out(field, output_type);
						quote!(#pat: &#core_types::record::ElementEdge<'_, #frames_lifetime, #out, #source_generic>)
					}
					(LazyBinding::Element, false) => {
						let out = lazy_read_out(field, output_type);
						quote!(#pat: #core_types::record::ElementLazyInput<'_, #frames_lifetime, #out, #source_generic>)
					}
					(LazyBinding::Generic, true) => {
						let bound = lazy_bound();
						quote!(#pat: &impl #bound)
					}
					(LazyBinding::Generic, false) => {
						let bound = lazy_bound();
						quote!(#pat: #core_types::node::LazyInput<'_, #frames_lifetime, impl #bound>)
					}
				}
			}
		}
	});

	let node_bounds = regular_fields.iter().enumerate().zip(&node_generics).map(|((index, field), node_generic)| {
		let plain = quote!(#node_generic: #core_types::node::Node<#ctx_ident>);
		// A lazy edge the kernel evaluates at derived contexts needs the
		// derived form: the derived context's arena binding is unnameable
		// under a higher rank.
		let derived = quote!(#node_generic: #derived_edge);
		let derived_plus = quote! {
			#node_generic: #core_types::node::Node<#ctx_ident>,
			#node_generic: #derived_edge
		};
		match &field.ty {
			ParsedFieldType::Node(_) if flip => match derives {
				true => derived_plus,
				false => plain,
			},
			ParsedFieldType::Node(_) if record_io && !skips_carrier && index == 0 => match derives {
				true => derived,
				false => plain,
			},
			// An element-consuming lazy secondary rides a record edge, derivable
			// when the kernel evaluates it at derived contexts.
			ParsedFieldType::Node(_) if record_io && matches!(ir::lazy_binding(&node, index), LazyBinding::Element) => match derives {
				true => derived_plus,
				false => plain,
			},
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) if routing_source(output_type) => match derives {
				true => derived,
				false => plain,
			},
			ParsedFieldType::Node(_) if opaque => plain,
			ParsedFieldType::Node(_) => {
				let bound = lazy_bound();
				quote!(#node_generic: #bound)
			}
			// Every wire is a record edge; a value input's element copies out of
			// the record its edge serves.
			ParsedFieldType::Regular(_) => plain,
		}
	});

	let mut lend_outlives: Vec<TokenStream2> = Vec::new();
	if let Type::Reference(reference) = &slot_value_type(&parsed.output_type)
		&& let Some(lifetime) = &reference.lifetime
	{
		let inner = &reference.elem;
		lend_outlives.push(quote!(#inner: #lifetime));
	}

	// The slot persists the plain value even on record wires, so the Clone
	// bound targets the slot type, not the (possibly lifted) trait output.
	let slot_ty = crate::codegen::classify::slot_static_type(&parsed.output_type);
	let mut async_bounds = match (async_fn, future_kernel) {
		(false, false) => Vec::new(),
		(false, true) => vec![quote!(#slot_ty: Clone)],
		(true, _) => {
			let output_clone = std::iter::once(quote!(#slot_ty: Clone));
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
	if async_source {
		async_bounds.push(quote!(for<'__derived> #core_types::context::Derived<'__derived, #ctx_ident>: #core_types::CacheHash));
	}

	let clampable_bounds = regular_fields.iter().filter_map(|field| {
		let ParsedFieldType::Regular(RegularParsedField {
			ty, number_hard_min, number_hard_max, ..
		}) = &field.ty
		else {
			return None;
		};
		(number_hard_min.is_some() || number_hard_max.is_some()).then(|| quote!(#ty: #core_types::misc::Clampable))
	});

	let flat_reads = field_reads(&regular_fields);
	let read_binding = |slot: usize, read: &AttributeRead, rec: TokenStream2| {
		let pat = &read.pat_ident;
		let marker = &read.marker;
		let slot = format_ident!("__read_{slot}");
		quote! {
			let #pat = unsafe { #core_types::record::read_at::<#marker>(#rec, self.#slot) };
		}
	};
	let reads_of = |field_index: usize| {
		flat_reads
			.iter()
			.enumerate()
			.filter(move |(_, (owner, _))| *owner == field_index)
			.map(|(slot, (_, read))| (slot, *read))
			.collect::<Vec<(usize, &AttributeRead)>>()
	};

	let frame_entry = quote! {
		#[allow(unused_mut, unused_variables)]
		let mut __frame = __slot;
	};
	// The batch loop is not a serve: the lane's own frame is its region of the
	// run's slab, so it serves in place.
	let lane_frame_entry = quote! {
		#[allow(unused_mut, unused_variables)]
		let mut __frame = __run.slot(__lane, &__lane_frames);
	};
	// A lazy edge claims beyond every input frame this node holds, and its
	// cursor is shared, so the edges a kernel drives claim past each other.
	let lazy_frames_entry = quote! {
		let __lazy_frames = __frame.frames().reborrow();
	};
	let bind_body = |index: usize, field: &ParsedField, batch_mode: bool, frames: &TokenStream2| {
		let name = &field.pat_ident.ident;
		// The bind's failure exits return through the enclosing fn: `GPoll` in
		// `eval`, `BatchStatus` in the generated `eval_batch`.
		let pending = match batch_mode {
			false => quote!(return #core_types::gpoll::GPoll::Pending),
			true => quote!(return #core_types::node::BatchStatus::Pending),
		};
		let fail = |error: TokenStream2| match batch_mode {
			false => quote!(return #core_types::gpoll::GPoll::Error(::std::boxed::Box::new(#error))),
			true => quote!(return #core_types::node::BatchStatus::Error(#error)),
		};
		let interrupt_return = match batch_mode {
			false => quote!(return interrupt.into()),
			true => quote!(return interrupt.into()),
		};
		match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => match ir::value_binding(&node, index) {
				// A carrier primary evaluates beyond the node's own frame (in the
				// record/flip tail), so it does not bind here.
				ValueBinding::Carrier => quote!(),
				ValueBinding::Materialized => {
					let fn_name = &parsed.fn_name;
					let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'_");
					let cache_slot = format_ident!("__mat_cache_{index}");
					let non_exact = fail(quote!(#core_types::gpoll::GraphError::new(::std::concat!("reduce over a non-exact extent in ", ::std::stringify!(#fn_name)))));
					let batch_error = fail(quote!(__error));
					let batch_failed = fail(quote!(#core_types::gpoll::GraphError::new("reduce batch failed")));
					// A fold consumes the whole subject wire: a deeper wire's
					// total flat span, sized under the evaluation context, so a
					// fold inside a pushed level covers that copy's span. The
					// span caches per (lane-normalized context, generation):
					// every lane of a per-lane emitter re-enters this bind, and
					// without the cache each lane would re-materialize the
					// whole subject.
					quote! {
						let __arena = #core_types::context::ExtractArena::arena(__input);
						let __mat_key = {
							let mut __keyed = *__input;
							#core_types::context::InjectIndex::set_index(&mut __keyed, 0);
							#core_types::registry::cache_key(&__keyed)
						};
						let __mat_hit = match *self.#cache_slot.lock().unwrap() {
							::core::option::Option::Some((__key, __span)) if __key == __mat_key => __span.batch(__arena, #core_types::node::Node::<#ctx_ident>::layout(&self.#name)),
							_ => ::core::option::Option::None,
						};
						let __batch = match __mat_hit {
							::core::option::Option::Some(__batch) => __batch,
							::core::option::Option::None => {
								let __sized = match #core_types::node::Node::extent(&self.#name, __input, #core_types::gpoll::Level::Total, #frames) {
									#core_types::gpoll::GPoll::Final(#core_types::gpoll::Extent::Exactly(__count)) => ::core::result::Result::Ok(__count),
									#core_types::gpoll::GPoll::Final(#core_types::gpoll::Extent::AtLeast(__bound)) => ::core::result::Result::Err(__bound),
									#core_types::gpoll::GPoll::Pending => #pending,
									_ => #non_exact,
								};
								let __fresh = match __sized {
									::core::result::Result::Ok(__count) => {
										let __start: u64 = 0;
										match #core_types::record::materialize_batch(&self.#name, __input, __start..__start + __count as u64, __arena, #frames) {
											#core_types::node::BatchStatus::Lent(__batch, ..) => __batch,
											#core_types::node::BatchStatus::Filled(__batch, ..) => __batch.into_shared(),
											#core_types::node::BatchStatus::Pending => #pending,
											#core_types::node::BatchStatus::Error(__error) => #batch_error,
											_ => #batch_failed,
										}
									}
									// The count is a lower bound: drain by guess-and-double
									// until a short fill, each reply's hint seeding the next
									// guess.
									::core::result::Result::Err(__bound) => {
										let mut __guess = __bound.max(16);
										loop {
											let (__batch, __hint) = match #core_types::record::materialize_batch(&self.#name, __input, 0..__guess as u64, __arena, #frames) {
												#core_types::node::BatchStatus::Lent(__batch, _, __hint) => (__batch, __hint),
												#core_types::node::BatchStatus::Filled(__batch, _, __hint) => (__batch.into_shared(), __hint),
												#core_types::node::BatchStatus::Pending => #pending,
												#core_types::node::BatchStatus::Error(__error) => #batch_error,
												_ => #batch_failed,
											};
											let __filled = __batch.len();
											if __filled < __guess {
												break __batch;
											}
											match __hint {
												#core_types::gpoll::Extent::Exactly(__total) if __total <= __filled => break __batch,
												#core_types::gpoll::Extent::Exactly(__total) => __guess = __total,
												#core_types::gpoll::Extent::AtLeast(__more) => __guess = (__guess * 2).max(__more),
												#core_types::gpoll::Extent::Free => __guess *= 2,
											}
										}
									}
								};
								if let ::core::option::Option::Some(__span) = #core_types::record::MaterializedSpan::of(&__fresh, __arena) {
									*self.#cache_slot.lock().unwrap() = ::core::option::Option::Some((__mat_key, __span));
								}
								__fresh
							}
						};
						let #name = unsafe { #core_types::node::List::<#ty>::new(__batch) };
					}
				}
				// A reading secondary input claims a record edge: the element and
				// the declared reads copy out right after its eval.
				ValueBinding::ReadingSecondary => {
					let slot = format_ident!("__in_{index}");
					let rec_local = format_ident!("__rec_{index}");
					let bindings: Vec<TokenStream2> = reads_of(index).into_iter().map(|(slot, read)| read_binding(slot, read, quote!(#rec_local))).collect();
					quote! {
						let #name = match __cell.eval_input(#index, &self.#name, __input, #frames) {
							Ok(value) => value,
							Err(interrupt) => #interrupt_return,
						};
						let #rec_local = self.#slot.rec(&#name);
						#(#bindings)*
						let #name: #ty = unsafe { #core_types::record::read_element(#rec_local) };
					}
				}
				// The lend input's frame is claimed out of this node's own claim and
				// lives as long as it does, so the borrow stays valid in place.
				ValueBinding::Lend => {
					let slot = format_ident!("__in_{index}");
					let record_local = format_ident!("__record_{index}");
					quote! {
						let #record_local = match __cell.eval_input(#index, &self.#name, __input, #frames) {
							Ok(value) => value,
							Err(interrupt) => #interrupt_return,
						};
						let #name = unsafe { #core_types::record::borrow_element::<#ty>(self.#slot.rec(&#record_local)) };
					}
				}
				// A flip value or a routing non-source value rides a record edge; the
				// element copies out into `name`. The mark/rewind that reclaims the
				// record's frame is applied by the step lowering (see `reads_out`).
				ValueBinding::RecordElement => {
					let slot = format_ident!("__in_{index}");
					quote! {
						let #name = match __cell.eval_input(#index, &self.#name, __input, #frames) {
							Ok(value) => value,
							Err(interrupt) => #interrupt_return,
						};
						let #name: #ty = unsafe { #core_types::record::read_element(self.#slot.rec(&#name)) };
					}
				}
				// A plain value rides a record edge like every other input; the
				// element copies out against the edge's own layout, except for a
				// routing source, whose record is what the kernel forwards.
				ValueBinding::Plain => {
					let read = (!routing_source(ty)).then(|| {
						quote! {
							let #name: #ty = unsafe {
								#core_types::record::read_element(#core_types::node::Node::<#ctx_ident>::layout(&self.#name).rec(&#name))
							};
						}
					});
					quote! {
						let #name = match __cell.eval_input(#index, &self.#name, __input, #frames) {
							Ok(value) => value,
							Err(interrupt) => #interrupt_return,
						};
						#read
					}
				}
			},
			ParsedFieldType::Node(NodeParsedField { output_type, .. }) => match (ir::lazy_binding(&node, index), raw_lazy) {
				// A raw poll edge is threaded straight through, so it does not bind here.
				(LazyBinding::Generic, true) => quote!(),
				(LazyBinding::DeriveRouting, _) => quote! {
					let #name = #core_types::record::RecordLazyInput::new(&self.#name, &__cell, #index, self.__layout.depth.saturating_sub(#pushed_levels), &__lazy_frames);
				},
				(LazyBinding::DeriveCarrier, _) => {
					let reads = reads_of(index);
					let read_fn = format_ident!("__{}_read_{}", fn_name, index);
					match reads.is_empty() {
						true => quote! {
							let #name = #core_types::record::DerivedLazyInput::new(&self.#name, &__cell, #index, self.__layout.depth.saturating_sub(#pushed_levels), &[], #core_types::record::token_only, &__lazy_frames);
						},
						false => {
							let slot_idents: Vec<Ident> = reads.iter().map(|(slot, _)| format_ident!("__read_{slot}")).collect();
							quote! {
								let __carrier_reads = [#(self.#slot_idents),*];
								let #name = #core_types::record::DerivedLazyInput::new(&self.#name, &__cell, #index, self.__layout.depth.saturating_sub(#pushed_levels), &__carrier_reads, self::#read_fn, &__lazy_frames);
							}
						}
					}
				}
				(LazyBinding::Element, true) => {
					let slot = format_ident!("__in_{index}");
					match field.attribute_reads.is_empty() {
						true => quote! {
							let #name = #core_types::record::ElementEdge::<#output_type, _>::new(&self.#name, &self.#slot, &__lazy_frames);
						},
						false => {
							let arr = format_ident!("__reads_{index}");
							let read_fn = format_ident!("__{}_read_{}", fn_name, index);
							quote! {
								let #name = #core_types::record::ElementEdge::with_reads(&self.#name, &self.#slot, &self.#arr, self::#read_fn, &__lazy_frames);
							}
						}
					}
				}
				(LazyBinding::Element, false) => {
					let slot = format_ident!("__in_{index}");
					match field.attribute_reads.is_empty() {
						true => quote! {
							let #name = #core_types::record::ElementLazyInput::<#output_type, _>::new(&self.#name, &__cell, #index, &self.#slot, &__lazy_frames);
						},
						false => {
							let arr = format_ident!("__reads_{index}");
							let read_fn = format_ident!("__{}_read_{}", fn_name, index);
							quote! {
								let #name = #core_types::record::ElementLazyInput::with_reads(&self.#name, &__cell, #index, &self.#slot, &self.#arr, self::#read_fn, &__lazy_frames);
							}
						}
					}
				}
				(LazyBinding::OpaqueRecord, _) => quote! {
					let #name = #core_types::record::RecordEdgeInput::new(&self.#name, &self.__layout, &__lazy_frames);
				},
				(LazyBinding::Generic, false) => quote! {
					let #name = #core_types::node::LazyInput::new(&self.#name, &__cell, #index, &__lazy_frames);
				},
			},
		}
	};

	// A bind whose element copies out reclaims the edge's frame; a forwarded
	// record must outlive the bind, so its frame stays.
	let _reads_out_at = |index: usize| match &regular_fields[index].ty {
		ParsedFieldType::Regular(RegularParsedField { ty, .. }) => !routing_source(ty) && ir::value_binding(&node, index).reads_out(),
		ParsedFieldType::Node(_) => false,
	};

	let clamp_tokens = |field: &ParsedField| {
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
	};

	let call_args = regular_fields.iter().enumerate().filter(|(_, field)| !injected_name(&field.pat_ident.ident)).map(|(index, field)| {
		let name = &field.pat_ident.ident;
		match &field.ty {
			// A lend param binds an owned edge; the kernel borrows the
			// evaluated value.
			ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }) if !flip => quote!(&#name),
			ParsedFieldType::Regular(_) => quote!(#name),
			ParsedFieldType::Node(_) => match (ir::lazy_binding(&node, index), raw_lazy) {
				(LazyBinding::Element, true) | (LazyBinding::OpaqueRecord, _) => quote!(&#name),
				(LazyBinding::Generic, true) => quote!(&self.#name),
				_ => quote!(#name),
			},
		}
	});

	// The extent override is the leveled `extent_at`; consumers query the
	// composite `extent(ctx, Level)`, which the trait derives from it. A node
	// without `extent = fn` keeps the scalar default (one item at every level).
	// The typed extent surface: the node's inputs in declaration order (values
	// readable without unsafe, edges as per-level extent queries, derived
	// content promoted per copy), then the level paired with the node's depth.
	let extent_impl = if let Some(path) = &parsed.attributes.extent {
		let mut arg_decls: Vec<TokenStream2> = Vec::new();
		let mut arg_names: Vec<Ident> = Vec::new();
		for (index, field) in regular_fields.iter().enumerate() {
			let name = &field.pat_ident.ident;
			if injected_name(name) {
				continue;
			}
			let arg = format_ident!("__extent_arg_{index}");
			let query = format_ident!("__extent_query_{index}");
			let extent_edge = |query: &Ident, arg: &Ident| {
				quote! {
					let #query = |_: u64, __lvl: u8| #core_types::node::Node::extent_at(&self.#name, __input, __lvl, &__frames.scope());
					let #arg = #core_types::extent::ExtentIn::new(&#query);
				}
			};
			let decl = match &field.ty {
				ParsedFieldType::Node(_) => match ir::lazy_binding(&node, index) {
					ir::LazyBinding::DeriveRouting | ir::LazyBinding::DeriveCarrier => quote! {
						let #query = |__copy: u64, __lvl: u8| {
							let mut __frame = #core_types::context::IndexLink { index: 0, outer: None };
							let __derived = #core_types::context::DeriveCtx::push_level(__input, &mut __frame, __copy, 0);
							#core_types::record::DerivedRecordInput::extent_at_derived(&self.#name, &__derived, __lvl, &__frames.scope())
						};
						let #arg = #core_types::extent::ExtentIn::new(&#query);
					},
					_ => extent_edge(&query, &arg),
				},
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => match ir::value_binding(&node, index) {
					// A ranked input materializes whole (the wire's total flat
					// span, as in eval), so a data-dependent extent can walk
					// its lanes.
					ValueBinding::Materialized => {
						let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'_");
						quote! {
							let #query = || {
								let __arena = #core_types::context::ExtractArena::arena(__input);
								let __count = match #core_types::node::Node::extent(&self.#name, __input, #core_types::gpoll::Level::Total, &__frames.scope()) {
									#core_types::gpoll::GPoll::Final(#core_types::gpoll::Extent::Exactly(__count)) => __count,
									#core_types::gpoll::GPoll::Pending => return #core_types::gpoll::GPoll::Pending,
									_ => return #core_types::gpoll::GPoll::Error(::std::boxed::Box::new(#core_types::gpoll::GraphError::new("extent over a non-exact ranked input"))),
								};
								match #core_types::record::materialize_batch(&self.#name, __input, 0..__count as u64, __arena, __frames) {
									#core_types::node::BatchStatus::Lent(__batch, ..) => #core_types::gpoll::GPoll::Final(unsafe { #core_types::node::List::<#ty>::new(__batch) }),
									#core_types::node::BatchStatus::Filled(__batch, ..) => #core_types::gpoll::GPoll::Final(unsafe { #core_types::node::List::<#ty>::new(__batch.into_shared()) }),
									#core_types::node::BatchStatus::Pending => #core_types::gpoll::GPoll::Pending,
									#core_types::node::BatchStatus::Error(__error) => #core_types::gpoll::GPoll::Error(::std::boxed::Box::new(__error)),
									_ => #core_types::gpoll::GPoll::Error(::std::boxed::Box::new(#core_types::gpoll::GraphError::new("extent could not materialize a ranked input"))),
								}
							};
							let __total = || #core_types::node::Node::extent(&self.#name, __input, #core_types::gpoll::Level::Total, &__frames.scope());
							let #arg = #core_types::extent::ListIn::new(&#query, &__total);
						}
					}
					// A routing source forwards its record whole; its extents are
					// the queryable quantity.
					_ if routing_source(ty) => extent_edge(&query, &arg),
					ValueBinding::RecordElement | ValueBinding::ReadingSecondary | ValueBinding::Plain => {
						let layout = match ir::value_binding(&node, index) {
							ValueBinding::Plain => quote!(#core_types::node::Node::<#ctx_ident>::layout(&self.#name)),
							_ => {
								let slot = format_ident!("__in_{index}");
								quote!(self.#slot)
							}
						};
						quote! {
							let #query = || {
								// The element copies out by value, so the edge's
								// claim dies with the query.
								let __scope = __frames.scope();
								#core_types::record::serve_input(&self.#name, __input, &__scope)
									.map(|__value| unsafe { #core_types::record::read_element::<#ty>(#layout.rec(&__value)) })
							};
							let #arg = #core_types::extent::ValueIn::new(&#query);
						}
					}
					// A carrier, lent, or materialized ranked input is a record
					// edge; its extents are the queryable quantity.
					_ => extent_edge(&query, &arg),
				},
			};
			arg_decls.push(decl);
			arg_names.push(arg);
		}
		quote! {
			fn extent_at<'__serve>(&self, __input: &#ctx_ident, __level: u8, __frames: &#core_types::record::Frames<'__serve>) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent>
				where
					#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__serve #core_types::arena::Arena>,
				{
				#(#arg_decls)*
				let __level_in = #core_types::extent::LevelIn::new(__level, <Self as #core_types::node::Node<#ctx_ident>>::layout(self).depth);
				#path(#(#arg_names,)* __level_in)
			}
		}
	} else if let Some(path) = &parsed.attributes.extent_raw {
		quote! {
			fn extent_at<'__serve>(&self, __input: &#ctx_ident, __level: u8, _: &#core_types::record::Frames<'__serve>) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent>
				where
					#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__serve #core_types::arena::Arena>,
				{
				#path(self, __input, __level)
			}
		}
	} else if let Some(subject_index) = ir::forwarded_subject(&node).filter(|_| node.output.shape.depth == 0) {
		// A level-preserving passthrough forwards its subject's extents,
		// through the same per-binding query forms the explicit surface uses.
		let field = &regular_fields[subject_index];
		let name = &field.pat_ident.ident;
		let query = match &field.ty {
			ParsedFieldType::Node(_) => match ir::lazy_binding(&node, subject_index) {
				ir::LazyBinding::DeriveRouting | ir::LazyBinding::DeriveCarrier => quote! {
					let __query = |_: u64, __lvl: u8| {
						let __head = #core_types::context::DeriveCtx::index_head(__input);
						let __derived = #core_types::context::DeriveCtx::replaced(__input, __head.index);
						#core_types::record::DerivedRecordInput::extent_at_derived(&self.#name, &__derived, __lvl, &__frames.scope())
					};
				},
				_ => quote! {
					let __query = |_: u64, __lvl: u8| #core_types::node::Node::extent_at(&self.#name, __input, __lvl, &__frames.scope());
				},
			},
			ParsedFieldType::Regular(_) => quote! {
				let __query = |_: u64, __lvl: u8| #core_types::node::Node::extent_at(&self.#name, __input, __lvl, &__frames.scope());
			},
		};
		quote! {
			fn extent_at<'__serve>(&self, __input: &#ctx_ident, __level: u8, __frames: &#core_types::record::Frames<'__serve>) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent>
				where
					#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__serve #core_types::arena::Arena>,
				{
				#query
				let __arg = #core_types::extent::ExtentIn::new(&__query);
				let __level_in = #core_types::extent::LevelIn::new(__level, <Self as #core_types::node::Node<#ctx_ident>>::layout(self).depth);
				__arg.at(__level_in)
			}
		}
	} else if node.output.shape.depth > 0 {
		// A leveled output without an extent fn reports a lower bound;
		// consumers size it by draining to the past-end signal.
		quote! {
			fn extent_at<'__serve>(&self, _: &#ctx_ident, _: u8, _: &#core_types::record::Frames<'__serve>) -> #core_types::gpoll::GPoll<#core_types::gpoll::Extent>
				where
					#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__serve #core_types::arena::Arena>,
				{
				#core_types::gpoll::GPoll::Final(#core_types::gpoll::Extent::AtLeast(0))
			}
		}
	} else {
		quote!()
	};

	let serialize_impl = match &parsed.attributes.serialize {
		Some(path) => {
			let data_refs = data_names.iter().map(|name| quote!(&self.#name));
			quote! {
				fn serialize(&self) -> Option<::std::sync::Arc<dyn ::std::any::Any + Send + Sync>> {
					#path(#(#data_refs),*)
				}
			}
		}
		None => quote!(),
	};

	let batch_signature = quote! {
		fn eval_batch<'__batch, '__serve>(
			&'__batch self,
			__input: &'__batch #ctx_ident,
			__range: ::std::ops::Range<u64>,
			__scratch: Option<&'__batch mut [::std::mem::MaybeUninit<u64>]>,
			__frames: &#core_types::record::Frames<'__serve>,
		) -> #core_types::node::BatchStatus<'__batch>
		where
			#ctx_ident: #core_types::context::InjectIndex + Copy + #core_types::context::ExtractArena<ArenaRef = &'__serve #core_types::arena::Arena>,
	};
	let produces_records = record_io || routing_generic.is_some() || flip;

	let ctx_pat = &parsed.input.pat_ident;
	let fn_where = &parsed.where_clause;
	let body = if level_delta > 0 { rewrite_emit(&parsed.body) } else { parsed.body.clone() };
	let vis = &parsed.vis;
	let kernel_fields: Vec<&&ParsedField> = regular_fields.iter().filter(|field| !injected_name(&field.pat_ident.ident)).collect();
	// A bare `Attr<M>` in the return type cannot elide its lifetime, so the
	// kernel gets a fresh one; reference-valued writes name their real
	// lifetime explicitly and pass through untouched. An async source's value
	// outlives the evaluation, so its writes are `'static` instead.
	let attr_injected = record_io.then(|| inject_attr_lifetimes(&parsed.output_type, if async_source { "'static" } else { "'__attr" })).flatten();
	let attr_lifetime = (attr_injected.is_some() && !async_source).then(|| quote!('__attr,));
	let lane_injected = gather_carrier
		.then(|| crate::codegen::classify::inject_lane_lifetime(attr_injected.as_ref().unwrap_or(&parsed.output_type)))
		.flatten();
	let lane_lifetime = lane_injected.is_some().then(|| quote!('__lane,));
	let kernel_output = lane_injected.or(attr_injected);
	let kernel_output = match routing_generic.is_some() {
		true => {
			let generic = routing_generic.as_ref().expect("guarded by the arm");
			let ty = substitute_routing_record(&parsed.output_type, generic, core_types);
			quote!(#ty)
		}
		false => kernel_output.map(|ty| quote!(#ty)).unwrap_or_else(|| quote!(#output_type)),
	};
	let claim_param = parsed.claim.iter().map(|claim| quote!(, #claim));
	let claim_arg = parsed.claim.iter().map(|_| quote!(, __frame));
	let kernel = match async_fn {
		false => quote! {
			#[allow(clippy::too_many_arguments, clippy::type_complexity)]
			#vis fn #fn_name<#attr_lifetime #lane_lifetime #(#generics,)*>(#ctx_pat: &#ctx_ident #(, #data_params)* #(, #kernel_params)* #(#claim_param)*) -> #kernel_output #fn_where #body
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
				#[allow(clippy::too_many_arguments, clippy::type_complexity)]
				#vis async fn #fn_name<#(#kernel_generics,)*>(#(#params),*) -> #kernel_output #fn_where #body
			}
		}
	};
	let cell_constructor = match parsed.attributes.no_partial {
		true => quote!(#core_types::node::StatusCell::no_partial()),
		false => quote!(#core_types::node::StatusCell::new()),
	};
	let kernel_call = quote!(self::#fn_name(__input #(, &self.#data_names)* #(, #call_args)* #(#claim_arg)*));
	// A record-opaque kernel serves through the claim it was handed; every
	// other forwarding kernel returns a record of this node's layout, which
	// fills the claim.
	let forwarded = |value: TokenStream2| match opaque {
		true => value,
		// SAFETY: the kernel's record is of this node's layout.
		false => quote!(unsafe { __frame.forward(&#value) }),
	};
	let lift = match *model {
		Dialect::Interrupt => {
			let served = forwarded(quote!(value));
			quote! {
				match #kernel_call {
					Ok(value) => __cell.finish(#served),
					Err(interrupt) => interrupt.into()
				}
			}
		}
		Dialect::Poll => match opaque {
			true => quote!(__cell.merge(#kernel_call)),
			// SAFETY: the kernel's record is of this node's layout.
			false => quote!(__cell.merge(#kernel_call).map(|value| unsafe { __frame.forward(&value) })),
		},
		_ => {
			let served = forwarded(quote!(#kernel_call));
			quote!(__cell.finish(#served))
		}
	};

	let placeholder_value_names: Vec<&Ident> = kernel_fields
		.iter()
		.filter(|field| matches!(field.ty, ParsedFieldType::Regular(_)))
		.map(|field| &field.pat_ident.ident)
		.collect();
	// A carried tail claims the node's frame first, evaluates the carrier
	// beyond it, and carries its fields; every exit closes the frame through
	// `lift_poll_into`.
	let carried_prelude = carrier_flip.then(|| {
		let field = regular_fields[0];
		let name = &field.pat_ident.ident;
		let read = match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => {
				quote!(let #name: &#ty = unsafe { #core_types::record::borrow_element(__src_rec) };)
			}
			ParsedFieldType::Regular(RegularParsedField { ty, .. }) => quote!(let #name: #ty = unsafe { #core_types::record::read_element(__src_rec) };),
			_ => unreachable!("a flip carrier is a regular value input"),
		};
		let clamp = clamp_tokens(field);
		quote! {
			let __src = match __cell.eval_input(0, &self.#name, __input, __frame.frames()) {
				Ok(value) => value,
				Err(interrupt) => return interrupt.into(),
			};
			let __src_rec = self.__in_0.rec(&__src);
			unsafe { __frame.carry(__src_rec, &self.__plan) };
			#read
			#clamp
		}
	});
	// A writing source's carrier is its record-io carrier, so the fields it
	// passes through ride the record plan rather than the flip one.
	let carried_prelude = carried_prelude.or_else(|| {
		(record_io && async_source && !skips_carrier).then(|| {
			let field = regular_fields[0];
			let name = &field.pat_ident.ident;
			let ty = carrier_read_ty.expect("a carrying record source reads a concrete element");
			quote! {
				let __src = match __cell.eval_input(0, &self.#name, __input, __frame.frames()) {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				};
				let __src_rec = self.__carrier.rec(&__src);
				unsafe { __frame.carry(__src_rec, &self.__plan) };
				let #name: #ty = unsafe { #core_types::record::read_element(__src_rec) };
			}
		})
	});
	// Async slots persist plain values across evaluations; the source lifts
	// the slot value onto its record wire at every merge point, into the
	// carried frame when the node has a carrier.
	// A writing source stores the kernel's whole tuple as that plain value:
	// the lift writes the attributes through the claim, then lifts the
	// element, the shape the sync record tail closes with. An owned crossing
	// parks into the serving arena first, so its exhaustion is a poll.
	let source_writes = (record_io && async_source && !write_markers.is_empty()).then(|| {
		let binders: Vec<Ident> = (0..write_markers.len()).map(|index| format_ident!("__attr_{index}")).collect();
		let stores = node.output.shape.attrs.iter().enumerate().map(|(index, attr)| {
			let binder = &binders[index];
			let slot = format_ident!("__write_{index}");
			match attr.owned {
				false => quote!(unsafe { __frame.attr_at(self.#slot, #binder.0) };),
				true => quote! {
					let #binder = match #binder.park(#core_types::context::ExtractArena::arena(__input)) {
						::core::option::Option::Some(value) => value,
						::core::option::Option::None => return #core_types::gpoll::GPoll::arena_exhausted(),
					};
					unsafe { __frame.attr_at(self.#slot, #binder) };
				},
			}
		});
		quote! {
			.and_then(|(__element #(, #binders)*)| {
				#(#stores)*
				#core_types::gpoll::GPoll::Final(__element)
			})
		}
	});
	let merge_lifted = |poll: TokenStream2| match &source_writes {
		None => quote!(__cell.merge(__frame.lift_served(#poll, #core_types::context::ExtractArena::arena(__input)))),
		Some(writes) => quote! {{
			let __lifted = (#poll) #writes;
			__cell.merge(__frame.lift_served(__lifted, #core_types::context::ExtractArena::arena(__input)))
		}},
	};
	// The claim drops with the frame still claimed, so a valueless exit needs
	// no closing of its own.
	let pending_return = quote!(#core_types::gpoll::GPoll::Pending);
	let inflight = match &parsed.attributes.placeholder {
		Some(path) => merge_lifted(quote!(#core_types::gpoll::GPoll::Partial(#path(#(&#placeholder_value_names),*)))),
		None => pending_return.clone(),
	};
	let slot_hit = merge_lifted(quote!(value.clone()));
	let slot_check = quote! {
		let __scope = #core_types::context::DeriveCtx::scope(__input).excluding(_source);
		let __key = #core_types::registry::cache_key(&#core_types::context::DeriveCtx::with_scope(__input, &__scope));
		{
			let __entries = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
			if let Some(__state) = __entries.get(&__key) {
				return match __state {
					Some(value) => #slot_hit,
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
	let spawned_hit = merge_lifted(quote!(__value.clone()));
	let spawn_tail = |completion: TokenStream2, fallback: TokenStream2| {
		let spawned_hit = spawned_hit.clone();
		quote! {
			self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, None);
			let __slot = std::sync::Arc::clone(&self.slot);
			if _runtime.0.spawn(_source, Box::pin(async move {
				let __value = #completion;
				__slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(__key, Some(__value));
			})) {
				let __entries = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
				if let Some(Some(__value)) = __entries.get(&__key) {
					return #spawned_hit;
				}
			}
			#fallback
		}
	};
	let record_tail_core = record_io.then(|| {
		let tuple_arg = |field: &ParsedField, value: TokenStream2| match field.attribute_reads.is_empty() {
			true => value,
			false => {
				let read_pats = field.attribute_reads.iter().map(|read| &read.pat_ident.ident);
				quote!((#value #(, #read_pats)*))
			}
		};
		let carrier_arg = if skips_carrier {
			None
		} else if lazy_carrier {
			// The kernel drives the derived carrier itself through its handle.
			let name = &regular_fields[0].pat_ident.ident;
			Some(quote!(#name))
		} else if let Some(ty) = carrier_read_ty {
			Some(tuple_arg(regular_fields[0], quote!(unsafe { #core_types::record::read_element::<#ty>(__src_rec) })))
		} else {
			Some(tuple_arg(regular_fields[0], quote!(#core_types::record::ElToken)))
		}
		.into_iter();
		let value_args = regular_fields.iter().skip(if skips_carrier { 0 } else { 1 }).map(|field| {
			let name = &field.pat_ident.ident;
			match &field.ty {
				// A lend param binds an owned edge; the kernel borrows the
				// evaluated value.
				ParsedFieldType::Regular(RegularParsedField { lend: Some(_), .. }) => quote!(&#name),
				_ => tuple_arg(field, quote!(#name)),
			}
		});
		let record_kernel_call = quote!(self::#fn_name(__input #(, &self.#data_names)* #(, #carrier_arg)* #(, #value_args)*));
		let carrier_eval = (!skips_carrier && !lazy_carrier).then(|| {
			let name = &regular_fields[0].pat_ident.ident;
			quote! {
				let __src = match __cell.eval_input(0, &self.#name, __input, __frame.frames()) {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into()
				};
				let __src_rec = self.__carrier.rec(&__src);
			}
		});
		let carry = (!skips_carrier && !lazy_carrier).then(|| quote!(unsafe { __frame.carry(__src_rec, &self.__plan) };));
		// A lazy carrier's source record is the token the kernel returned; its
		// content frames sit above the claim and stay readable until its drop.
		let lazy_carry = match lazy_carrier {
			true => quote! {
				let __src_rec = self.__carrier.rec(&__element);
				unsafe { __frame.carry(__src_rec, &self.__plan) };
			},
			false => TokenStream2::new(),
		};
		// A gathered lane owns its record, so the plan reads straight off it.
		let gather_carry = match gather_carrier {
			true => quote! {
				let __src_rec = __element.rec();
				unsafe { __frame.carry(__src_rec, &self.__plan) };
			},
			false => TokenStream2::new(),
		};
		let carrier_read_bindings: Vec<TokenStream2> = match skips_carrier || lazy_carrier {
			true => Vec::new(),
			false => reads_of(0).into_iter().map(|(slot, read)| read_binding(slot, read, quote!(__src_rec))).collect(),
		};
		let kernel_value = match *model {
			Dialect::Interrupt => quote! {
				match #record_kernel_call {
					Ok(__value) => __value,
					Err(__interrupt) => return __interrupt.into()
				}
			},
			_ => quote!(#record_kernel_call),
		};
		let attr_binders: Vec<Ident> = (0..write_markers.len()).map(|index| format_ident!("__attr_{index}")).collect();
		let element_binder = match (element_write.is_some(), lazy_carrier || gather_carrier) {
			(true, _) | (_, true) => quote!(__element),
			(false, false) => quote!(_),
		};
		// Slot binders in the return tuple's own order: an `Attr` binds the
		// next write binder, a `RemoveAttr` binds nothing.
		let slot_binders: Vec<TokenStream2> = {
			let mut binders = attr_binders.iter();
			match output_row.clone() {
				Type::Tuple(tuple) => tuple
					.elems
					.iter()
					.skip(1)
					.map(|slot| match attr_marker(slot) {
						Some(_) => {
							let binder = binders.next().expect("write binders match the Attr slots");
							quote!(#core_types::attribute::Attr(#binder))
						}
						None => quote!(_),
					})
					.collect(),
				_ => Vec::new(),
			}
		};
		let destructure = match slot_binders.is_empty() {
			true => quote!(let #element_binder = __kernel_value;),
			false => quote!(let (#element_binder #(, #slot_binders)*) = __kernel_value;),
		};
		// A droppable element parks in the arena and rides as a reference.
		let element_store = element_write.map(|ty| {
			let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'_");
			quote! {
				if __frame.element::<#ty>(__element, #core_types::context::ExtractArena::arena(__input)).is_none() {
					return #core_types::gpoll::Interrupt::from(#core_types::gpoll::GraphError {
						kind: #core_types::gpoll::ErrorKind::ArenaExhausted,
						trace: ::std::vec::Vec::new(),
					})
					.into();
				}
			}
		});
		let attr_stores = attr_binders.iter().enumerate().map(|(index, binder)| {
			let slot = format_ident!("__write_{index}");
			quote!(unsafe { __frame.attr_at(self.#slot, #binder) };)
		});
		quote! {
			#carrier_eval
			#carry
			#(#carrier_read_bindings)*
			let __kernel_value = #kernel_value;
			#destructure
			#lazy_carry
			#gather_carry
			#element_store
			#(#attr_stores)*
			// SAFETY: the carry and the writes above complete the record.
			let __value = unsafe { __frame.finish_served() };
		}
	});
	let record_tail = record_tail_core.clone().map(|core| {
		quote! {
			#core
			__cell.finish(__value)
		}
	});
	let flip_tail = flip.then(|| {
		if matches!(*model, Dialect::Poll) {
			let prelude = carried_prelude.clone().unwrap_or_default();
			return quote! {
				#prelude
				__cell.merge(__frame.lift_served(#kernel_call, #core_types::context::ExtractArena::arena(__input)))
			};
		}
		let kernel_value = match *model {
			Dialect::Interrupt => quote! {
				match #kernel_call {
					Ok(value) => value,
					Err(interrupt) => return interrupt.into(),
				}
			},
			_ => quote!(#kernel_call),
		};
		let prelude = carried_prelude.clone().unwrap_or_default();
		quote! {
			#prelude
			let __kernel_value = #kernel_value;
			__cell.merge(__frame.lift_served(#core_types::gpoll::GPoll::Final(__kernel_value), #core_types::context::ExtractArena::arena(__input)))
		}
	});
	let tail_form = if async_fn {
		Tail::SpawnAsyncFn
	} else if future_kernel {
		Tail::SpawnFuture
	} else {
		match ir::node_kind(&node) {
			ir::NodeKind::RecordIo => Tail::Record,
			ir::NodeKind::Flip => Tail::Flip,
			ir::NodeKind::Routing | ir::NodeKind::Opaque => Tail::Forward,
		}
	};
	let lower_tail = |form: Tail| match form {
		Tail::Forward => lift.clone(),
		Tail::Record => record_tail.clone().expect("a record-io node has a record tail"),
		Tail::Flip => flip_tail.clone().expect("a flip node has a flip tail"),
		Tail::SpawnAsyncFn => {
			let kernel_value_names: Vec<&Ident> = kernel_fields.iter().map(|field| &field.pat_ident.ident).collect();
			let snapshot_binding = snapshot_ctx.then(|| quote!(let __snapshot = #core_types::context::CtxSnapshot::capture(__input);)).into_iter();
			let snapshot_arg = snapshot_ctx.then(|| quote!(__snapshot)).into_iter();
			let future_args = snapshot_arg
				.chain(data_names.iter().map(|name| quote!(self.#name.clone())))
				.chain(kernel_value_names.iter().map(|name| quote!(#name.clone())));
			let completion = future_completion(&parsed.output_type);
			let tail = spawn_tail(completion, inflight.clone());
			let prelude = carried_prelude.iter();
			quote! {
				#(#prelude)*
				#slot_check
				#(#snapshot_binding)*
				let __future = self::#fn_name(#(#future_args),*);
				#tail
			}
		}
		Tail::SpawnFuture => {
			let (placeholder_binding, spawn_return) = match &parsed.attributes.placeholder {
				Some(path) => (
					quote!(let __placeholder = #path(#(&#placeholder_value_names),*);),
					merge_lifted(quote!(#core_types::gpoll::GPoll::Partial(__placeholder))),
				),
				None => (quote!(), pending_return.clone()),
			};
			let acquire = match *model {
				Dialect::FutureInterrupt => quote! {
					let __future = match #kernel_call {
						Ok(future) => future,
						Err(interrupt) => return interrupt.into()
					};
				},
				_ => quote!(let __future = #kernel_call;),
			};
			let payload = match kernel_kind(&parsed.output_type) {
				KernelKind::Future(payload) | KernelKind::FutureInterrupt(payload) => payload,
				_ => unreachable!("guarded by future_kernel"),
			};
			let completion = future_completion(&payload);
			let tail = spawn_tail(completion, spawn_return);
			let prelude = carried_prelude.iter();
			quote! {
				#(#prelude)*
				#slot_check
				#placeholder_binding
				#acquire
				#tail
			}
		}
	};

	// The default batch body binds every non-lazy input once at the batch's
	// base lane, so the per-lane loop runs only the kernel and the carrier;
	// eager inputs are batch-invariant by contract (per-lane variance rides
	// lazy carriers).
	// Each lane serves in place into its own region of the run, so the loop
	// collects the serving proofs.
	let hoisted_lane_poll = match tail_form {
		Tail::Record => record_tail_core.clone().map(|core| {
			quote! {
				#core
				let __poll = __cell.finish(__value);
			}
		}),
		Tail::Forward if routing_generic.is_some() => Some(quote!(let __poll = #lift;)),
		_ => None,
	};
	// A serving-lifetime element rides the per-lane fill loop: the hoisted
	// batch fill cannot yet carry an arena-lifetimed element through the
	// caller's scratch.
	let hoisted_lane_poll = match &node.output.shape.element {
		ir::Element::Concrete(element) if crate::codegen::classify::named_serving_lifetime(element).is_some() => None,
		_ => hoisted_lane_poll,
	};
	let hoisted_batch = parsed.attributes.batch.is_none() && produces_records && hoisted_lane_poll.is_some();
	let batch_impl = match (&parsed.attributes.batch, produces_records, hoisted_lane_poll) {
		(Some(path), ..) => quote! {
			#batch_signature
			{
				#path(self, __input, __range, __scratch, __frames)
			}
		},
		(None, true, Some(lane_poll)) => {
			// A non-materialized subject rides the per-lane record, so it binds
			// in the loop (or in the tail, for a carrier); everything else is
			// batch-invariant and hoists.
			let hoists = |index: usize| matches!(ir::value_binding(&node, index), ValueBinding::Materialized) || !node.inputs[index].subject;
			let hoisted_binds: Vec<TokenStream2> = regular_fields
				.iter()
				.enumerate()
				.filter(|(index, field)| matches!(field.ty, ParsedFieldType::Regular(_)) && hoists(*index))
				.map(|(index, field)| bind_body(index, field, true, &quote!((&*__frames))))
				.collect();
			let hoisted_clamps: Vec<TokenStream2> = regular_fields
				.iter()
				.enumerate()
				.filter(|(index, field)| matches!(field.ty, ParsedFieldType::Regular(_)) && hoists(*index))
				.filter_map(|(_, field)| clamp_tokens(field))
				.collect();
			let lane_binds: Vec<TokenStream2> = lazy_last(
				regular_fields
					.iter()
					.enumerate()
					.filter(|(index, field)| match field.ty {
						ParsedFieldType::Node(_) => true,
						ParsedFieldType::Regular(_) => !hoists(*index) && !matches!(ir::value_binding(&node, *index), ValueBinding::Carrier),
					})
					.map(|(index, field)| {
						let body = bind_body(index, field, true, &quote!(__frame.frames()));
						let clamp = clamp_tokens(field);
						(field, quote!(#body #clamp))
					}),
				&lazy_frames_entry,
			);
			// The rebind path with nothing hoisted: every non-carrier input binds
			// fresh per lane, so an index-dependent edge reaches its own lane.
			let rebound_lane_binds: Vec<TokenStream2> = lazy_last(
				regular_fields
					.iter()
					.enumerate()
					.filter(|(index, field)| match field.ty {
						ParsedFieldType::Node(_) => true,
						ParsedFieldType::Regular(_) => !matches!(ir::value_binding(&node, *index), ValueBinding::Carrier),
					})
					.map(|(index, field)| {
						let body = bind_body(index, field, true, &quote!(__frame.frames()));
						let clamp = clamp_tokens(field);
						(field, quote!(#body #clamp))
					}),
				&lazy_frames_entry,
			);
			// A hoisted value is moved into every lane's kernel call, so each
			// lane consumes a clone; view and borrow binds copy freely.
			let lane_rebinds: Vec<TokenStream2> = regular_fields
				.iter()
				.enumerate()
				.filter(|(index, field)| {
					matches!(field.ty, ParsedFieldType::Regular(_))
						&& hoists(*index) && matches!(ir::value_binding(&node, *index), ValueBinding::Plain | ValueBinding::ReadingSecondary | ValueBinding::RecordElement)
				})
				.map(|(_, field)| {
					let name = &field.pat_ident.ident;
					quote!(let #name = ::core::clone::Clone::clone(&#name);)
				})
				.collect();
			// A hoisted input past bit 31 has no bit to check, so it never reads
			// back as invariant and the node keeps rebinding it.
			let hoistable_mask: u32 = regular_fields
				.iter()
				.enumerate()
				.filter(|(index, field)| matches!(field.ty, ParsedFieldType::Regular(_)) && hoists(*index) && *index < 32)
				.fold(0, |mask, (index, _)| mask | (1u32 << index));
			let fill_loop = |hoisted: Vec<TokenStream2>, clamps: Vec<TokenStream2>, rebinds: Vec<TokenStream2>, binds: Vec<TokenStream2>| {
				let hoisted = hoisted.into_iter();
				let clamps = clamps.into_iter();
				let rebinds = rebinds.into_iter();
				let binds = binds.into_iter();
				quote! {
					#(#hoisted)*
					#(#clamps)*
					let ::core::option::Option::Some(mut __run) = __frames.run(__scratch, __len, __node_layout) else {
						return #core_types::node::BatchStatus::InvalidRange;
					};
					let mut __finality = #core_types::gpoll::Finality::AllFinal;
					let mut __hint = #core_types::gpoll::Extent::AtLeast(__range.end as usize);
					let mut __lane_ctx = __base_ctx;
					for __lane in 0..__len {
						#core_types::context::InjectIndex::set_index(&mut __lane_ctx, __range.start + __lane as u64);
						let __input = &__lane_ctx;
						// The lane's inputs claim beyond its slab region, and their
						// space is free again at the next lane.
						let __lane_frames = __frames.scope();
						#lane_frame_entry
						let __cell = __cell.snapshot();
						#(#rebinds)*
						#(#binds)*
						#lane_poll
						let __served = match __poll {
							#core_types::gpoll::GPoll::Final(__value) => __value,
							#core_types::gpoll::GPoll::Partial(__value) => {
								__finality = #core_types::gpoll::Finality::Partial;
								__value
							}
							#core_types::gpoll::GPoll::Pending => return #core_types::node::BatchStatus::Pending,
							#core_types::gpoll::GPoll::Fallback(__boxed) => return #core_types::node::BatchStatus::Error(__boxed.1),
							// A lane past a lower-bound level ends the data: the fill
							// comes back short and the hint turns exact.
							#core_types::gpoll::GPoll::Error(__error) if __error.kind == #core_types::gpoll::ErrorKind::PastEnd => {
								__hint = #core_types::gpoll::Extent::Exactly(__range.start as usize + __lane);
								break;
							}
							#core_types::gpoll::GPoll::Error(__error) => return #core_types::node::BatchStatus::Error(*__error),
						};
						__run.served(__lane, &__served);
					}
					#core_types::node::BatchStatus::Filled(__run.finish(), __finality, __hint)
				}
			};
			let hoisted_fill = fill_loop(hoisted_binds, hoisted_clamps, lane_rebinds, lane_binds);
			let rebound_fill = fill_loop(Vec::new(), Vec::new(), Vec::new(), rebound_lane_binds);
			// With nothing hoisted the two fills are the same code, and the mask
			// test would read as an empty bit mask.
			let selected_fill = match hoistable_mask {
				0 => hoisted_fill,
				mask => quote! {
					// Binding once at the base lane is sound only where the
					// installed layout marks every hoisted input invariant under
					// the innermost index.
					const __HOISTABLE: u32 = #mask;
					if (self.__lane_invariant & __HOISTABLE) == __HOISTABLE {
						#hoisted_fill
					} else {
						#rebound_fill
					}
				},
			};
			quote! {
				#batch_signature
				{
					let ::core::option::Option::Some(__scratch) = __scratch else {
						return #core_types::node::BatchStatus::NeedBuffer;
					};
					let ::core::option::Option::Some(__len) = __range.end.checked_sub(__range.start).and_then(|__len| usize::try_from(__len).ok()) else {
						return #core_types::node::BatchStatus::InvalidRange;
					};
					let __node_layout = <Self as #core_types::node::Node<#ctx_ident>>::layout(self);
					// The batch's own claims are free again when it returns, so
					// the caller's free space comes back as it was lent.
					let __frames = __frames.scope();
					let __cell = #cell_constructor;
					let __base_ctx = {
						let mut __ctx = *__input;
						#core_types::context::InjectIndex::set_index(&mut __ctx, __range.start);
						__ctx
					};
					let __input = &__base_ctx;
					#selected_fill
				}
			}
		}
		// The eager forward runs the shared copy-out loop with statically
		// dispatched evals, so an erased batch costs one virtual call.
		(None, true, None) => quote! {
			#batch_signature
			{
				#core_types::record::fill_frames(self, __input, __range, __scratch, __frames)
			}
		},
		(None, false, _) => quote!(),
	};

	let record_bounds: Vec<TokenStream2> = {
		// The serving lifetime is the serve method's, so the arena binding
		// rides there rather than on the impl.
		let mut bounds: Vec<TokenStream2> = Vec::new();
		// A reading secondary input's element copies out of its record, as
		// does a concrete carrier read.
		if record_io {
			bounds.extend(
				reading_secondary_indices(&regular_fields, skips_carrier)
					.into_iter()
					.filter_map(|index| match &regular_fields[index].ty {
						ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some({
							let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
							quote!(#ty: ::core::clone::Clone)
						}),
						_ => None,
					}),
			);
			if let Some(ty) = carrier_read_ty {
				bounds.push({
					let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
					quote!(#ty: ::core::clone::Clone)
				});
			}
			// The element store parks droppable elements in the arena.
			if let Some(ty) = element_write {
				bounds.push({
					let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
					quote!(#ty: ::core::marker::Send + ::core::marker::Sync + #core_types::StaticTypeSized + 'static)
				});
			}
		}
		// A routing node's value elements copy out of their records.
		if let Some(generic) = &routing_generic {
			bounds.extend(routing_value_indices(&regular_fields, generic).into_iter().filter_map(|index| match &regular_fields[index].ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some({
					let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
					quote!(#ty: ::core::clone::Clone)
				}),
				_ => None,
			}));
		}
		// The batch loop clones each hoisted value per lane.
		if hoisted_batch {
			bounds.extend(regular_fields.iter().enumerate().filter_map(|(index, field)| match &field.ty {
				ParsedFieldType::Regular(RegularParsedField { ty, .. })
					if !node.inputs[index].subject && matches!(ir::value_binding(&node, index), ValueBinding::Plain | ValueBinding::ReadingSecondary | ValueBinding::RecordElement) =>
				{
					Some({
						let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
						quote!(#ty: ::core::clone::Clone)
					})
				}
				_ => None,
			}));
		}
		// The materialized-span cache keys on the lane-normalized context.
		if !materialized_indices(&regular_fields, &node).is_empty() {
			bounds.push(quote!(#ctx_ident: #core_types::graphene_hash::CacheHash + #core_types::context::InjectIndex + ::core::marker::Copy));
		}
		bounds
	};

	let flip_bounds: Vec<TokenStream2> = match flip {
		true => {
			let mut bounds: Vec<TokenStream2> = regular_fields
				.iter()
				.enumerate()
				.filter(|(index, _)| ir::materialized_levels(&node, *index) == 0)
				.filter_map(|(_, field)| match &field.ty {
					// The conditional arena-park moves a lend element once.
					ParsedFieldType::Regular(RegularParsedField { ty, lend: Some(_), .. }) => Some({
						let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
						quote!(#ty: ::core::marker::Send + ::core::marker::Sync + 'static)
					}),
					ParsedFieldType::Regular(RegularParsedField { ty, .. }) => Some({
						let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
						quote!(#ty: ::core::clone::Clone)
					}),
					ParsedFieldType::Node(NodeParsedField { output_type, .. }) => Some({
						let output_type = &crate::codegen::classify::substitute_lifetimes(output_type, "'static");
						quote!(#output_type: ::core::clone::Clone)
					}),
				})
				.collect();
			let out = crate::codegen::classify::substitute_lifetimes(&slot_value_type(&parsed.output_type), "'static");
			bounds.push(quote!(#out: ::core::marker::Send + ::core::marker::Sync + #core_types::StaticTypeSized + 'static));
			bounds
		}
		false => Vec::new(),
	};

	let set_layout_body = if flip {
		let plan = carrier_flip.then(|| quote!(self.__plan = __resolved.plan;));
		Some(quote! {
			self.__frame_bytes = __resolved.frame_bytes;
			#plan
			self.__layout = __resolved.layout;
		})
	} else if record_io {
		let write_installs = write_markers.iter().enumerate().map(|(index, marker)| {
			let slot = format_ident!("__write_{index}");
			quote! {
				self.#slot = __resolved.layout.offset_of(<#marker as #core_types::attribute::Attribute>::NAME, 0).expect("a written attribute is always part of the wired layout");
			}
		});
		let plan = (!skips_carrier || gather_carrier).then(|| quote!(self.__plan = __resolved.plan;));
		Some(quote! {
			#(#write_installs)*
			self.__frame_bytes = __resolved.frame_bytes;
			self.__lane_invariant = __resolved.lane_invariant;
			#plan
			self.__layout = __resolved.layout;
		})
	} else if routing_generic.is_some() {
		Some(quote! {
			self.__lane_invariant = __resolved.lane_invariant;
			self.__layout = __resolved.layout;
		})
	} else {
		None
	};
	let set_layout_method = set_layout_body.map(|body| {
		quote! {
			fn set_layout(&mut self, __resolved: #core_types::record::RecordLayout) {
				#body
			}
		}
	});
	let record_layout_impl = match record_io || routing_generic.is_some() || flip || opaque {
		true => quote! {
			fn layout(&self) -> &#core_types::record::Layout {
				&self.__layout
			}
			#set_layout_method
		},
		false => quote!(),
	};
	let flip_meta_concrete = flip && !element_write.is_some_and(|ty| crate::codegen::classify::contains_open_generic(parsed, ty));
	let flip_layout_meta_fn = flip_meta_concrete.then(|| {
		let layout_meta_fn = format_ident!("{}_layout_meta", fn_name);
		let element_spec = match element_write {
			Some(ty) => {
				let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
				quote!(#core_types::record::ElementSpec::Concrete({ use #core_types::record::{ElementWritePickHashed as _, ElementWritePickPlain as _}; (&#core_types::record::ElementWritePick::<#ty>(::core::marker::PhantomData)).element_write() }))
			}
			None => quote!(#core_types::record::ElementSpec::Carried),
		};
		let layout_meta = crate::codegen::ir::layout_meta_tokens(&node, element_spec, core_types);
		// A flipped shader node's struct and impl are std-gated; its layout meta must be too.
		let cfg = crate::shader_nodes::modify_cfg(&parsed.attributes);
		quote! {
			#cfg
			#vis fn #layout_meta_fn() -> #core_types::record::LayoutMeta {
				#layout_meta
			}
		}
	});

	let entries = entries_tokens(parsed, &struct_name, &data_field_generic_idents, &regular_fields);
	let cfg = crate::shader_nodes::modify_cfg(&parsed.attributes);

	let record_wiring = record_io.then(|| {
		let layout_fn = format_ident!("{}_layout", fn_name);
		let write_descs: Vec<TokenStream2> = write_markers
			.iter()
			.map(|marker| quote!(#core_types::record::FieldWrite::of::<#marker>(0)))
			.collect();
		let remove_pairs: Vec<TokenStream2> = removes
			.iter()
			.map(|marker| quote!((<#marker as #core_types::attribute::Attribute>::NAME, 0)))
			.collect();
		let subtraction = (!remove_pairs.is_empty()).then(|| quote!(.without(&[#(#remove_pairs),*])));
		let element = match element_write {
			Some(ty) => {
				let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
				quote!({ use #core_types::record::{ElementWritePickHashed as _, ElementWritePickPlain as _}; (&#core_types::record::ElementWritePick::<#ty>(::core::marker::PhantomData)).element_write() })
			}
			None => quote!(__carrier.element),
		};
		// A gather carrier's base is the gathered subject's layout, so its free
		// layout fn takes that layout even though the subject materializes.
		let layout_def = match skips_carrier && !gather_carrier {
			true => quote! {
				#vis fn #layout_fn() -> #core_types::record::Layout {
					#core_types::record::Layout::default().with_writes(0, #element, &[#(#write_descs),*])
				}
			},
			false => quote! {
				#vis fn #layout_fn(__carrier: &#core_types::record::Layout) -> #core_types::record::Layout {
					__carrier #subtraction.with_writes(__carrier.depth + #pushed_levels, #element, &[#(#write_descs),*])
				}
			},
		};
		let layout_meta_fn = format_ident!("{}_layout_meta", fn_name);
		let element_spec = match element_write {
			Some(ty) => {
				let ty = &crate::codegen::classify::substitute_lifetimes(ty, "'static");
				quote!(#core_types::record::ElementSpec::Concrete({ use #core_types::record::{ElementWritePickHashed as _, ElementWritePickPlain as _}; (&#core_types::record::ElementWritePick::<#ty>(::core::marker::PhantomData)).element_write() }))
			}
			None => quote!(#core_types::record::ElementSpec::Carried),
		};
		let layout_meta = crate::codegen::ir::layout_meta_tokens(&node, element_spec, core_types);
		let layout_meta_def = quote! {
			#vis fn #layout_meta_fn() -> #core_types::record::LayoutMeta {
				#layout_meta
			}
		};
		let reading_secondaries = reading_secondary_indices(&regular_fields, skips_carrier);
		// The layout slots the constructor fills: reading secondaries plus the
		// element-consuming lazy inputs, in field order to match the entries.
		let layout_slots: Vec<usize> = {
			let mut slots = reading_secondaries.clone();
			slots.extend(crate::codegen::ir::element_lazy_indices(&regular_fields, &node));
			slots.sort_unstable();
			slots
		};
		let edge_args = regular_fields.iter().zip(&node_generics).map(|(field, generic)| {
			let name = &field.pat_ident.ident;
			quote!(#name: #generic)
		});
		let carrier_layout_param = (!skips_carrier).then(|| quote!(__carrier_layout: &#core_types::record::Layout,)).into_iter();
		let input_layout_params = layout_slots.iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(#slot: &#core_types::record::Layout,)
		});
		let read_inits = flat_reads.iter().enumerate().map(|(slot, (owner, read))| {
			let marker = &read.marker;
			let slot = format_ident!("__read_{slot}");
			let source = match !skips_carrier && *owner == 0 {
				true => quote!(__carrier_layout),
				false => format_ident!("__in_{owner}").to_token_stream(),
			};
			quote!(let #slot = #source.offset_of(<#marker as #core_types::attribute::Attribute>::NAME, 0);)
		});
		let data_inits = data_names.iter().map(|name| quote!(#name: ::core::default::Default::default(),));
		let edge_inits = regular_fields.iter().map(|field| {
			let name = &field.pat_ident.ident;
			quote!(#name,)
		});
		let carrier_init = (!skips_carrier).then(|| quote!(__carrier: __carrier_layout.clone(),)).into_iter();
		let input_layout_inits = layout_slots.iter().map(|index| {
			let slot = format_ident!("__in_{index}");
			quote!(#slot: #slot.clone(),)
		});
		let plan_default = (!skips_carrier || gather_carrier).then(|| quote!(__plan: ::std::vec::Vec::new(),)).into_iter();
		let read_names = (0..flat_reads.len()).map(|index| format_ident!("__read_{index}")).map(|slot| quote!(#slot,));
		let write_defaults = (0..write_markers.len()).map(|index| format_ident!("__write_{index}")).map(|slot| quote!(#slot: 0,));
		let mat_cache_defaults = materialized_indices(&regular_fields, &node).into_iter().map(|index| {
			let slot = format_ident!("__mat_cache_{index}");
			quote!(#slot: ::core::default::Default::default(),)
		});
		let slot_default = async_source.then(|| quote!(slot: ::core::default::Default::default(),)).into_iter();
		// A ranked input's element generic rides the struct as a phantom
		// parameter, so the constructor declares and initializes it too.
		let carried_type_params: Vec<&Ident> = struct_type_params
			.iter()
			.filter(|ident| !data_field_generic_idents.contains(ident) && !node_generics.contains(ident))
			.collect();
		let carried_generic_params: Vec<TokenStream2> = carried_type_params
			.iter()
			.map(|ident| {
				parsed
					.fn_generics
					.iter()
					.find_map(|param| match param {
						GenericParam::Type(type_param) if &&type_param.ident == ident => Some(quote!(#type_param)),
						_ => None,
					})
					.unwrap_or_else(|| quote!(#ident))
			})
			.collect();
		let marker_init = (!carried_type_params.is_empty()).then(|| quote!(__marker: ::core::marker::PhantomData,)).into_iter();
		quote! {
			#layout_def
			#layout_meta_def

			#[automatically_derived]
			impl<#(#data_field_generic_idents,)* #(#node_generics,)* #(#carried_generic_params,)*> #mod_name::#struct_name<#(#struct_type_params,)*> {
				#[allow(clippy::too_many_arguments)]
				#vis fn new(#(#edge_args,)* #(#carrier_layout_param)* #(#input_layout_params)*) -> Self {
					#(#read_inits)*
					Self {
						#(#data_inits)*
						#(#edge_inits)*
						#(#carrier_init)*
						#(#input_layout_inits)*
						__layout: ::core::default::Default::default(),
						#(#plan_default)*
						#(#marker_init)*
						__frame_bytes: 0,
						__lane_invariant: 0,
						#(#read_names)*
						#(#write_defaults)*
						#(#mat_cache_defaults)*
						#(#slot_default)*
					}
				}
			}
		}
	});

	// The eval body as an ordered step sequence: bind each input, clamp, then the
	// tail. Every input's frame is claimed out of this node's own claim and
	// stays claimed until it dies, which is the sizing the wiring layer derives.
	let mut bind_order: Vec<(bool, usize, &&ParsedField)> = regular_fields
		.iter()
		.enumerate()
		.map(|(index, field)| (matches!(field.ty, ParsedFieldType::Node(_)), index, field))
		.collect();
	bind_order.sort_by_key(|(lazy, ..)| *lazy);
	let eval_steps: Vec<EvalStep> = bind_order
		.iter()
		.map(|(_, index, field)| EvalStep::Bind(*index, field))
		.chain(
			regular_fields
				.iter()
				.enumerate()
				.filter(|(index, _)| !(carrier_flip && *index == 0))
				.map(|(_, field)| EvalStep::Clamp(field)),
		)
		.chain(std::iter::once(EvalStep::Tail(tail_form)))
		.collect();
	let mut lazy_declared = false;
	let eval_body: Vec<TokenStream2> = eval_steps
		.iter()
		.map(|step| match step {
			EvalStep::Bind(index, field) => {
				let body = bind_body(*index, field, false, &quote!(__frame.frames()));
				match matches!(field.ty, ParsedFieldType::Node(_)) && !std::mem::replace(&mut lazy_declared, true) {
					true => quote!(#lazy_frames_entry #body),
					false => body,
				}
			}
			EvalStep::Clamp(field) => clamp_tokens(field).unwrap_or_default(),
			EvalStep::Tail(form) => lower_tail(*form),
		})
		.collect();

	let top_level = quote! {
		#cfg
		#[automatically_derived]
		impl<#(#impl_generics,)* #(#node_generics,)*> #core_types::node::Node<#ctx_ident> for #mod_name::#struct_name<#(#struct_type_params,)*>
		where
			#(#node_bounds,)*
			#(#lend_outlives,)*
			#(#clampable_bounds,)*
			#(#async_bounds,)*
			#(#record_bounds,)*
			#(#flip_bounds,)*
			#(#where_predicates,)*
		{
			fn serve<'__serve, '__slot>(&self, __input: &#ctx_ident, __slot: #core_types::record::FrameClaim<'__serve, '__slot>) -> #core_types::gpoll::GPoll<#core_types::record::Served<'__serve>>
			where
				#ctx_ident: #core_types::context::ExtractArena<ArenaRef = &'__serve #core_types::arena::Arena>,
			{
				#frame_entry
				let __cell = #cell_constructor;
				#(#eval_body)*
			}

			#extent_impl

			#serialize_impl

			#record_layout_impl

			#batch_impl
		}
	};

	let lazy_read_fns: Vec<TokenStream2> = lazy_read_fields(&regular_fields)
		.into_iter()
		.map(|(index, field)| {
			let ParsedFieldType::Node(NodeParsedField { output_type, .. }) = &field.ty else {
				unreachable!("lazy read fields are Node fields");
			};
			let read_fn = format_ident!("__{}_read_{}", fn_name, index);
			let generics: Vec<&Ident> = parsed
				.fn_generics
				.iter()
				.filter_map(|param| match param {
					GenericParam::Type(type_param) if type_contains_ident(output_type, &type_param.ident) => Some(&type_param.ident),
					_ => None,
				})
				.collect();
			let attr_slots = field.attribute_reads.iter().enumerate().map(|(slot, read)| {
				let marker = &read.marker;
				quote!(unsafe { #core_types::record::read_at::<#marker>(__rec, __reads[#slot]) })
			});
			let attr_tys = field.attribute_reads.iter().map(|read| {
				let marker = &read.marker;
				quote!(#core_types::attribute::Attr<'__read, #marker>)
			});
			if matches!(ir::lazy_binding(&node, index), LazyBinding::DeriveCarrier) {
				return quote! {
					/// # Safety
					/// `__rec` must be a spilled record's frame, of the layout
					/// `__reads` was resolved against; the token rebinds it.
					unsafe fn #read_fn<'__read>(__rec: #core_types::record::Rec<'_>, __reads: &[Option<usize>]) -> (#core_types::record::RecordValue<'__read> #(, #attr_tys)*) {
						(#core_types::record::RecordValue::spilled(__rec) #(, #attr_slots)*)
					}
				};
			}
			quote! {
				/// # Safety
				/// `__rec` must be a record whose element is the declared output
				/// type, of the layout `__reads` was resolved against.
				unsafe fn #read_fn<'__read #(, #generics)*>(__rec: #core_types::record::Rec<'_>, __reads: &[Option<usize>]) -> (#output_type #(, #attr_tys)*)
				where
					#output_type: ::core::clone::Clone,
				{
					(unsafe { #core_types::record::read_element::<#output_type>(__rec) } #(, #attr_slots)*)
				}
			}
		})
		.collect();

	Ok(NodePlan {
		kernel,
		lazy_read_fns: quote!(#(#lazy_read_fns)*),
		record_ctor: quote!(#record_wiring #flip_layout_meta_fn),
		node_impl: top_level,
		entries,
		..Default::default()
	})
}
