//! Pilot record nodes exercising the macro's record-tier attribute io:
//! per-input tuple reads resolved against each input's wire, offset writes,
//! `RemoveAttr` layout subtraction, the ElToken byte-carry for passthrough
//! elements, and the `_: ()` no-carrier form. These are the flat-wave law
//! tests; the node forms are the production authoring surface, and the
//! wiring is by hand until the compiler pass constructs layouts.

use core_types::attribute::{Attr, Opacity, RemoveAttr};
use core_types::context::ExtractArena;
use core_types::gpoll::{ErrorKind, GraphError, Interrupt};
use core_types::{Context, Ctx};

core_types::attribute! {
	/// Test-only measured length of an element.
	pub Length("length"): f64;
	/// Test-only label parked in the arena by its writer.
	pub Label("label"): &str;
}

#[node_macro::node(category("Test"))]
fn multiply_opacity(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), factor: f64) -> (f64, Attr<Opacity>) {
	(element, Attr(*opacity * factor))
}

#[node_macro::node(category("Test"))]
fn measure(_: impl Ctx, element: f64) -> (f64, Attr<Length>) {
	(element, Attr(element.abs()))
}

#[node_macro::node(category("Test"))]
fn shade(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>)) -> f64 {
	element * *opacity
}

#[node_macro::node(category("Test"))]
fn checked_multiply_opacity(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), factor: f64) -> Result<(f64, Attr<Opacity>), Interrupt> {
	if factor < 0. {
		return Err(GraphError::new("negative factor").into());
	}
	Ok((element, Attr(*opacity * factor)))
}

#[node_macro::node(category("Test"))]
fn scale(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), factor: &f64) -> (f64, Attr<Opacity>) {
	(element * *factor, Attr(*opacity))
}

#[node_macro::node(category("Test"))]
fn fade<T>(_: impl Ctx, (element, opacity): (T, Attr<Opacity>), factor: f64) -> (T, Attr<Opacity>) {
	(element, Attr(*opacity * factor))
}

#[node_macro::node(category("Test"))]
fn source_opacity(_: impl Ctx, _: (), element: f64, opacity: f64) -> (f64, Attr<Opacity>) {
	(element, Attr(opacity))
}

#[node_macro::node(category("Test"))]
fn label<'e>(ctx: impl Ctx + ExtractArena<'e>, (element, label): (f64, Attr<Label>), text: String) -> Result<(f64, Attr<'e, Label>), Interrupt> {
	let joined = format!("{}{text}", *label);
	let (parked, _) = ctx.arena().alloc(joined).ok_or(GraphError {
		kind: ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
	Ok((element, Attr(parked.as_str())))
}

#[node_macro::node(category("Test"))]
fn transfer_opacity(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), (other, other_opacity): (f64, Attr<Opacity>)) -> (f64, Attr<Opacity>) {
	(element + other, Attr(*opacity * *other_opacity))
}

#[node_macro::node(category("Test"))]
fn strip_opacity<T>(_: impl Ctx, element: T) -> (T, RemoveAttr<Opacity>) {
	(element, RemoveAttr::new())
}

#[node_macro::node(category("Test"))]
fn relength(_: impl Ctx, element: f64) -> (f64, RemoveAttr<Opacity>, Attr<Length>) {
	(element, RemoveAttr::new(), Attr(element * 2.))
}

#[node_macro::node(category("Test"))]
fn boost(_: impl Ctx, element: f64, factor: f64) -> f64 {
	element * factor
}

#[node_macro::node(category("Test"))]
fn boost_poll(_: impl Ctx, element: f64, factor: f64) -> core_types::gpoll::GPoll<f64> {
	core_types::gpoll::GPoll::Final(element * factor)
}

#[node_macro::node(category("Test"))]
fn offset(_: impl Ctx, element: f64, by: &f64) -> f64 {
	element + *by
}

#[node_macro::node(category("Test"))]
async fn double_async(_: impl Ctx, element: f64) -> f64 {
	element * 2.
}

#[node_macro::node(category("Test"))]
fn fallback(
	ctx: impl Ctx,
	_: (),
	#[expose] content: impl Node<Context<'_>, Output = (f64, Attr<Opacity>)>,
	#[expose] alternate: impl Node<Context<'_>, Output = f64>,
) -> Result<f64, Interrupt> {
	let (element, opacity) = content.eval(ctx)?;
	Ok(if *opacity > 0. { element } else { alternate.eval(ctx)? })
}

#[node_macro::node(category("Test"))]
fn pick<T>(ctx: impl Ctx, take_second: bool, first: impl Node<Context<'_>, Output = T>, second: impl Node<Context<'_>, Output = T>) -> Result<T, Interrupt> {
	if take_second { second.eval(ctx) } else { first.eval(ctx) }
}

#[node_macro::node(category("Test"))]
fn hold_first<T>(ctx: impl Ctx, take_second: bool, first: impl Node<Context<'_>, Output = T>, second: impl Node<Context<'_>, Output = T>) -> Result<T, Interrupt> {
	let held = first.eval(ctx)?;
	let alt = second.eval(ctx)?;
	Ok(if take_second { alt } else { held })
}

#[node_macro::node(category("Test"))]
fn forward_record<T>(_: impl Ctx, element: T) -> T {
	element
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::arena::Arena;
	use core_types::attribute::Attribute as AttributeMarker;
	use core_types::context::{ContextImpl, EvalScope};
	use core_types::gpoll::GPoll;
	use core_types::node::Node;
	use core_types::record::{Layout, Rec, RecordSource, RecordValue, stack};

	struct ValueNode<T>(T);

	impl<T: Clone, Input> Node<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	struct RecordSourceNode<E> {
		layout: Layout,
		element: E,
		fields: Vec<(usize, f64)>,
		partial: bool,
	}

	impl<'e, E: Copy> Node<ContextImpl<'e>> for RecordSourceNode<E> {
		type Output = RecordValue<'e>;

		fn eval(&self, _input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			let mut value = RecordValue::zeroed();
			let dst = match self.layout.frame_bytes() {
				0 => value.as_mut_ptr(),
				bytes => stack::push(bytes),
			};
			unsafe {
				dst.cast::<E>().write(self.element);
				for (offset, field) in &self.fields {
					dst.add(*offset).cast::<f64>().write(*field);
				}
			}
			if self.layout.frame_bytes() != 0 {
				stack::pop(dst);
				value = RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) });
			}
			match self.partial {
				true => GPoll::Partial(value),
				false => GPoll::Final(value),
			}
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		stack::reserve(1 << 16);
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn f64_layout(names: &[&'static str]) -> Layout {
		let writes: Vec<core_types::record::FieldWrite> = names
			.iter()
			.map(|name| core_types::record::FieldWrite {
				name,
				level: 0,
				size: 8,
				align: 8,
				read_erased: <Opacity as AttributeMarker>::read_erased,
				repark: None,
			})
			.collect();
		Layout::default().with_writes(0, core_types::record::element_write::<f64>(), &writes)
	}

	fn reserve_for(layouts: &[&Layout]) {
		stack::reserve(layouts.iter().map(|layout| layout.frame_bytes()).sum::<usize>().max(1 << 12));
	}

	fn lifted_value<T: Clone + Send + Sync + 'static>(value: T) -> (core_types::record::RecordLift<T, ValueNode<T>>, Layout) {
		let lift = core_types::record::RecordLift::<T, _>::new(ValueNode(value));
		let layout = Node::<ContextImpl>::layout(&lift).unwrap().clone();
		(lift, layout)
	}

	fn bare_source(layout: &Layout, element: f64) -> RecordSourceNode<f64> {
		RecordSourceNode {
			layout: layout.clone(),
			element,
			fields: vec![],
			partial: false,
		}
	}

	#[test]
	fn defaults_then_modify_then_stack() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let stacked = multiply_opacity_layout(&modified);
		reserve_for(&[&source_layout, &modified, &stacked]);

		let chain = MultiplyOpacityNode::new(MultiplyOpacityNode::new(bare_source(&source_layout, 2.), ValueNode(0.5), &source_layout), ValueNode(0.5), &modified);
		assert_eq!(chain.layout(), Some(&stacked));
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = stacked.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 2.);
		assert_eq!(unsafe { rec.read::<f64>(stacked.offset_of(Opacity::NAME, 0).unwrap()) }, 0.25);
	}

	#[test]
	fn tuple_write_element_and_attribute() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let measured = measure_layout(&source_layout);
		reserve_for(&[&source_layout, &measured]);

		let chain = MeasureNode::new(bare_source(&source_layout, -2.), &source_layout);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = measured.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, -2.);
		assert_eq!(unsafe { rec.read::<f64>(measured.offset_of(Length::NAME, 0).unwrap()) }, 2.);
	}

	#[test]
	fn elementwise_write_carries_unrelated_fields() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let measured = measure_layout(&modified);
		reserve_for(&[&source_layout, &modified, &measured]);

		let chain = MeasureNode::new(MultiplyOpacityNode::new(bare_source(&source_layout, -2.), ValueNode(0.5), &source_layout), &modified);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = measured.rec(&value);
		assert_eq!(unsafe { rec.read::<f64>(measured.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);
		assert_eq!(unsafe { rec.read::<f64>(measured.offset_of(Length::NAME, 0).unwrap()) }, 2.);
	}

	#[test]
	fn reads_only_kernel_reads_the_declared_default_and_carries() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let shaded = shade_layout(&modified);
		reserve_for(&[&source_layout, &modified, &shaded]);

		let bare = ShadeNode::new(bare_source(&source_layout, 4.), &source_layout);
		let GPoll::Final(value) = bare.eval(&ctx) else {
			panic!("expected a final record");
		};
		assert_eq!(unsafe { source_layout.rec(&value).element::<f64>() }, 4.);

		let chain = ShadeNode::new(MultiplyOpacityNode::new(bare_source(&source_layout, 4.), ValueNode(0.5), &source_layout), &modified);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = shaded.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 2.);
		assert_eq!(unsafe { rec.read::<f64>(shaded.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);
	}

	#[test]
	fn token_passthrough_carries_any_element_type() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let f64_source = f64_layout(&[]);
		let f64_faded = fade_layout(&f64_source);
		let u32_source = Layout::default().with_writes(0, core_types::record::element_write::<u32>(), &[]);
		let u32_faded = fade_layout(&u32_source);
		reserve_for(&[&f64_source, &f64_faded, &u32_source, &u32_faded]);

		let wide = FadeNode::new(bare_source(&f64_source, 8.), ValueNode(0.5), &f64_source);
		let GPoll::Final(value) = wide.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = f64_faded.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 8.);
		assert_eq!(unsafe { rec.read::<f64>(f64_faded.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);

		let narrow = FadeNode::new(
			RecordSourceNode {
				layout: u32_source.clone(),
				element: 7u32,
				fields: vec![],
				partial: false,
			},
			ValueNode(0.25),
			&u32_source,
		);
		let GPoll::Final(value) = narrow.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = u32_faded.rec(&value);
		assert_eq!(unsafe { rec.element::<u32>() }, 7);
		assert_eq!(unsafe { rec.read::<f64>(u32_faded.offset_of(Opacity::NAME, 0).unwrap()) }, 0.25);
	}

	#[test]
	fn no_carrier_form_writes_a_fresh_record() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = source_opacity_layout();
		reserve_for(&[&layout]);

		let node = SourceOpacityNode::new(ValueNode(3.), ValueNode(0.25));
		assert_eq!(Node::<ContextImpl>::layout(&node), Some(&layout));
		let GPoll::Final(value) = node.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = layout.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 3.);
		assert_eq!(unsafe { rec.read::<f64>(layout.offset_of(Opacity::NAME, 0).unwrap()) }, 0.25);
	}

	#[test]
	fn partial_carrier_downgrades_the_output() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		reserve_for(&[&source_layout, &modified]);

		let chain = MultiplyOpacityNode::new(
			RecordSourceNode {
				layout: source_layout.clone(),
				element: 1.,
				fields: vec![],
				partial: true,
			},
			ValueNode(0.5),
			&source_layout,
		);
		let GPoll::Partial(value) = chain.eval(&ctx) else {
			panic!("expected a partial record");
		};
		assert_eq!(unsafe { modified.rec(&value).read::<f64>(modified.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);
	}

	#[test]
	fn interrupt_kernel_errors_stop_the_eval() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = checked_multiply_opacity_layout(&source_layout);
		reserve_for(&[&source_layout, &modified]);

		let ok = CheckedMultiplyOpacityNode::new(bare_source(&source_layout, 1.), ValueNode(0.5), &source_layout);
		let GPoll::Final(value) = ok.eval(&ctx) else {
			panic!("expected a final record");
		};
		assert_eq!(unsafe { modified.rec(&value).read::<f64>(modified.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);

		let failing = CheckedMultiplyOpacityNode::new(bare_source(&source_layout, 1.), ValueNode(-1.), &source_layout);
		let GPoll::Error(error) = failing.eval(&ctx) else {
			panic!("expected an error");
		};
		assert!(error.kind == "negative factor");
	}

	#[test]
	fn lend_value_params_wire_into_record_kernels() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let scaled = scale_layout(&modified);
		reserve_for(&[&source_layout, &modified, &scaled]);

		let chain = ScaleNode::new(MultiplyOpacityNode::new(bare_source(&source_layout, 2.), ValueNode(0.5), &source_layout), ValueNode(3.), &modified);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = scaled.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 6.);
		assert_eq!(unsafe { rec.read::<f64>(scaled.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);
	}

	#[test]
	fn secondary_input_reads_bind_to_their_own_wire() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let carrier_layout = f64_layout(&["opacity"]);
		let secondary_layout = f64_layout(&["opacity"]);
		let transferred = transfer_opacity_layout(&carrier_layout);
		reserve_for(&[&carrier_layout, &secondary_layout, &transferred]);

		let chain = TransferOpacityNode::new(
			f64_record_source(&carrier_layout, 2., vec![(carrier_layout.offset_of("opacity", 0).unwrap(), 0.5)]),
			f64_record_source(&secondary_layout, 3., vec![(secondary_layout.offset_of("opacity", 0).unwrap(), 0.25)]),
			&carrier_layout,
			&secondary_layout,
		);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = transferred.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 5.);
		assert_eq!(unsafe { rec.read::<f64>(transferred.offset_of(Opacity::NAME, 0).unwrap()) }, 0.125);

		let bare_secondary = f64_layout(&[]);
		let defaulted = TransferOpacityNode::new(
			f64_record_source(&carrier_layout, 2., vec![(carrier_layout.offset_of("opacity", 0).unwrap(), 0.5)]),
			bare_source(&bare_secondary, 3.),
			&carrier_layout,
			&bare_secondary,
		);
		let GPoll::Final(value) = defaulted.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = transferred.rec(&value);
		assert_eq!(unsafe { rec.read::<f64>(transferred.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5, "an absent secondary attribute reads its default");
	}

	#[test]
	fn a_flipped_node_carries_its_primary_inputs_fields() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity"]);
		let factor = core_types::record::RecordLift::<f64, _>::new(ValueNode(3.));
		let factor_layout = Node::<ContextImpl>::layout(&factor).unwrap().clone();
		reserve_for(&[&source_layout]);

		let node = BoostNode::new(
			f64_record_source(&source_layout, 2., vec![(source_layout.offset_of("opacity", 0).unwrap(), 0.25)]),
			factor,
			&source_layout,
			&factor_layout,
		);
		let out_layout = Node::<ContextImpl>::layout(&node).unwrap().clone();
		let opacity_offset = out_layout.offset_of(Opacity::NAME, 0).expect("the primary input's fields pass through to the output");
		let GPoll::Final(value) = node.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = out_layout.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 6.);
		assert_eq!(unsafe { rec.read::<f64>(opacity_offset) }, 0.25);
	}

	#[test]
	fn a_poll_kernel_carries_its_primary_inputs_fields() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity"]);
		let (factor, factor_layout) = lifted_value(3.);
		reserve_for(&[&source_layout]);

		let node = BoostPollNode::new(
			f64_record_source(&source_layout, 2., vec![(source_layout.offset_of("opacity", 0).unwrap(), 0.25)]),
			factor,
			&source_layout,
			&factor_layout,
		);
		let out_layout = Node::<ContextImpl>::layout(&node).unwrap().clone();
		let opacity_offset = out_layout.offset_of(Opacity::NAME, 0).expect("the primary input's fields pass through the poll kernel");
		let GPoll::Final(value) = node.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = out_layout.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 6.);
		assert_eq!(unsafe { rec.read::<f64>(opacity_offset) }, 0.25);
	}

	#[test]
	fn a_byte_carried_spilled_borrow_parks_and_survives_the_carrier_eval() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let carrier_layout = f64_layout(&["opacity"]);
		let by_layout = f64_layout(&["opacity", "length"]);
		assert!(by_layout.frame_bytes() != 0, "the borrow must point into a spilled frame to exercise the park");
		reserve_for(&[&carrier_layout, &by_layout]);

		let node = OffsetNode::new(
			f64_record_source(&carrier_layout, 2., vec![(carrier_layout.offset_of("opacity", 0).unwrap(), 0.25)]),
			f64_record_source(&by_layout, 40., vec![]),
			&carrier_layout,
			&by_layout,
		);
		let out_layout = Node::<ContextImpl>::layout(&node).unwrap().clone();
		let GPoll::Final(value) = node.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = out_layout.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 42., "the parked borrow survives the carrier evaluation reusing its frame");
		assert_eq!(unsafe { rec.read::<f64>(out_layout.offset_of(Opacity::NAME, 0).unwrap()) }, 0.25);
	}

	struct InlineRuntime;

	impl core_types::runtime::Runtime for InlineRuntime {
		fn spawn(&self, _source: SourceId, mut future: core_types::runtime::SourceFuture) -> bool {
			let mut task_ctx = std::task::Context::from_waker(std::task::Waker::noop());
			assert!(future.as_mut().poll(&mut task_ctx).is_ready(), "the inline runtime completes tasks at spawn");
			true
		}
	}

	#[test]
	fn an_async_source_carries_its_primary_inputs_fields_around_the_slot() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity"]);
		let (runtime, runtime_layout) = lifted_value(core_types::runtime::RuntimeHandle(std::sync::Arc::new(InlineRuntime)));
		let (source_id, source_id_layout) = lifted_value(7 as SourceId);
		reserve_for(&[&source_layout]);

		let node = DoubleAsyncNode::new(
			f64_record_source(&source_layout, 3., vec![(source_layout.offset_of("opacity", 0).unwrap(), 0.25)]),
			runtime,
			source_id,
			&source_layout,
			&runtime_layout,
			&source_id_layout,
		);
		let out_layout = Node::<ContextImpl>::layout(&node).unwrap().clone();
		let opacity_offset = out_layout.offset_of(Opacity::NAME, 0).expect("the carrier's fields pass through the async source");

		let GPoll::Final(value) = node.eval(&ctx) else {
			panic!("an inline completion is final on the spawning eval");
		};
		let rec = out_layout.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 6.);
		assert_eq!(unsafe { rec.read::<f64>(opacity_offset) }, 0.25);

		let GPoll::Final(value) = node.eval(&ctx) else {
			panic!("a slot hit is final");
		};
		let rec = out_layout.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 6., "the slot hit replays the element");
		assert_eq!(unsafe { rec.read::<f64>(opacity_offset) }, 0.25, "the fields re-carry on every eval");
	}

	#[test]
	fn lazy_reads_bind_to_their_edge_and_leave_the_untaken_branch_unevaluated() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let unit = core_types::record::RecordLift::<(), _>::new(ValueNode(()));
		let unit_layout = Node::<ContextImpl>::layout(&unit).unwrap().clone();
		let content_layout = f64_layout(&["opacity"]);
		reserve_for(&[&content_layout]);

		let run = |opacity: Option<f64>| {
			let evals = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
			let alternate = core_types::record::RecordLift::<f64, _>::new(CountingValue(evals.clone()));
			let alternate_layout = Node::<ContextImpl>::layout(&alternate).unwrap().clone();
			let (content_layout, fields) = match opacity {
				Some(value) => (content_layout.clone(), vec![(content_layout.offset_of("opacity", 0).unwrap(), value)]),
				None => (f64_layout(&[]), vec![]),
			};
			let node = FallbackNode::new(
				core_types::record::RecordLift::<(), _>::new(ValueNode(())),
				f64_record_source(&content_layout, 7., fields),
				alternate,
				&unit_layout,
				&content_layout,
				&alternate_layout,
			);
			let GPoll::Final(value) = node.eval(&ctx) else {
				panic!("expected a final record");
			};
			let element = unsafe { Node::<ContextImpl>::layout(&node).unwrap().rec(&value).element::<f64>() };
			(element, evals.load(std::sync::atomic::Ordering::Relaxed))
		};

		assert_eq!(run(Some(0.5)), (7., 0), "a visible content skips the alternate branch entirely");
		assert_eq!(run(Some(0.)), (21., 1), "a transparent content evaluates the alternate branch");
		assert_eq!(run(None), (7., 0), "an absent attribute reads its declared default");
	}

	#[test]
	fn remove_attr_leaves_the_layout_and_downstream_reads_the_default() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let stripped = strip_opacity_layout(&modified);
		assert!(stripped.offset_of(Opacity::NAME, 0).is_none(), "the removed name leaves the output layout");
		let shaded = shade_layout(&stripped);
		reserve_for(&[&source_layout, &modified, &stripped, &shaded]);

		let chain = ShadeNode::new(
			StripOpacityNode::new(MultiplyOpacityNode::new(bare_source(&source_layout, 4.), ValueNode(0.5), &source_layout), &modified),
			&stripped,
		);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		assert_eq!(unsafe { shaded.rec(&value).element::<f64>() }, 4., "a read after the removal yields the declared default");
	}

	#[test]
	fn mixed_writes_and_removes_destructure_in_tuple_order() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity", "length"]);
		let relengthed = relength_layout(&source_layout);
		assert!(relengthed.offset_of(Opacity::NAME, 0).is_none());
		reserve_for(&[&source_layout, &relengthed]);

		let chain = RelengthNode::new(
			f64_record_source(
				&source_layout,
				3.,
				vec![(source_layout.offset_of("opacity", 0).unwrap(), 0.25), (source_layout.offset_of("length", 0).unwrap(), 9.)],
			),
			&source_layout,
		);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = relengthed.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 3.);
		assert_eq!(unsafe { rec.read::<f64>(relengthed.offset_of(Length::NAME, 0).unwrap()) }, 6.);
	}

	#[test]
	fn parked_reference_attributes_write_and_carry() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let labeled = label_layout(&source_layout);
		let relabeled = label_layout(&labeled);
		reserve_for(&[&source_layout, &labeled, &relabeled]);

		let chain = LabelNode::new(
			LabelNode::new(bare_source(&source_layout, 1.), ValueNode(String::from("a")), &source_layout),
			ValueNode(String::from("b")),
			&labeled,
		);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = relabeled.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<&str>(relabeled.offset_of(Label::NAME, 0).unwrap()) }, "ab");
	}

	#[test]
	fn census_fills_reference_defaults_from_static_data() {
		let source = f64_layout(&[]);
		let labeled = Layout::default().with_writes(0, core_types::record::element_write::<f64>(), &[core_types::record::FieldWrite::of::<Label>(0)]);

		let plan = core_types::record::SourcePlan::new(&source, &labeled).unwrap();
		let record = [5f64];
		let mut buffer = vec![0u64; labeled.size.div_ceil(8)];
		let translated = unsafe { plan.translate(Rec::new(record.as_ptr().cast()), buffer.as_mut_ptr().cast()) };
		assert_eq!(unsafe { translated.element::<f64>() }, 5.);
		assert_eq!(unsafe { translated.read::<&str>(labeled.offset_of(Label::NAME, 0).unwrap()) }, "");
	}

	fn f64_record_source(layout: &Layout, element: f64, fields: Vec<(usize, f64)>) -> RecordSourceNode<f64> {
		RecordSourceNode {
			layout: layout.clone(),
			element,
			fields,
			partial: false,
		}
	}

	#[test]
	fn record_monitor_forwards_and_captures_for_the_introspection_window() {
		let mut arena = Arena::new(1024).unwrap();
		let generations = [];

		let layout = f64_layout(&["opacity"]);
		reserve_for(&[&layout, &layout]);

		let monitor = crate::memo::MonitorNode::new(f64_record_source(&layout, 4., vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)]), &layout);
		{
			let scope = scope_fixture(&generations, &arena);
			let ctx = ContextImpl::root(&scope);
			let GPoll::Final(value) = monitor.eval(&ctx) else {
				panic!("expected a final record");
			};
			assert_eq!(unsafe { layout.rec(&value).element::<f64>() }, 4.);
		}

		let io = Node::<ContextImpl>::serialize(&monitor).unwrap();
		let io = io
			.downcast_ref::<core_types::memo::IORecord<core_types::context::CtxSnapshot, core_types::record::RecordCapture>>()
			.unwrap();
		let fields = io.output.materialize(&arena).unwrap();
		assert_eq!(fields.len(), 1);
		assert_eq!(fields[0].0, "opacity");
		assert_eq!(*fields[0].1.as_any().downcast_ref::<f64>().unwrap(), 0.25);

		arena.reset();
		assert!(io.output.materialize(&arena).is_none(), "a dead generation materializes to nothing");
	}

	#[test]
	fn routing_unions_branch_layouts_and_fills_census_defaults() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout_a = f64_layout(&["opacity"]);
		let layout_b = f64_layout(&["length"]);
		let union = Layout::union(&[&layout_a, &layout_b]);
		reserve_for(&[&layout_a, &layout_b, &union, &union]);

		let taken = |second: bool| {
			let (condition, condition_layout) = lifted_value(second);
			PickNode::new(
				condition,
				RecordSource::new(f64_record_source(&layout_a, 1., vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)]), &layout_a, &union),
				RecordSource::new(f64_record_source(&layout_b, 3., vec![(layout_b.offset_of("length", 0).unwrap(), 3.)]), &layout_b, &union),
				&union,
				&condition_layout,
			)
		};

		let GPoll::Final(value) = taken(false).eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = union.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 0.5);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("length", 0).unwrap()) }, 0.);

		let GPoll::Final(value) = taken(true).eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = union.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 3.);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 1.);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("length", 0).unwrap()) }, 3.);
	}

	#[test]
	fn routing_provenance_survives_later_evaluations() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout_a = f64_layout(&["opacity"]);
		let layout_b = f64_layout(&["length"]);
		let union = Layout::union(&[&layout_a, &layout_b]);
		reserve_for(&[&layout_a, &layout_b, &union, &union]);

		let (condition, condition_layout) = lifted_value(false);
		let chain = HoldFirstNode::new(
			condition,
			RecordSource::new(f64_record_source(&layout_a, 1., vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)]), &layout_a, &union),
			RecordSource::new(f64_record_source(&layout_b, 3., vec![(layout_b.offset_of("length", 0).unwrap(), 3.)]), &layout_b, &union),
			&union,
			&condition_layout,
		);

		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = union.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 0.5);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("length", 0).unwrap()) }, 0.);
	}

	struct RealTimeProbe {
		layout: Layout,
	}

	impl<'e> Node<ContextImpl<'e>> for RealTimeProbe {
		type Output = RecordValue<'e>;

		fn eval(&self, input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			let element: f64 = match core_types::context::ExtractRealTime::try_real_time(input) {
				Some(_) => 1.,
				None => 0.,
			};
			let mut value = RecordValue::zeroed();
			let dst = match self.layout.frame_bytes() {
				0 => value.as_mut_ptr(),
				bytes => stack::push(bytes),
			};
			unsafe { dst.cast::<f64>().write(element) };
			if self.layout.frame_bytes() != 0 {
				stack::pop(dst);
				value = RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) });
			}
			GPoll::Final(value)
		}
	}

	#[test]
	fn context_modification_nullifies_for_the_inner_record_edge() {
		use core_types::context::{ContextFeatures, ContextModification};

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&[]);
		reserve_for(&[&layout]);

		let probed = |features: ContextFeatures| {
			let (modification, modification_layout) = lifted_value(ContextModification::from_sources(features, &[]));
			let node = crate::context_modification::ContextModificationNode::new(RealTimeProbe { layout: layout.clone() }, modification, &layout, &modification_layout);
			assert_eq!(Node::<ContextImpl>::layout(&node), Some(&layout));
			let GPoll::Final(value) = node.eval(&ctx) else {
				panic!("expected a final record");
			};
			unsafe { layout.rec(&value).element::<f64>() }
		};

		assert_eq!(probed(ContextFeatures::all()), 1., "kept features stay readable under the modification");
		assert_eq!(probed(ContextFeatures::empty()), 0., "nullified features read as absent for the inner edge");
	}

	#[test]
	fn context_modification_forwards_record_partiality() {
		use core_types::context::{ContextFeatures, ContextModification};

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity"]);
		reserve_for(&[&layout]);

		let (modification, modification_layout) = lifted_value(ContextModification::from_sources(ContextFeatures::all(), &[]));
		let node = crate::context_modification::ContextModificationNode::new(
			RecordSourceNode {
				layout: layout.clone(),
				element: 4.,
				fields: vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)],
				partial: true,
			},
			modification,
			&layout,
			&modification_layout,
		);

		let GPoll::Partial(value) = node.eval(&ctx) else {
			panic!("expected a partial record");
		};
		let rec = layout.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 4.);
		assert_eq!(unsafe { rec.read::<f64>(layout.offset_of("opacity", 0).unwrap()) }, 0.25);
	}

	#[test]
	fn droppable_elements_park_and_clone_out() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let lift = core_types::record::RecordLift::<String, _>::new(ValueNode(String::from("parked")));
		let layout = Node::<ContextImpl>::layout(&lift).unwrap().clone();
		let chain = core_types::record::RecordExtract::<String, _>::new(lift, &layout);

		let GPoll::Final(text) = chain.eval(&ctx) else {
			panic!("expected a final value");
		};
		assert_eq!(text, "parked");
	}

	#[test]
	fn identity_layouts_forward_the_record_pointer() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity", "length"]);
		reserve_for(&[&layout]);
		let base = stack::push(0);
		stack::pop(base);

		let chain = ForwardRecordNode::new(
			RecordSource::new(f64_record_source(&layout, 4., vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)]), &layout, &layout.clone()),
			&layout,
		);

		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = layout.rec(&value);
		assert_eq!(rec.ptr(), base.cast_const());
		assert_eq!(unsafe { rec.element::<f64>() }, 4.);
		assert_eq!(unsafe { rec.read::<f64>(layout.offset_of("opacity", 0).unwrap()) }, 0.25);
	}

	#[test]
	fn partial_routing_sources_downgrade_the_output() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity"]);
		reserve_for(&[&layout]);

		let chain = ForwardRecordNode::new(
			RecordSource::new(
				RecordSourceNode {
					layout: layout.clone(),
					element: 4.,
					fields: vec![],
					partial: true,
				},
				&layout,
				&layout.clone(),
			),
			&layout,
		);

		let GPoll::Partial(value) = chain.eval(&ctx) else {
			panic!("expected a partial record");
		};
		assert_eq!(unsafe { layout.rec(&value).element::<f64>() }, 4.);
	}

	struct CountingValue(std::sync::Arc<std::sync::atomic::AtomicU32>);

	impl<Input> Node<Input> for CountingValue {
		type Output = f64;

		fn eval(&self, _input: &Input) -> GPoll<f64> {
			self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			GPoll::Final(21.)
		}
	}

	#[test]
	fn record_memo_replays_the_deep_copy_on_a_context_hit() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let evals = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
		let lift = core_types::record::RecordLift::<f64, _>::new(CountingValue(evals.clone()));
		let layout = Node::<ContextImpl>::layout(&lift).unwrap().clone();
		let memo = crate::memo::MemoizeNode::new(lift, &layout);

		let GPoll::Final(value) = memo.eval(&ctx) else {
			panic!("expected a final record");
		};
		assert_eq!(unsafe { layout.rec(&value).element::<f64>() }, 21.);
		let GPoll::Final(value) = memo.eval(&ctx) else {
			panic!("expected a final record");
		};
		assert_eq!(unsafe { layout.rec(&value).element::<f64>() }, 21.);
		assert_eq!(evals.load(std::sync::atomic::Ordering::Relaxed), 1, "a context hit must not re-evaluate the edge");
	}

	#[test]
	fn record_memo_caches_partial_finality() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity"]);
		reserve_for(&[&layout, &layout]);

		let source = RecordSourceNode {
			layout: layout.clone(),
			element: 4.,
			fields: vec![(layout.offset_of("opacity", 0).unwrap(), 0.5)],
			partial: true,
		};
		let memo = crate::memo::MemoizeNode::new(source, &layout);

		let GPoll::Partial(_) = memo.eval(&ctx) else {
			panic!("expected a partial record");
		};
		let GPoll::Partial(value) = memo.eval(&ctx) else {
			panic!("expected the replay to keep the partial finality");
		};
		assert_eq!(unsafe { layout.rec(&value).read::<f64>(layout.offset_of("opacity", 0).unwrap()) }, 0.5);
	}

	#[test]
	fn record_memo_re_parks_droppable_payloads_on_replay() {
		let generations = [];

		let source_layout = f64_layout(&[]);
		let labeled = label_layout(&source_layout);
		reserve_for(&[&labeled, &labeled]);

		let chain = LabelNode::new(bare_source(&source_layout, 1.), ValueNode(String::from("a")), &source_layout);
		let memo = crate::memo::MemoizeNode::new(chain, &labeled);

		let first_arena = Arena::new(1024).unwrap();
		{
			let scope = scope_fixture(&generations, &first_arena);
			let ctx = ContextImpl::root(&scope);
			let GPoll::Final(_) = memo.eval(&ctx) else {
				panic!("expected a final record");
			};
		}

		let replay_arena = Arena::new(1024).unwrap();
		let scope = scope_fixture(&generations, &replay_arena);
		let ctx = ContextImpl::root(&scope);
		let GPoll::Final(value) = memo.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = labeled.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<&str>(labeled.offset_of(Label::NAME, 0).unwrap()) }, "a");
	}
}
