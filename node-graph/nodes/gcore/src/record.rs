//! Pilot record nodes exercising the macro's record-tier attribute io:
//! offset reads and writes against record edges, the ElToken byte-carry for
//! passthrough elements, and the `_: ()` no-carrier form. These are the
//! flat-wave law tests; the node forms are the production authoring surface,
//! and the wiring is by hand until the compiler pass constructs layouts.

use core_types::attribute::{Attr, Opacity};
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
fn multiply_opacity(_: impl Ctx, element: f64, factor: f64, opacity: Attr<Opacity>) -> (f64, Attr<Opacity>) {
	(element, Attr(*opacity * factor))
}

#[node_macro::node(category("Test"))]
fn measure(_: impl Ctx, element: f64) -> (f64, Attr<Length>) {
	(element, Attr(element.abs()))
}

#[node_macro::node(category("Test"))]
fn shade(_: impl Ctx, element: f64, opacity: Attr<Opacity>) -> f64 {
	element * *opacity
}

#[node_macro::node(category("Test"))]
fn checked_multiply_opacity(_: impl Ctx, element: f64, factor: f64, opacity: Attr<Opacity>) -> Result<(f64, Attr<Opacity>), Interrupt> {
	if factor < 0. {
		return Err(GraphError::new("negative factor").into());
	}
	Ok((element, Attr(*opacity * factor)))
}

#[node_macro::node(category("Test"))]
fn scale(_: impl Ctx, element: f64, factor: &f64, opacity: Attr<Opacity>) -> (f64, Attr<Opacity>) {
	(element * *factor, Attr(*opacity))
}

#[node_macro::node(category("Test"))]
fn fade<T>(_: impl Ctx, element: T, factor: f64, opacity: Attr<Opacity>) -> (T, Attr<Opacity>) {
	(element, Attr(*opacity * factor))
}

#[node_macro::node(category("Test"))]
fn source_opacity(_: impl Ctx, _: (), element: f64, opacity: f64) -> (f64, Attr<Opacity>) {
	(element, Attr(opacity))
}

#[node_macro::node(category("Test"))]
fn label<'e>(ctx: impl Ctx + ExtractArena<'e>, element: f64, text: String, label: Attr<Label>) -> Result<(f64, Attr<'e, Label>), Interrupt> {
	let joined = format!("{}{text}", *label);
	let (parked, _) = ctx.arena().alloc(joined).ok_or(GraphError {
		kind: ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
	Ok((element, Attr(parked.as_str())))
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
		frame_bytes: usize,
		element: E,
		fields: Vec<(usize, f64)>,
		partial: bool,
	}

	impl<'e, E: Copy> Node<ContextImpl<'e>> for RecordSourceNode<E> {
		type Output = RecordValue<'e>;

		fn eval(&self, _input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			let dst = stack::push(self.frame_bytes);
			let value = unsafe {
				dst.cast::<E>().write(self.element);
				for (offset, value) in &self.fields {
					dst.add(*offset).cast::<f64>().write(*value);
				}
				RecordValue::from_rec(Rec::new(dst))
			};
			stack::pop(dst);
			match self.partial {
				true => GPoll::Partial(value),
				false => GPoll::Final(value),
			}
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
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
			})
			.collect();
		Layout::default().with_writes(0, (8, 8), &writes)
	}

	fn frame_bytes(layout: &Layout) -> usize {
		layout.size.next_multiple_of(8)
	}

	fn reserve_for(layouts: &[&Layout]) {
		stack::reserve(layouts.iter().map(|layout| frame_bytes(layout)).sum());
	}

	fn bare_source(layout: &Layout, element: f64) -> RecordSourceNode<f64> {
		RecordSourceNode {
			frame_bytes: frame_bytes(layout),
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

		let chain = MultiplyOpacityNode::new(
			MultiplyOpacityNode::new(bare_source(&source_layout, 2.), ValueNode(0.5), &source_layout),
			ValueNode(0.5),
			&modified,
		);
		assert_eq!(chain.layout(), Some(&stacked));
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
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
		let rec = value.rec();
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
		let rec = value.rec();
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
		assert_eq!(unsafe { value.rec().element::<f64>() }, 4.);

		let chain = ShadeNode::new(MultiplyOpacityNode::new(bare_source(&source_layout, 4.), ValueNode(0.5), &source_layout), &modified);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
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
		let u32_source = Layout::default().with_writes(0, (4, 4), &[]);
		let u32_faded = fade_layout(&u32_source);
		reserve_for(&[&f64_source, &f64_faded, &u32_source, &u32_faded]);

		let wide = FadeNode::new(bare_source(&f64_source, 8.), ValueNode(0.5), &f64_source);
		let GPoll::Final(value) = wide.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
		assert_eq!(unsafe { rec.element::<f64>() }, 8.);
		assert_eq!(unsafe { rec.read::<f64>(f64_faded.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);

		let narrow = FadeNode::new(
			RecordSourceNode {
				frame_bytes: frame_bytes(&u32_source),
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
		let rec = value.rec();
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
		let rec = value.rec();
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
				frame_bytes: frame_bytes(&source_layout),
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
		assert_eq!(unsafe { value.rec().read::<f64>(modified.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);
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
		assert_eq!(unsafe { value.rec().read::<f64>(modified.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);

		let failing = CheckedMultiplyOpacityNode::new(bare_source(&source_layout, 1.), ValueNode(-1.), &source_layout);
		let GPoll::Error(error) = failing.eval(&ctx) else {
			panic!("expected an error");
		};
		assert!(error.kind == "negative factor");
	}

	static FACTOR: f64 = 3.;

	struct StaticLendNode(&'static f64);

	impl<'e> Node<ContextImpl<'e>> for StaticLendNode {
		type Output = &'e f64;

		fn eval(&self, _input: &ContextImpl<'e>) -> GPoll<&'e f64> {
			GPoll::Final(self.0)
		}
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

		let chain = ScaleNode::new(
			MultiplyOpacityNode::new(bare_source(&source_layout, 2.), ValueNode(0.5), &source_layout),
			StaticLendNode(&FACTOR),
			&modified,
		);
		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
		assert_eq!(unsafe { rec.element::<f64>() }, 6.);
		assert_eq!(unsafe { rec.read::<f64>(scaled.offset_of(Opacity::NAME, 0).unwrap()) }, 0.5);
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
		let rec = value.rec();
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<&str>(relabeled.offset_of(Label::NAME, 0).unwrap()) }, "ab");
	}

	#[test]
	fn census_fills_reference_defaults_from_static_data() {
		let source = f64_layout(&[]);
		let labeled = Layout::default().with_writes(0, (8, 8), &[core_types::record::FieldWrite::of::<Label>(0)]);

		let plan = core_types::record::SourcePlan::new(&source, &labeled).unwrap();
		let record = [5f64];
		let mut buffer = vec![0u64; labeled.size.div_ceil(8)];
		let translated = unsafe { plan.translate(Rec::new(record.as_ptr().cast()), buffer.as_mut_ptr().cast()) };
		assert_eq!(unsafe { translated.element::<f64>() }, 5.);
		assert_eq!(unsafe { translated.read::<&str>(labeled.offset_of(Label::NAME, 0).unwrap()) }, "");
	}

	fn f64_record_source(layout: &Layout, element: f64, fields: Vec<(usize, f64)>) -> RecordSourceNode<f64> {
		RecordSourceNode {
			frame_bytes: frame_bytes(layout),
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

		let monitor = core_types::record::RecordMonitor::new(f64_record_source(&layout, 4., vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)]), &layout);
		{
			let scope = scope_fixture(&generations, &arena);
			let ctx = ContextImpl::root(&scope);
			let GPoll::Final(value) = monitor.eval(&ctx) else {
				panic!("expected a final record");
			};
			assert_eq!(unsafe { value.rec().element::<f64>() }, 4.);
		}

		let capture = Node::<ContextImpl>::serialize(&monitor).unwrap();
		let capture = capture.downcast_ref::<core_types::record::RecordCapture>().unwrap();
		let fields = capture.materialize(&arena).unwrap();
		assert_eq!(fields.len(), 1);
		assert_eq!(fields[0].0, "opacity");
		assert_eq!(*fields[0].1.as_any().downcast_ref::<f64>().unwrap(), 0.25);

		arena.reset();
		assert!(capture.materialize(&arena).is_none(), "a dead generation materializes to nothing");
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
			PickNode::new(
				ValueNode(second),
				RecordSource::new(f64_record_source(&layout_a, 1., vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)]), &layout_a, &union),
				RecordSource::new(f64_record_source(&layout_b, 3., vec![(layout_b.offset_of("length", 0).unwrap(), 3.)]), &layout_b, &union),
			)
		};

		let GPoll::Final(value) = taken(false).eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 0.5);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("length", 0).unwrap()) }, 0.);

		let GPoll::Final(value) = taken(true).eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
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

		let chain = HoldFirstNode::new(
			ValueNode(false),
			RecordSource::new(f64_record_source(&layout_a, 1., vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)]), &layout_a, &union),
			RecordSource::new(f64_record_source(&layout_b, 3., vec![(layout_b.offset_of("length", 0).unwrap(), 3.)]), &layout_b, &union),
		);

		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 0.5);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("length", 0).unwrap()) }, 0.);
	}

	#[test]
	fn identity_layouts_forward_the_record_pointer() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity"]);
		reserve_for(&[&layout]);
		let base = stack::push(0);
		stack::pop(base);

		let chain = ForwardRecordNode::new(RecordSource::new(
			f64_record_source(&layout, 4., vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)]),
			&layout,
			&layout.clone(),
		));

		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
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

		let chain = ForwardRecordNode::new(RecordSource::new(
			RecordSourceNode {
				frame_bytes: frame_bytes(&layout),
				element: 4.,
				fields: vec![],
				partial: true,
			},
			&layout,
			&layout.clone(),
		));

		let GPoll::Partial(value) = chain.eval(&ctx) else {
			panic!("expected a partial record");
		};
		assert_eq!(unsafe { value.rec().element::<f64>() }, 4.);
	}
}
