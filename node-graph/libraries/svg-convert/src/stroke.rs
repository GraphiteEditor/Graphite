use graphic_types::vector_types::vector::style::{Stroke, StrokeCap, StrokeJoin};

pub fn style(stroke: &usvg::Stroke) -> Stroke {
	let (dash_lengths, dash_offset) = match stroke.dasharray() {
		Some(lengths) => (lengths.iter().copied().map(f64::from).collect(), f64::from(stroke.dashoffset())),
		None => (Vec::new(), 0.),
	};

	Stroke {
		weight: stroke.width().get().into(),
		dash_lengths,
		dash_offset,
		cap: match stroke.linecap() {
			usvg::LineCap::Butt => StrokeCap::Butt,
			usvg::LineCap::Round => StrokeCap::Round,
			usvg::LineCap::Square => StrokeCap::Square,
		},
		join: match stroke.linejoin() {
			usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => StrokeJoin::Miter,
			usvg::LineJoin::Round => StrokeJoin::Round,
			usvg::LineJoin::Bevel => StrokeJoin::Bevel,
		},
		join_miter_limit: stroke.miterlimit().get().into(),
		..Stroke::default()
	}
}
