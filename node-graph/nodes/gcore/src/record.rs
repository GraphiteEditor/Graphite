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
		stack::reserve(layouts.iter().map(|layout| layout.frame_bytes()).sum());
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

		let chain = MultiplyOpacityNode::new(
			MultiplyOpacityNode::new(bare_source(&source_layout, 2.), ValueNode(0.5), &source_layout),
			ValueNode(0.5),
			&modified,
		);
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
		let rec = scaled.rec(&value);
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

		let monitor = core_types::record::RecordMonitor::new(f64_record_source(&layout, 4., vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)]), &layout);
		{
			let scope = scope_fixture(&generations, &arena);
			let ctx = ContextImpl::root(&scope);
			let GPoll::Final(value) = monitor.eval(&ctx) else {
				panic!("expected a final record");
			};
			assert_eq!(unsafe { layout.rec(&value).element::<f64>() }, 4.);
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
				&union,
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

		let chain = HoldFirstNode::new(
			ValueNode(false),
			RecordSource::new(f64_record_source(&layout_a, 1., vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)]), &layout_a, &union),
			RecordSource::new(f64_record_source(&layout_b, 3., vec![(layout_b.offset_of("length", 0).unwrap(), 3.)]), &layout_b, &union),
			&union,
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
			assert!(self.layout.is_inline());
			let mut value = RecordValue::zeroed();
			let element: f64 = match core_types::context::ExtractRealTime::try_real_time(input) {
				Some(_) => 1.,
				None => 0.,
			};
			unsafe { value.as_mut_ptr().cast::<f64>().write(element) };
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
			let node = crate::context_modification::ContextModificationNode::new(
				RealTimeProbe { layout: layout.clone() },
				ValueNode(ContextModification::from_sources(features, &[])),
				&layout,
			);
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

		let node = crate::context_modification::ContextModificationNode::new(
			RecordSourceNode {
				layout: layout.clone(),
				element: 4.,
				fields: vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)],
				partial: true,
			},
			ValueNode(ContextModification::from_sources(ContextFeatures::all(), &[])),
			&layout,
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
	fn inline_records_survive_sibling_evaluations_by_value() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout_a = f64_layout(&["opacity"]);
		let layout_b = f64_layout(&[]);
		let union = Layout::union(&[&layout_a, &layout_b]);
		assert!(union.is_inline());
		reserve_for(&[&layout_a, &layout_b, &union, &union]);

		let chain = HoldFirstNode::new(
			ValueNode(false),
			RecordSource::new(f64_record_source(&layout_a, 1., vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)]), &layout_a, &union),
			RecordSource::new(f64_record_source(&layout_b, 3., vec![]), &layout_b, &union),
			&union,
		);

		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = union.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 0.5);
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
