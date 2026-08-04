//! Pilot record nodes exercising the macro's attribute io over materialized
//! lists: `Attr<A>` reads, tuple writes, column carry, and the census
//! defaults. These are the flat-wave law tests; the node forms are the
//! production authoring surface, the list driver behind them is interim.

use core_types::attribute::{Attr, Opacity};
use core_types::gpoll::{GraphError, Interrupt};
use core_types::{Context, Ctx};

core_types::attribute! {
	/// Test-only measured length of an element.
	pub Length("length"): f64;
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
	use core_types::list::List;
	use core_types::node::Node;

	struct ValueNode<T>(T);

	impl<T: Clone, Input> Node<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	struct PartialListNode(List<f64>);

	impl<Input> Node<Input> for PartialListNode {
		type Output = List<f64>;

		fn eval(&self, _input: &Input) -> GPoll<List<f64>> {
			GPoll::Partial(self.0.clone())
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn elements(list: &List<f64>) -> Vec<f64> {
		list.iter_element_values().copied().collect()
	}

	fn column(list: &List<f64>, key: &str) -> Vec<f64> {
		list.iter_attribute_values::<f64>(key).unwrap().copied().collect()
	}

	#[test]
	fn defaults_then_modify_then_stack() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let chain = MultiplyOpacityNode::new(
			MultiplyOpacityNode::new(ValueNode(List::from_element_values(vec![1., 2.])), ValueNode(0.5)),
			ValueNode(0.5),
		);
		let GPoll::Final(list) = chain.eval(&ctx) else {
			panic!("expected a final list");
		};
		assert_eq!(elements(&list), vec![1., 2.]);
		assert_eq!(column(&list, Opacity::NAME), vec![0.25, 0.25]);
	}

	#[test]
	fn tuple_write_element_and_attribute() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let chain = MeasureNode::new(ValueNode(List::from_element_values(vec![-2., 3.])));
		let GPoll::Final(list) = chain.eval(&ctx) else {
			panic!("expected a final list");
		};
		assert_eq!(elements(&list), vec![-2., 3.]);
		assert_eq!(column(&list, Length::NAME), vec![2., 3.]);
	}

	#[test]
	fn elementwise_write_carries_unrelated_columns() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let chain = MeasureNode::new(MultiplyOpacityNode::new(ValueNode(List::from_element_values(vec![-2., 3.])), ValueNode(0.5)));
		let GPoll::Final(list) = chain.eval(&ctx) else {
			panic!("expected a final list");
		};
		assert_eq!(column(&list, Opacity::NAME), vec![0.5, 0.5]);
		assert_eq!(column(&list, Length::NAME), vec![2., 3.]);
	}

	#[test]
	fn reads_only_kernel_reads_the_declared_default_and_carries() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let bare = ShadeNode::new(ValueNode(List::from_element_values(vec![4., 6.])));
		let GPoll::Final(list) = bare.eval(&ctx) else {
			panic!("expected a final list");
		};
		assert_eq!(elements(&list), vec![4., 6.]);

		let shaded = ShadeNode::new(MultiplyOpacityNode::new(ValueNode(List::from_element_values(vec![4., 6.])), ValueNode(0.5)));
		let GPoll::Final(list) = shaded.eval(&ctx) else {
			panic!("expected a final list");
		};
		assert_eq!(elements(&list), vec![2., 3.]);
		assert_eq!(column(&list, Opacity::NAME), vec![0.5, 0.5]);
	}

	#[test]
	fn partial_carrier_downgrades_the_output() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let chain = MultiplyOpacityNode::new(PartialListNode(List::from_element_values(vec![1.])), ValueNode(0.5));
		let GPoll::Partial(list) = chain.eval(&ctx) else {
			panic!("expected a partial list");
		};
		assert_eq!(column(&list, Opacity::NAME), vec![0.5]);
	}

	use core_types::context::ExtractFrame;
	use core_types::record::{Frame, FrameLayout, Layout, Rec, RecordSource, RecordValue};

	struct RecordSourceNode {
		slot: usize,
		element: f64,
		fields: Vec<(usize, f64)>,
		partial: bool,
	}

	impl<'e> Node<ContextImpl<'e>> for RecordSourceNode {
		type Output = RecordValue<'e>;

		fn eval(&self, input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			let frame = ExtractFrame::frame(input).unwrap();
			let value = unsafe {
				let dst = frame.slot(self.slot);
				dst.cast::<f64>().write(self.element);
				for (offset, value) in &self.fields {
					dst.add(*offset).cast::<f64>().write(*value);
				}
				RecordValue::from_rec(Rec::new(dst))
			};
			match self.partial {
				true => GPoll::Partial(value),
				false => GPoll::Final(value),
			}
		}
	}

	fn f64_layout(names: &[&'static str]) -> Layout {
		let writes: Vec<(&'static str, u8, usize, usize)> = names.iter().map(|name| (*name, 0, 8, 8)).collect();
		Layout::default().with_writes(0, (8, 8), &writes)
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

		let chain = ScaleNode::new(
			MultiplyOpacityNode::new(ValueNode(List::from_element_values(vec![1., 2.])), ValueNode(0.5)),
			StaticLendNode(&FACTOR),
		);
		let GPoll::Final(list) = chain.eval(&ctx) else {
			panic!("expected a final list");
		};
		assert_eq!(elements(&list), vec![3., 6.]);
		assert_eq!(column(&list, Opacity::NAME), vec![0.5, 0.5]);
	}

	#[test]
	fn routing_unions_branch_layouts_and_fills_census_defaults() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];

		let layout_a = f64_layout(&["opacity"]);
		let layout_b = f64_layout(&["length"]);
		let union = Layout::union(&[&layout_a, &layout_b]);

		let mut frame_layout = FrameLayout::default();
		let slot_a = frame_layout.slot(&layout_a);
		let slot_b = frame_layout.slot(&layout_b);
		let translate_a = frame_layout.slot(&union);
		let translate_b = frame_layout.slot(&union);
		let frame = Frame::new(frame_layout.size());

		let scope = scope_fixture(&generations, &arena).with_frame(&frame);
		let ctx = ContextImpl::root(&scope);

		let taken = |second: bool| {
			PickNode::new(
				ValueNode(second),
				RecordSource::wire(
					RecordSourceNode {
						slot: slot_a,
						element: 1.,
						fields: vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)],
						partial: false,
					},
					&layout_a,
					&union,
					translate_a,
				),
				RecordSource::wire(
					RecordSourceNode {
						slot: slot_b,
						element: 3.,
						fields: vec![(layout_b.offset_of("length", 0).unwrap(), 3.)],
						partial: false,
					},
					&layout_b,
					&union,
					translate_b,
				),
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

		let layout_a = f64_layout(&["opacity"]);
		let layout_b = f64_layout(&["length"]);
		let union = Layout::union(&[&layout_a, &layout_b]);

		let mut frame_layout = FrameLayout::default();
		let slot_a = frame_layout.slot(&layout_a);
		let slot_b = frame_layout.slot(&layout_b);
		let translate_a = frame_layout.slot(&union);
		let translate_b = frame_layout.slot(&union);
		let frame = Frame::new(frame_layout.size());

		let scope = scope_fixture(&generations, &arena).with_frame(&frame);
		let ctx = ContextImpl::root(&scope);

		let chain = HoldFirstNode::new(
			ValueNode(false),
			RecordSource::wire(
				RecordSourceNode {
					slot: slot_a,
					element: 1.,
					fields: vec![(layout_a.offset_of("opacity", 0).unwrap(), 0.5)],
					partial: false,
				},
				&layout_a,
				&union,
				translate_a,
			),
			RecordSource::wire(
				RecordSourceNode {
					slot: slot_b,
					element: 3.,
					fields: vec![(layout_b.offset_of("length", 0).unwrap(), 3.)],
					partial: false,
				},
				&layout_b,
				&union,
				translate_b,
			),
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

		let layout = f64_layout(&["opacity"]);
		let mut frame_layout = FrameLayout::default();
		let slot = frame_layout.slot(&layout);
		let frame = Frame::new(frame_layout.size());

		let scope = scope_fixture(&generations, &arena).with_frame(&frame);
		let ctx = ContextImpl::root(&scope);

		let chain = ForwardRecordNode::new(RecordSource::wire(
			RecordSourceNode {
				slot,
				element: 4.,
				fields: vec![(layout.offset_of("opacity", 0).unwrap(), 0.25)],
				partial: false,
			},
			&layout,
			&layout.clone(),
			0,
		));

		let GPoll::Final(value) = chain.eval(&ctx) else {
			panic!("expected a final record");
		};
		let rec = value.rec();
		assert_eq!(rec.ptr(), unsafe { frame.slot(slot) }.cast_const());
		assert_eq!(unsafe { rec.element::<f64>() }, 4.);
		assert_eq!(unsafe { rec.read::<f64>(layout.offset_of("opacity", 0).unwrap()) }, 0.25);
	}

	#[test]
	fn partial_routing_sources_downgrade_the_output() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];

		let layout = f64_layout(&["opacity"]);
		let mut frame_layout = FrameLayout::default();
		let slot = frame_layout.slot(&layout);
		let frame = Frame::new(frame_layout.size());

		let scope = scope_fixture(&generations, &arena).with_frame(&frame);
		let ctx = ContextImpl::root(&scope);

		let chain = ForwardRecordNode::new(RecordSource::wire(
			RecordSourceNode {
				slot,
				element: 4.,
				fields: vec![],
				partial: true,
			},
			&layout,
			&layout.clone(),
			0,
		));

		let GPoll::Partial(value) = chain.eval(&ctx) else {
			panic!("expected a partial record");
		};
		assert_eq!(unsafe { value.rec().element::<f64>() }, 4.);
	}

	#[test]
	fn interrupt_kernel_errors_stop_the_eval() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let ok = CheckedMultiplyOpacityNode::new(ValueNode(List::from_element_values(vec![1.])), ValueNode(0.5));
		let GPoll::Final(list) = ok.eval(&ctx) else {
			panic!("expected a final list");
		};
		assert_eq!(column(&list, Opacity::NAME), vec![0.5]);

		let failing = CheckedMultiplyOpacityNode::new(ValueNode(List::from_element_values(vec![1.])), ValueNode(-1.));
		let GPoll::Error(error) = failing.eval(&ctx) else {
			panic!("expected an error");
		};
		assert!(error.kind == "negative factor");
	}
}
