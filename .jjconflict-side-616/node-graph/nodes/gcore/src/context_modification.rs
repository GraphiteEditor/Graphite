use core_types::context::{ContextFeatures, ContextModification, Ctx, DeriveCtx, IndexLink, nullify_index_levels};
use core_types::gpoll::{ErrorKind, GraphError, Interrupt};

/// Filters out what should be unused components of the context based on the specified requirements.
/// This node is inserted by the compiler to "zero out" unused context components.
#[node_macro::node(category(""))]
fn context_modification<T>(
	ctx: impl Ctx + DeriveCtx,
	/// The data to pass through, evaluated with the stripped down context.
	value: impl Node<Context<'_>, Output = T>,
	/// The parts of the context to keep when evaluating the input value. All other parts are nullified.
	modification: ContextModification,
) -> Result<T, Interrupt> {
	let scope = ctx.scope().nullified(modification.features, Some(modification.sources()));
	let exhausted = || {
		Interrupt::from(GraphError {
			kind: ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	let index = match modification.features.contains(ContextFeatures::INDEX) {
		true => nullify_index_levels(ctx.index_head(), modification.index_levels, scope.arena()).ok_or_else(exhausted)?,
		false => IndexLink { index: 0, outer: None },
	};
	value.eval(&ctx.nullified(modification.features, index, &scope))
}
