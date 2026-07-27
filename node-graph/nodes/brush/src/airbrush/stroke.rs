use brush_types::{BrushStyle, Sample, Stroke};
use bytemuck::{Pod, Zeroable};
use core_types::math::bbox::AxisAlignedBbox;
use glam::{DVec2, UVec2};

const MIN_SIGMA: f32 = f32::EPSILON;
pub(super) const CUTOFF_SIGMA: f32 = 5.;
const MAX_EDGE_SHIFT: f32 = 0.25;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(super) struct Edge {
	a: [f32; 2],
	b: [f32; 2],
	sigma: f32,
	weight: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Dab {
	position: DVec2,
	sigma: f32,
	weight: f32,
}

fn dab(sample: &Sample, style: &BrushStyle) -> Dab {
	let pressure = sample.pressure.clamp(0., 1.);
	let hardness = style.hardness.clamp(0., 1.) as f32;
	Dab {
		position: sample.position,
		sigma: style.diameter.max(0.) as f32 * pressure / 4.,
		// ~17x buildup at the default hardness saturates the core to near-solid.
		weight: style.flow.clamp(0., 1.) as f32 * (1. + hardness * hardness * 24.) * pressure,
	}
}

fn dab_pad(dab: Dab, scale: f64) -> AxisAlignedBbox {
	let sigma = (dab.sigma as f64).max(MIN_SIGMA as f64 / scale);
	let pad = DVec2::splat(CUTOFF_SIGMA as f64 * sigma + 1f64.max(1. / scale));
	AxisAlignedBbox {
		start: dab.position - pad,
		end: dab.position + pad,
	}
}

pub(super) fn union(bounds: &mut Option<AxisAlignedBbox>, other: AxisAlignedBbox) {
	*bounds = Some(match bounds.take() {
		Some(existing) => existing.union(&other),
		None => other,
	});
}

pub(super) fn bounds(stroke: &Stroke, style: &BrushStyle, scale: f64) -> AxisAlignedBbox {
	let mut bounds = None;
	for sample in stroke.samples() {
		union(&mut bounds, dab_pad(dab(&sample, style), scale));
	}
	bounds.unwrap_or(AxisAlignedBbox::ZERO)
}

#[derive(Clone)]
pub(super) struct Walk {
	sigma_min: f32,
	pub(super) kept_last: Option<Dab>,
	kept: usize,
	pub(super) consumed: usize,
}

impl Default for Walk {
	fn default() -> Self {
		Self {
			sigma_min: f32::MAX,
			kept_last: None,
			kept: 0,
			consumed: 0,
		}
	}
}

impl Walk {
	pub(super) fn advance(&mut self, stroke: &Stroke, style: &BrushStyle, scale: f64) -> Vec<Dab> {
		let mut kept = Vec::new();
		for index in self.consumed..stroke.len() {
			let sample = stroke.sample(index);
			let dab = dab(&sample, style);
			self.sigma_min = self.sigma_min.min(dab.sigma);
			let min_step = (self.sigma_min as f64 * 0.5).max(0.5 / scale);
			if self.kept_last.is_none_or(|last| last.position.distance(dab.position) >= min_step) {
				kept.push(dab);
				self.kept_last = Some(dab);
				self.kept += 1;
			}
		}
		self.consumed = stroke.len();
		kept
	}

	pub(super) fn tail(&self, stroke: &Stroke, style: &BrushStyle) -> Option<(Dab, Dab)> {
		let kept_last = self.kept_last?;
		let dab = dab(&stroke.sample(stroke.len() - 1), style);
		if dab.position == kept_last.position {
			return (self.kept == 1).then_some((kept_last, kept_last));
		}
		Some((kept_last, dab))
	}
}

fn texel(region: &super::region::Region, p: DVec2) -> [f32; 2] {
	[((p.x - region.min.x) * region.scale) as f32, ((p.y - region.min.y) * region.scale) as f32]
}

fn edge(region: &super::region::Region, a: Dab, b: Dab) -> Edge {
	Edge {
		a: texel(region, a.position),
		b: texel(region, b.position),
		sigma: ((a.sigma + b.sigma) / 2. * region.scale as f32).max(MIN_SIGMA),
		weight: (a.weight + b.weight) / 2.,
	}
}

fn mix(a: Dab, b: Dab, t: f32) -> Dab {
	Dab {
		position: a.position.lerp(b.position, t as f64),
		sigma: a.sigma + (b.sigma - a.sigma) * t,
		weight: a.weight + (b.weight - a.weight) * t,
	}
}

fn segment_edges(edges: &mut Vec<Edge>, region: &super::region::Region, a: Dab, b: Dab) {
	let scale = region.scale as f32;
	let gradient = (a.sigma.min(b.sigma) * scale).max(1.);
	let shift = (b.sigma - a.sigma).abs() * scale * CUTOFF_SIGMA;
	let pieces = (shift / (MAX_EDGE_SHIFT * gradient)).ceil().clamp(1., 64.) as usize;
	let mut previous = a;
	for piece in 1..=pieces {
		let next = if piece == pieces { b } else { mix(a, b, piece as f32 / pieces as f32) };
		edges.push(edge(region, previous, next));
		previous = next;
	}
}

pub(super) fn edges(region: &super::region::Region, prev: Option<Dab>, kept: &[Dab], tail: Option<(Dab, Dab)>) -> Vec<Edge> {
	let mut edges = Vec::with_capacity(kept.len() + 1);
	let mut last = prev;
	for &dab in kept {
		if let Some(previous) = last {
			segment_edges(&mut edges, region, previous, dab);
		}
		last = Some(dab);
	}
	if let Some((a, b)) = tail {
		segment_edges(&mut edges, region, a, b);
	}
	edges
}

pub(super) fn walk(stroke: &Stroke, style: &BrushStyle, scale: f64) -> (Vec<Dab>, Option<(Dab, Dab)>) {
	let mut walk = Walk::default();
	let kept = walk.advance(stroke, style, scale);
	let tail = walk.tail(stroke, style);
	(kept, tail)
}

pub(super) fn scissor(region: &super::region::Region, crop: &super::region::Crop, bounds: AxisAlignedBbox) -> (UVec2, UVec2) {
	let clamp = |texels: UVec2| texels.max(crop.origin).min(crop.origin + crop.size) - crop.origin;
	let min = ((bounds.start - region.min) * region.scale).floor().max(DVec2::ZERO).as_uvec2().min(region.size);
	let max = ((bounds.end - region.min) * region.scale).ceil().max(DVec2::ZERO).as_uvec2().min(region.size);
	(clamp(min), clamp(max) - clamp(min))
}
