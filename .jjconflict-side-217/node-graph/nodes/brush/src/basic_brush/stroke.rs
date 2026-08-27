use super::consts::{RIDGE_GAIN, SIGMA_CUTOFF, SIGMA_PER_DIAMETER};
use brush_types::{Sample, Stroke};
use bytemuck::{Pod, Zeroable};
use core_types::Color;
use core_types::math::bbox::AxisAlignedBbox;
use glam::DVec2;

const MIN_SIGMA: f32 = f32::EPSILON;
const MAX_EDGE_SHIFT: f32 = 0.25;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Pod, Zeroable)]
pub(super) struct Edge {
	a: [f32; 2],
	b: [f32; 2],
	sigma: f32,
	weight: f32,
}

pub(super) struct StyledStroke {
	pub(super) color: Color,
	pub(super) diameter: f64,
	pub(super) hardness: f64,
	pub(super) flow: f64,
	pub(super) stroke: Stroke,
}

#[derive(Clone, Copy)]
struct Dab {
	position: DVec2,
	sigma: f32,
	weight: f32,
}

fn dab(sample: &Sample, stroke: &StyledStroke) -> Dab {
	let pressure = sample.pressure.clamp(0., 1.);
	let flow = stroke.flow.clamp(0., 1.) as f32;
	Dab {
		position: sample.position,
		sigma: (stroke.diameter.max(0.) * SIGMA_PER_DIAMETER) as f32 * pressure,
		weight: -(1. - flow * (1. - (-RIDGE_GAIN).exp())).ln() / RIDGE_GAIN,
	}
}

fn dab_pad(dab: Dab, scale: f64) -> AxisAlignedBbox {
	let sigma = (dab.sigma as f64).max(MIN_SIGMA as f64 / scale);
	let pad = DVec2::splat(SIGMA_CUTOFF as f64 * sigma + 1f64.max(1. / scale));
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

pub(super) fn bounds(stroke: &StyledStroke, scale: f64) -> AxisAlignedBbox {
	let mut bounds = None;
	for sample in stroke.stroke.samples() {
		union(&mut bounds, dab_pad(dab(&sample, stroke), scale));
	}
	bounds.unwrap_or(AxisAlignedBbox::ZERO)
}

pub(super) struct Update {
	pub(super) committed: Vec<Edge>,
	pub(super) tail: Vec<Edge>,
}

#[derive(Clone)]
pub(super) struct Walk {
	sigma_min: f32,
	kept_last: Option<Dab>,
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
	fn advance(&mut self, stroke: &StyledStroke, scale: f64) -> Vec<Dab> {
		let mut kept = Vec::new();
		for index in self.consumed..stroke.stroke.len() {
			let sample = stroke.stroke.sample(index);
			let dab = dab(&sample, stroke);
			self.sigma_min = self.sigma_min.min(dab.sigma);
			let min_step = (self.sigma_min as f64 * 0.5).max(0.5 / scale);
			if self.kept_last.is_none_or(|last| last.position.distance(dab.position) >= min_step) {
				kept.push(dab);
				self.kept_last = Some(dab);
				self.kept += 1;
			}
		}
		self.consumed = stroke.stroke.len();
		kept
	}

	fn tail(&self, stroke: &StyledStroke) -> Option<(Dab, Dab)> {
		let kept_last = self.kept_last?;
		let dab = dab(&stroke.stroke.sample(stroke.stroke.len() - 1), stroke);
		if dab.position == kept_last.position {
			return (self.kept == 1).then_some((kept_last, kept_last));
		}
		Some((kept_last, dab))
	}

	pub(super) fn update(&mut self, stroke: &StyledStroke, region: &super::region::Region) -> Update {
		let previous = self.kept_last;
		let kept = self.advance(stroke, region.scale);
		let tail = self.tail(stroke);
		Update {
			committed: edges(region, previous, &kept, None),
			tail: edges(region, None, &[], tail),
		}
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
	let shift = (b.sigma - a.sigma).abs() * scale * SIGMA_CUTOFF;
	let pieces = (shift / (MAX_EDGE_SHIFT * gradient)).ceil().clamp(1., 64.) as usize;
	let mut previous = a;
	for piece in 1..=pieces {
		let next = if piece == pieces { b } else { mix(a, b, piece as f32 / pieces as f32) };
		edges.push(edge(region, previous, next));
		previous = next;
	}
}

fn edges(region: &super::region::Region, prev: Option<Dab>, kept: &[Dab], tail: Option<(Dab, Dab)>) -> Vec<Edge> {
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

#[cfg(test)]
mod tests {
	use super::*;
	use glam::{DVec2, UVec2};

	fn stroke(points: &[[f64; 2]]) -> StyledStroke {
		StyledStroke {
			color: Color::BLACK,
			diameter: 20.,
			hardness: 0.8,
			flow: 1.,
			stroke: Stroke {
				position: points.iter().copied().map(DVec2::from).collect(),
				..Default::default()
			},
		}
	}

	fn region() -> super::super::region::Region {
		super::super::region::Region {
			min: DVec2::ZERO,
			scale: 2.,
			size: UVec2::splat(512),
		}
	}

	#[test]
	fn chunked_walk_matches_whole_stroke() {
		let partial = stroke(&[[10., 10.], [15., 12.], [20., 15.]]);
		let complete = stroke(&[[10., 10.], [15., 12.], [20., 15.], [31., 19.], [45., 24.]]);
		let region = region();

		let mut chunked = Walk::default();
		let first = chunked.update(&partial, &region);
		let second = chunked.update(&complete, &region);
		let mut committed = first.committed;
		committed.extend(second.committed);

		let whole = Walk::default().update(&complete, &region);
		assert_eq!(committed, whole.committed);
		assert_eq!(second.tail, whole.tail);
	}
}
