use super::*;

/// How a record node's primary input lowers: `None` writes a fresh record,
/// `Token` carries the element bytes through as `ElToken`, `Read` reads a
/// concrete element at offset 0, and `LazyToken` is a derive-routing carrier
/// the kernel evaluates itself, returning the row token it received. The
/// element and write set fold from the IR.
#[derive(Clone)]
pub(crate) enum RecordCarrier {
	None,
	Token,
	Read,
	LazyToken,
}

/// A well-formed record-io node: only the carrier form is retained, so
/// [`skips_carrier`] can gate the fresh-record path. Malformed record io yields
/// `None` from [`record_shape`] and generates no node impl.
#[derive(Clone)]
pub(crate) struct RecordShape {
	pub(crate) carrier: RecordCarrier,
}

impl RecordShape {
	pub(crate) fn skips_carrier(&self) -> bool {
		matches!(self.carrier, RecordCarrier::None)
	}
}

/// The effect/return axis of a node's kernel, resolved once from the signature.
/// It selects the eval tail (finish / merge / spawn) and the kernel signature
/// wrapping across every node kind.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Dialect {
	Sync,
	Interrupt,
	Poll,
	AsyncFn,
	Future,
	FutureInterrupt,
}

pub(crate) fn dialect(parsed: &ParsedNodeFn) -> Dialect {
	if parsed.is_async {
		return Dialect::AsyncFn;
	}
	match kernel_kind(&parsed.output_type) {
		KernelKind::Plain => Dialect::Sync,
		KernelKind::Interrupt(_) => Dialect::Interrupt,
		KernelKind::Poll(_) => Dialect::Poll,
		KernelKind::Future(_) => Dialect::Future,
		KernelKind::FutureInterrupt(_) => Dialect::FutureInterrupt,
	}
}

/// The dialect of a node fn that lowers to a `Node` impl, or `None` when no
/// lowering supports the signature (an async node with lazy inputs, malformed
/// record io, or a shape no kind accepts). The kind itself is derived from the
/// intent IR ([`crate::codegen::ir::node_kind`]); this only gates support.
pub(crate) fn analyze(parsed: &ParsedNodeFn) -> Option<Dialect> {
	if parsed.is_async && parsed.fields.iter().any(|field| matches!(field.ty, ParsedFieldType::Node(_))) {
		return None;
	}
	let supported = if record_shape(parsed).is_some() {
		true
	} else if has_record_io(parsed) {
		return None;
	} else {
		routing_io(parsed).is_some() || record_flip(parsed) || record_opaque(parsed) || has_materialized_input(parsed)
	};
	supported.then(|| dialect(parsed))
}

/// The tail form of a node's eval, selected from its class and dialect: forward
/// the kernel's own record, assemble a record (io or flip carrier), or spawn a
/// source and lift its completion.
#[derive(Clone, Copy)]
pub(crate) enum Tail {
	Forward,
	Record,
	Flip,
	SpawnAsyncFn,
	SpawnFuture,
}

/// One statement group of a node's `eval` body, lowered in order: the input
/// binds first (one per input), then the numeric clamps, then the tail that
/// assembles the output record and closes the dialect.
pub(crate) enum EvalStep<'a> {
	Bind(usize, &'a ParsedField),
	Clamp(&'a ParsedField),
	Tail(Tail),
}

/// Whether the signature declares record-tier attribute io: value-input reads
/// or return-tuple writes. Reads on lazy inputs belong to the record lowering
/// of the flip class instead.
pub(crate) fn has_record_io(parsed: &ParsedNodeFn) -> bool {
	let value_reads = parsed.fields.iter().any(|field| !field.attribute_reads.is_empty() && matches!(field.ty, ParsedFieldType::Regular(_)));
	value_reads || record_writes(&slot_value_type(&parsed.output_type)).is_some()
}

pub(crate) fn has_lazy_reads(parsed: &ParsedNodeFn) -> bool {
	parsed.fields.iter().any(|field| !field.attribute_reads.is_empty() && matches!(field.ty, ParsedFieldType::Node(_)))
}

/// The value inputs of a routing node (every regular field that is neither a
/// routing source nor a ranked whole-list input), with their indices into the
/// regular fields.
pub(crate) fn routing_value_indices(regular_fields: &[&ParsedField], generic: &Ident) -> Vec<usize> {
	regular_fields
		.iter()
		.enumerate()
		.filter(|(_, field)| match &field.ty {
			ParsedFieldType::Regular(RegularParsedField { ty, list_levels, .. }) => *list_levels == 0 && !matches!(ty, Type::Path(path) if path.path.get_ident() == Some(generic)),
			ParsedFieldType::Node(_) => false,
		})
		.map(|(index, _)| index)
		.collect()
}

/// The lazy inputs declaring attribute reads, with their indices into the
/// unit-skipped regular fields.
pub(crate) fn lazy_read_fields<'a>(regular_fields: &[&'a ParsedField]) -> Vec<(usize, &'a ParsedField)> {
	regular_fields
		.iter()
		.enumerate()
		.filter(|(_, field)| matches!(field.ty, ParsedFieldType::Node(_)) && !field.attribute_reads.is_empty())
		.map(|(index, field)| (index, *field))
		.collect()
}

/// The indices (into the unit-skipped regular fields) of value inputs whose
/// reads resolve against their own wire rather than the carrier's.
pub(crate) fn reading_secondary_indices(regular_fields: &[&ParsedField], skips_carrier: bool) -> Vec<usize> {
	regular_fields
		.iter()
		.enumerate()
		.filter(|(index, field)| !field.attribute_reads.is_empty() && (skips_carrier || *index != 0))
		.map(|(index, _)| index)
		.collect()
}

/// Every attribute read in field order with the owning field's index, flat so
/// read slots are numbered across inputs.
pub(crate) fn field_reads<'a>(regular_fields: &[&'a ParsedField]) -> Vec<(usize, &'a AttributeRead)> {
	regular_fields
		.iter()
		.enumerate()
		.flat_map(|(index, field)| field.attribute_reads.iter().map(move |read| (index, read)))
		.collect()
}

/// Substitutes bare generic idents with their row-assigned types.
pub(crate) fn substitute_ident_types(ty: &Type, assignments: &[(Ident, Type)]) -> Type {
	struct Subst<'a> {
		assignments: &'a [(Ident, Type)],
	}

	impl VisitMut for Subst<'_> {
		fn visit_type_mut(&mut self, ty: &mut Type) {
			if let Type::Path(path) = ty
				&& path.qself.is_none()
				&& let Some(ident) = path.path.get_ident()
				&& let Some((_, replacement)) = self.assignments.iter().find(|(generic, _)| generic == ident)
			{
				*ty = replacement.clone();
				return;
			}
			syn::visit_mut::visit_type_mut(self, ty);
		}
	}

	let mut ty = ty.clone();
	Subst { assignments }.visit_type_mut(&mut ty);
	ty
}

/// Replaces the routing generic in a derive-routing kernel's return type with
/// the routing record value, since the kernel's edges rebind to '__record.
pub(crate) fn substitute_routing_record(output: &Type, generic: &Ident, core_types: &TokenStream2) -> Type {
	struct Subst<'a> {
		generic: &'a Ident,
		replacement: Type,
	}

	impl VisitMut for Subst<'_> {
		fn visit_type_mut(&mut self, ty: &mut Type) {
			if let Type::Path(path) = ty
				&& path.qself.is_none()
				&& path.path.get_ident() == Some(self.generic)
			{
				*ty = self.replacement.clone();
				return;
			}
			syn::visit_mut::visit_type_mut(self, ty);
		}
	}

	let mut ty = output.clone();
	let mut subst = Subst {
		generic,
		replacement: syn::parse_quote!(#core_types::record::RecordValue<'__record>),
	};
	subst.visit_type_mut(&mut ty);
	ty
}

pub(crate) fn inject_attr_lifetimes(output: &Type) -> Option<Type> {
	struct Injector {
		changed: bool,
	}

	impl VisitMut for Injector {
		fn visit_path_segment_mut(&mut self, segment: &mut syn::PathSegment) {
			if segment.ident == "Attr"
				&& let PathArguments::AngleBracketed(args) = &mut segment.arguments
				&& !args.args.iter().any(|arg| matches!(arg, GenericArgument::Lifetime(_)))
			{
				args.args.insert(0, GenericArgument::Lifetime(Lifetime::new("'__attr", proc_macro2::Span::call_site())));
				self.changed = true;
			}
			syn::visit_mut::visit_path_segment_mut(self, segment);
		}
	}

	let mut ty = output.clone();
	let mut injector = Injector { changed: false };
	injector.visit_type_mut(&mut ty);
	injector.changed.then_some(ty)
}

pub(crate) fn contains_open_generic(parsed: &ParsedNodeFn, ty: &Type) -> bool {
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	parsed
		.fn_generics
		.iter()
		.any(|param| matches!(param, GenericParam::Type(type_param) if Some(&type_param.ident) != ctx_ident.as_ref() && type_contains_ident(ty, &type_param.ident)))
}

pub(crate) fn unbounded_generic(parsed: &ParsedNodeFn, ty: &Type) -> Option<Ident> {
	let ident = bare_ident(ty)?.clone();
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	parsed
		.fn_generics
		.iter()
		.find(|param| matches!(param, GenericParam::Type(type_param) if type_param.ident == ident && type_param.bounds.is_empty() && Some(&type_param.ident) != ctx_ident.as_ref()))?;
	if let Some(where_clause) = &parsed.where_clause
		&& tokens_contain_ident(where_clause.to_token_stream(), &ident)
	{
		return None;
	}
	Some(ident)
}

pub(crate) fn record_shape(parsed: &ParsedNodeFn) -> Option<RecordShape> {
	let value = match kernel_kind(&parsed.output_type) {
		KernelKind::Plain => parsed.output_type.clone(),
		KernelKind::Interrupt(inner) => inner,
		_ => return None,
	};
	let writes = record_writes(&value);
	let has_reads = parsed.fields.iter().any(|field| !field.attribute_reads.is_empty() && matches!(field.ty, ParsedFieldType::Regular(_)));
	if !has_reads && writes.is_none() {
		return None;
	}
	if parsed.is_async {
		return None;
	}
	let carrier_field = parsed.fields.first()?;
	if carrier_field.is_data_field {
		return None;
	}
	// A first-field lazy carrier: the kernel evaluates the derived content
	// itself and returns its opaque row token beside the write set.
	let lazy_carrier = matches!(&carrier_field.ty, ParsedFieldType::Node(_));
	// Lazy secondaries are consumed as plain elements; raw record edges and
	// ranked outputs have no element binding here.
	let unsupported_lazy_secondary = |field: &ParsedField| match &field.ty {
		ParsedFieldType::Node(NodeParsedField { output_type, .. }) => is_record_value(output_type) || crate::codegen::ir::strip_ilist(output_type).1 > 0 || !field.attribute_reads.is_empty(),
		ParsedFieldType::Regular(_) => false,
	};
	if parsed.fields.iter().skip(lazy_carrier as usize).any(|field| unsupported_lazy_secondary(field)) {
		return None;
	}
	let reads_well_placed = parsed.fields.iter().enumerate().all(|(index, field)| {
		field.attribute_reads.is_empty() || (lazy_carrier && index == 0) || (!field.is_data_field && matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { lend: None, .. })))
	});
	if !reads_well_placed {
		return None;
	}
	if lazy_carrier {
		let ParsedFieldType::Node(NodeParsedField { output_type, .. }) = &carrier_field.ty else {
			unreachable!("guarded by the lazy_carrier match");
		};
		let token = unbounded_generic(parsed, output_type)?;
		let element = match writes {
			Some(RecordWrites { element, .. }) => element,
			None => value,
		};
		if !matches!(bare_ident(&element), Some(ident) if ident == &token) {
			return None;
		}
		return Some(RecordShape { carrier: RecordCarrier::LazyToken });
	}
	let ParsedFieldType::Regular(RegularParsedField { ty, lend: None, implementations, .. }) = &carrier_field.ty else {
		return None;
	};
	let token = match ty {
		Type::Tuple(tuple) if tuple.elems.is_empty() => None,
		ty => match implementations.is_empty().then(|| unbounded_generic(parsed, ty)).flatten() {
			Some(token) => Some(token),
			None => {
				if contains_open_generic(parsed, ty) {
					return None;
				}
				None
			}
		},
	};
	let carrier = match ty {
		Type::Tuple(tuple) if tuple.elems.is_empty() => RecordCarrier::None,
		_ if token.is_some() => RecordCarrier::Token,
		_ => RecordCarrier::Read,
	};
	let (element, _, removes) = match writes {
		Some(RecordWrites { element, markers, removes }) => (element, markers, removes),
		None => (value, Vec::new(), Vec::new()),
	};
	match &token {
		Some(token) => {
			if !matches!(bare_ident(&element), Some(ident) if ident == token) {
				return None;
			}
		}
		None => {
			if contains_open_generic(parsed, &element) {
				return None;
			}
		}
	}
	if matches!(carrier, RecordCarrier::None) && !removes.is_empty() {
		return None;
	}
	Some(RecordShape { carrier })
}

pub(crate) fn is_poll_kernel(output: &Type) -> bool {
	matches!(kernel_kind(output), KernelKind::Poll(_))
}

/// A routing family: an unbounded generic shared by lazy inputs (and
/// optionally the first parameter) and returned whole, instantiated at
/// `RecordValue` so opaque records flow through the kernel. Detected only
/// when the family's fields carry no implementations lists, so the existing
/// per-type row spelling keeps its meaning.
#[derive(Clone)]
pub(crate) struct RoutingIo {
	pub(crate) generic: Ident,
}

/// Whether a flipped node's primary input is a carrier: the first parameter
/// after the context, when it is an owned or lent value input. A carrier's
/// fields pass through to the output; every production layout is element-only
/// until attribute adoption, so the copy plan is empty and behavior is
/// unchanged. Async kernels carry fields per eval around the slot (only the
/// element crosses the future boundary), so their carrier must be owned: the
/// future captures the element by value.
pub(crate) fn flip_carrier(parsed: &ParsedNodeFn) -> bool {
	if !record_flip(parsed) {
		return false;
	}
	let Some(first) = parsed.fields.first() else { return false };
	if first.is_data_field {
		return false;
	}
	let ParsedFieldType::Regular(RegularParsedField { ty, lend, .. }) = &first.ty else {
		return false;
	};
	if matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty()) {
		return false;
	}
	let async_kernel = parsed.is_async || matches!(kernel_kind(&parsed.output_type), KernelKind::Future(_) | KernelKind::FutureInterrupt(_));
	!(async_kernel && lend.is_some())
}

/// Whether a plain node's lowering flips onto record wires: sync,
/// fully-concrete value-input nodes in this cut; batch, shader, async, lend,
/// lazy, and generic nodes keep the plain lowering until their record forms
/// land.
pub(crate) fn has_materialized_input(parsed: &ParsedNodeFn) -> bool {
	parsed
		.fields
		.iter()
		.any(|field| matches!(&field.ty, ParsedFieldType::Regular(RegularParsedField { list_levels, .. }) if *list_levels > 0))
}

pub(crate) fn record_flip(parsed: &ParsedNodeFn) -> bool {
	if record_shape(parsed).is_some() || has_record_io(parsed) || routing_io(parsed).is_some() {
		return false;
	}
	// Shader nodes flip like any value node: the kernel doubles as the
	// shader body on the spirv target, but the struct and Node impl are
	// std-gated, so the record machinery never reaches the shader build.
	if parsed.attributes.batch.is_some() || parsed.attributes.plain {
		return false;
	}
	if type_disqualifies(&slot_value_type(&parsed.output_type)) {
		return false;
	}
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	for param in &parsed.fn_generics {
		match param {
			GenericParam::Type(type_param) if Some(&type_param.ident) == ctx_ident.as_ref() => {}
			// Registry rows assign a generic by unifying a field's type with
			// the row's, so a generic without an extractable position keeps
			// the plain lowering. A `skip_impl` node's rows are hand-written
			// with explicit types, so no extractable position is needed.
			GenericParam::Type(type_param) => {
				let extractable = parsed.fields.iter().filter(|field| !field.is_data_field).any(|field| {
					let ty = match &field.ty {
						ParsedFieldType::Regular(RegularParsedField { ty, .. }) => ty,
						ParsedFieldType::Node(NodeParsedField { output_type, .. }) => output_type,
					};
					generic_extractable(ty, &type_param.ident)
				});
				if !extractable && !parsed.attributes.skip_impl {
					return false;
				}
			}
			GenericParam::Lifetime(_) | GenericParam::Const(_) => return false,
		}
	}
	true
}

/// Whether unifying a value of `field_ty`'s shape can bind `generic`: the
/// generic sits bare or under path type arguments, the shapes
/// [`generic_assignment`] walks.
pub(crate) fn generic_extractable(field_ty: &Type, generic: &Ident) -> bool {
	match field_ty {
		Type::Path(path) if path.qself.is_none() && path.path.get_ident() == Some(generic) => true,
		Type::Path(path) => path.path.segments.iter().any(|segment| match &segment.arguments {
			PathArguments::AngleBracketed(args) => args.args.iter().any(|argument| match argument {
				GenericArgument::Type(inner) => generic_extractable(inner, generic),
				_ => false,
			}),
			_ => false,
		}),
		_ => false,
	}
}

/// Binds `generic` by unifying `field_ty` against `row_ty`: where the field
/// names the generic, the row's corresponding subtree is the assignment.
pub(crate) fn generic_assignment(field_ty: &Type, row_ty: &Type, generic: &Ident) -> Option<Type> {
	if matches!(field_ty, Type::Path(path) if path.qself.is_none() && path.path.get_ident() == Some(generic)) {
		return Some(row_ty.clone());
	}
	let (Type::Path(field_path), Type::Path(row_path)) = (field_ty, row_ty) else {
		return None;
	};
	let field_segment = field_path.path.segments.last()?;
	let row_segment = row_path.path.segments.last()?;
	let (PathArguments::AngleBracketed(field_args), PathArguments::AngleBracketed(row_args)) = (&field_segment.arguments, &row_segment.arguments) else {
		return None;
	};
	field_args.args.iter().zip(row_args.args.iter()).find_map(|(field_arg, row_arg)| match (field_arg, row_arg) {
		(GenericArgument::Type(field_inner), GenericArgument::Type(row_inner)) => generic_assignment(field_inner, row_inner, generic),
		_ => None,
	})
}

pub(crate) fn is_record_value(ty: &Type) -> bool {
	matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "RecordValue"))
}

/// Whether a kernel operates on whole records: it names `RecordValue` in its
/// output, receives raw record edges paired with the node's layout, and
/// takes on the record APIs' unsafe contracts itself.
pub(crate) fn record_opaque(parsed: &ParsedNodeFn) -> bool {
	is_record_value(&slot_value_type(&parsed.output_type))
}

pub(crate) fn routing_io(parsed: &ParsedNodeFn) -> Option<RoutingIo> {
	if has_record_io(parsed) || parsed.is_async {
		return None;
	}
	if !matches!(kernel_kind(&parsed.output_type), KernelKind::Plain | KernelKind::Interrupt(_)) {
		return None;
	}
	let value = slot_value_type(&parsed.output_type);
	let Type::Path(path) = &value else { return None };
	let ident = path.path.get_ident()?.clone();
	let ctx_ident = context_param(parsed).map(|ctx| ctx.ident.clone());
	parsed
		.fn_generics
		.iter()
		.find(|param| matches!(param, GenericParam::Type(type_param) if type_param.ident == ident && type_param.bounds.is_empty() && Some(&type_param.ident) != ctx_ident.as_ref()))?;
	if let Some(where_clause) = &parsed.where_clause
		&& tokens_contain_ident(where_clause.to_token_stream(), &ident)
	{
		return None;
	}
	let mut sources = 0;
	for (index, field) in parsed.fields.iter().enumerate() {
		match &field.ty {
			ParsedFieldType::Node(NodeParsedField {
				output_type,
				input_type,
				implementations,
			}) => {
				if routing_source_output(output_type, &ident) {
					// A source forwards its whole record opaquely; declared
					// reads contradict that and are rejected by validation.
					if !implementations.is_empty() || type_contains_ident(input_type, &ident) || !field.attribute_reads.is_empty() {
						return None;
					}
					sources += 1;
				} else if type_contains_ident(output_type, &ident) || type_contains_ident(input_type, &ident) {
					return None;
				}
			}
			ParsedFieldType::Regular(RegularParsedField { ty, implementations, lend, .. }) => {
				if bare_ident(ty) == Some(&ident) {
					if index != 0 || field.is_data_field || !implementations.is_empty() || lend.is_some() {
						return None;
					}
					sources += 1;
				} else if type_contains_ident(ty, &ident) {
					return None;
				}
			}
		}
	}
	(sources > 0).then(|| RoutingIo { generic: ident })
}

pub(crate) fn bare_ident(ty: &Type) -> Option<&Ident> {
	let Type::Path(path) = ty else { return None };
	path.path.get_ident()
}

/// Whether a lazy input's declared output is the routing generic, at any
/// `IList` nesting: the nesting is rank depth, not a distinct row type.
pub(crate) fn routing_source_output(output_type: &Type, generic: &Ident) -> bool {
	let (stripped, _) = crate::codegen::ir::strip_ilist(output_type);
	bare_ident(&stripped) == Some(generic)
}

pub(crate) fn tokens_contain_ident(tokens: TokenStream2, ident: &Ident) -> bool {
	tokens.into_iter().any(|token| match token {
		proc_macro2::TokenTree::Ident(candidate) => &candidate == ident,
		proc_macro2::TokenTree::Group(group) => tokens_contain_ident(group.stream(), ident),
		_ => false,
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

pub(crate) enum KernelKind {
	Plain,
	Interrupt(Type),
	Poll(Type),
	Future(Type),
	FutureInterrupt(Type),
}

pub(crate) fn source_future_payload(segment: &syn::PathSegment) -> Type {
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

pub(crate) fn kernel_kind(output: &Type) -> KernelKind {
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
			if error_path.path.segments.last().is_none_or(|segment| segment.ident != "Interrupt") {
				return plain();
			}
			if let Type::Path(inner_path) = inner
				&& let Some(inner_segment) = inner_path.path.segments.last()
				&& inner_segment.ident == "SourceFuture"
			{
				return KernelKind::FutureInterrupt(source_future_payload(inner_segment));
			}
			KernelKind::Interrupt(inner.clone())
		}
		_ => plain(),
	}
}

pub(crate) fn context_param(parsed: &ParsedNodeFn) -> Option<&TypeParam> {
	let Type::Path(path) = &parsed.input.ty else {
		return None;
	};
	let ident = path.path.get_ident()?;
	parsed.fn_generics.iter().find_map(|param| match param {
		GenericParam::Type(type_param) if &type_param.ident == ident => Some(type_param),
		_ => None,
	})
}

pub(crate) fn type_disqualifies(ty: &Type) -> bool {
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

pub(crate) fn desugar_extract_lifetime(bound: &TypeParamBound, core_types: &TokenStream2) -> TokenStream2 {
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
