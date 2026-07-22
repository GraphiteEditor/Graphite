use std::f64::consts::{FRAC_PI_3, TAU};
use std::rc::Rc;

use glam::{DAffine2, DVec2};
use graphic_types::Vector;
use vector_types::subpath;

const HAT_VERTEX_COUNT: usize = 13;

fn hex_pt(x: f64, y: f64) -> DVec2 {
	let hr3 = 3.0_f64.sqrt() / 2.0;
	DVec2::new(x + 0.5 * y, hr3 * y)
}

fn hat_outline() -> [DVec2; HAT_VERTEX_COUNT] {
	[
		hex_pt(0., 0.),
		hex_pt(-1., -1.),
		hex_pt(0., -2.),
		hex_pt(2., -2.),
		hex_pt(2., -1.),
		hex_pt(4., -2.),
		hex_pt(5., -1.),
		hex_pt(4., 0.),
		hex_pt(3., 0.),
		hex_pt(2., 2.),
		hex_pt(0., 3.),
		hex_pt(0., 2.),
		hex_pt(-1., 2.),
	]
}

fn rot_about(p: DVec2, ang: f64) -> DAffine2 {
	DAffine2::from_translation(p) * DAffine2::from_angle(ang) * DAffine2::from_translation(-p)
}

fn match_seg(p: DVec2, q: DVec2) -> DAffine2 {
	DAffine2::from_cols_array(&[q.x - p.x, q.y - p.y, p.y - q.y, q.x - p.x, p.x, p.y])
}

fn match_two(p1: DVec2, q1: DVec2, p2: DVec2, q2: DVec2) -> DAffine2 {
	match_seg(p2, q2) * match_seg(p1, q1).inverse()
}

fn intersect(p1: DVec2, q1: DVec2, p2: DVec2, q2: DVec2) -> DVec2 {
	let d = (q2.y - p2.y) * (q1.x - p1.x) - (q2.x - p2.x) * (q1.y - p1.y);
	if d.abs() < 1e-12 {
		log::warn!("parallel lines in intersect");
		return p1;
	}
	let ua = ((q2.x - p2.x) * (p1.y - p2.y) - (q2.y - p2.y) * (p1.x - p2.x)) / d;
	p1 + ua * (q1 - p1)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TileLabel {
	H1,
	H,
	T,
	P,
	F,
}

enum TileType {
	Hat(TileLabel),
	Meta(MetaTile),
}

struct MetaTile {
	outline: Vec<DVec2>,
	width: f64,
	children: Vec<(DAffine2, Rc<TileType>)>,
	bound_radius: f64,
}

impl MetaTile {
	fn new(outline: Vec<DVec2>, width: f64) -> Self {
		let n = outline.len() as f64;
		let centroid = if n > 0. { outline.iter().copied().sum::<DVec2>() / n } else { DVec2::ZERO };
		let bound_radius = outline.iter().map(|p| p.distance(centroid)).reduce(f64::max).unwrap_or(0.);
		Self {
			outline,
			width,
			children: Vec::new(),
			bound_radius,
		}
	}

	fn push(&mut self, xform: DAffine2, tile: Rc<TileType>) {
		self.children.push((xform, tile));
	}

	fn eval_child(&self, n: usize, i: usize) -> DVec2 {
		let (xf, tile) = &self.children[n];
		let outline = match tile.as_ref() {
			TileType::Meta(m) => &m.outline,
			TileType::Hat(_) => {
				log::warn!("eval_child called on Hat leaf");
				return DVec2::ZERO;
			}
		};
		if outline.is_empty() {
			return DVec2::ZERO;
		}
		xf.transform_point2(outline[i % outline.len()])
	}

	fn child_outline(&self, n: usize) -> (&DAffine2, &[DVec2]) {
		let (xf, tile) = &self.children[n];
		match tile.as_ref() {
			TileType::Meta(m) => (xf, &m.outline),
			TileType::Hat(_) => {
				log::warn!("child_outline called on Hat leaf");
				(xf, &[])
			}
		}
	}

	fn recentre(&mut self) {
		let n = self.outline.len() as f64;
		if n == 0. {
			return;
		}
		let centroid = self.outline.iter().copied().sum::<DVec2>() / n;
		for p in &mut self.outline {
			*p -= centroid;
		}
		let m = DAffine2::from_translation(-centroid);
		for (xf, _) in &mut self.children {
			*xf = m * *xf;
		}
	}
}

fn build_init_metatiles() -> (Rc<TileType>, Rc<TileType>, Rc<TileType>, Rc<TileType>) {
	let hr3 = 3.0_f64.sqrt() / 2.0;
	let hat = hat_outline();

	let h1_rc = Rc::new(TileType::Hat(TileLabel::H1));
	let h_rc = Rc::new(TileType::Hat(TileLabel::H));
	let t_rc = Rc::new(TileType::Hat(TileLabel::T));
	let p_rc = Rc::new(TileType::Hat(TileLabel::P));
	let f_rc = Rc::new(TileType::Hat(TileLabel::F));

	let h_outline = vec![
		DVec2::new(0., 0.),
		DVec2::new(4., 0.),
		DVec2::new(4.5, hr3),
		DVec2::new(2.5, 5. * hr3),
		DVec2::new(1.5, 5. * hr3),
		DVec2::new(-0.5, hr3),
	];
	let mut h = MetaTile::new(h_outline.clone(), 2.);
	h.push(match_two(hat[5], hat[7], h_outline[5], h_outline[0]), Rc::clone(&h_rc));
	h.push(match_two(hat[9], hat[11], h_outline[1], h_outline[2]), Rc::clone(&h_rc));
	h.push(match_two(hat[5], hat[7], h_outline[3], h_outline[4]), Rc::clone(&h_rc));
	h.push(
		DAffine2::from_translation(DVec2::new(2.5, hr3)) * DAffine2::from_cols_array(&[-0.5, hr3, -hr3, -0.5, 0., 0.]) * DAffine2::from_scale(DVec2::new(0.5, -0.5)),
		Rc::clone(&h1_rc),
	);

	let mut t = MetaTile::new(vec![DVec2::new(0., 0.), DVec2::new(3., 0.), DVec2::new(1.5, 3. * hr3)], 2.);
	t.push(DAffine2::from_cols_array(&[0.5, 0., 0., 0.5, 0.5, hr3]), Rc::clone(&t_rc));

	let mut p = MetaTile::new(vec![DVec2::new(0., 0.), DVec2::new(4., 0.), DVec2::new(3., 2. * hr3), DVec2::new(-1., 2. * hr3)], 2.);
	p.push(DAffine2::from_cols_array(&[0.5, 0., 0., 0.5, 1.5, hr3]), Rc::clone(&p_rc));
	p.push(
		DAffine2::from_translation(DVec2::new(0., 2. * hr3)) * DAffine2::from_cols_array(&[0.5, -hr3, hr3, 0.5, 0., 0.]) * DAffine2::from_scale(DVec2::splat(0.5)),
		Rc::clone(&p_rc),
	);

	let mut f = MetaTile::new(
		vec![DVec2::new(0., 0.), DVec2::new(3., 0.), DVec2::new(3.5, hr3), DVec2::new(3., 2. * hr3), DVec2::new(-1., 2. * hr3)],
		2.,
	);
	f.push(DAffine2::from_cols_array(&[0.5, 0., 0., 0.5, 1.5, hr3]), Rc::clone(&f_rc));
	f.push(
		DAffine2::from_translation(DVec2::new(0., 2. * hr3)) * DAffine2::from_cols_array(&[0.5, -hr3, hr3, 0.5, 0., 0.]) * DAffine2::from_scale(DVec2::splat(0.5)),
		Rc::clone(&f_rc),
	);

	h.recentre();
	t.recentre();
	p.recentre();
	f.recentre();

	(Rc::new(TileType::Meta(h)), Rc::new(TileType::Meta(t)), Rc::new(TileType::Meta(p)), Rc::new(TileType::Meta(f)))
}

// Substitution rules
const RULES: &[&[i32]] = &[
	&[-1],
	&[0, 0, -2, 2],
	&[1, 0, -3, 2],
	&[2, 0, -2, 2],
	&[3, 0, -3, 2],
	&[4, 4, -2, 2],
	&[0, 4, -4, 3],
	&[2, 4, -4, 3],
	&[4, 1, 3, 2, -4, 0],
	&[8, 3, -3, 0],
	&[9, 2, -2, 0],
	&[10, 2, -3, 0],
	&[11, 4, -2, 2],
	&[12, 0, -3, 2],
	&[13, 0, -4, 3],
	&[14, 2, -4, 1],
	&[15, 3, -3, 4],
	&[8, 2, -4, 1],
	&[17, 3, -3, 0],
	&[18, 2, -2, 0],
	&[19, 2, -3, 2],
	&[20, 4, -4, 3],
	&[20, 0, -2, 2],
	&[22, 0, -3, 2],
	&[23, 4, -4, 3],
	&[23, 0, -4, 3],
	&[16, 0, -2, 2],
	&[9, 4, 0, 2, -5, 2],
	&[4, 0, -4, 3],
];

fn shape_for_label<'a>(h: &'a Rc<TileType>, t: &'a Rc<TileType>, p: &'a Rc<TileType>, f: &'a Rc<TileType>, label: i32) -> &'a Rc<TileType> {
	match label.unsigned_abs() as usize {
		2 => p,
		3 => h,
		4 => f,
		5 => t,
		_ => {
			log::warn!("Unknown label {}", label);
			h // Default fallback
		}
	}
}

fn construct_patch(h: &Rc<TileType>, t: &Rc<TileType>, p: &Rc<TileType>, f: &Rc<TileType>) -> MetaTile {
	let h_meta = match h.as_ref() {
		TileType::Meta(m) => m,
		_ => {
			log::warn!("Expected MetaTile for 'h' in construct_patch");
			return MetaTile::new(Vec::new(), 0.0); // Return empty patch
		}
	};
	let mut ret = MetaTile::new(Vec::new(), h_meta.width);

	for r in RULES {
		match r.len() {
			1 => ret.push(DAffine2::IDENTITY, Rc::clone(h)),
			4 => {
				let (xf, poly) = ret.child_outline(r[0] as usize);
				let (p_pt, q_pt) = (xf.transform_point2(poly[(r[1] as usize + 1) % poly.len()]), xf.transform_point2(poly[r[1] as usize]));
				let nshp = shape_for_label(h, t, p, f, r[2]);
				let nshp_meta = match nshp.as_ref() {
					TileType::Meta(m) => m,
					_ => {
						log::warn!("Expected MetaTile in construct_patch rule");
						continue;
					}
				};
				let idx = r[3] as usize;
				if nshp_meta.outline.is_empty() {
					continue;
				}
				ret.push(match_two(nshp_meta.outline[idx], nshp_meta.outline[(idx + 1) % nshp_meta.outline.len()], p_pt, q_pt), Rc::clone(nshp));
			}
			_ => {
				let (xf_p, poly_p) = ret.child_outline(r[0] as usize);
				let q_pt = xf_p.transform_point2(poly_p[r[1] as usize]);
				let (xf_q, poly_q) = ret.child_outline(r[2] as usize);
				let p_pt = xf_q.transform_point2(poly_q[r[3] as usize]);
				let nshp = shape_for_label(h, t, p, f, r[4]);
				let nshp_meta = match nshp.as_ref() {
					TileType::Meta(m) => m,
					_ => {
						log::warn!("Expected MetaTile in construct_patch rule");
						continue;
					}
				};
				let idx = r[5] as usize;
				if nshp_meta.outline.is_empty() {
					continue;
				}
				ret.push(match_two(nshp_meta.outline[idx], nshp_meta.outline[(idx + 1) % nshp_meta.outline.len()], p_pt, q_pt), Rc::clone(nshp));
			}
		}
	}

	ret
}

// Metatile extraction from a substitution patch
fn construct_metatiles(patch: &MetaTile) -> (Rc<TileType>, Rc<TileType>, Rc<TileType>, Rc<TileType>) {
	let bps1 = patch.eval_child(8, 2);
	let bps2 = patch.eval_child(21, 2);
	let rbps = rot_about(bps1, -2.0 * TAU / 3.0).transform_point2(bps2);
	let (p72, p252) = (patch.eval_child(7, 2), patch.eval_child(25, 2));

	let llc = intersect(bps1, rbps, patch.eval_child(6, 2), p72);
	let mut w = patch.eval_child(6, 2) - llc;

	// Build new H outline
	let mut h_out = vec![llc, bps1];
	w = DAffine2::from_angle(-FRAC_PI_3).transform_vector2(w);
	h_out.push(h_out[1] + w);
	h_out.push(patch.eval_child(14, 2));
	w = DAffine2::from_angle(-FRAC_PI_3).transform_vector2(w);
	h_out.push(h_out[3] - w);
	h_out.push(patch.eval_child(6, 2));

	let copy_children = |meta: &mut MetaTile, indices: &[usize]| {
		for &i in indices {
			let (xf, tile) = &patch.children[i];
			meta.push(*xf, Rc::clone(tile));
		}
	};

	let mut new_h = MetaTile::new(h_out.clone(), patch.width * 2.);
	copy_children(&mut new_h, &[0, 9, 16, 27, 26, 6, 1, 8, 10, 15]);

	let mut new_p = MetaTile::new(vec![p72, p72 + (bps1 - llc), bps1, llc], patch.width * 2.);
	copy_children(&mut new_p, &[7, 2, 3, 4, 28]);

	let mut new_f = MetaTile::new(vec![bps2, patch.eval_child(24, 2), patch.eval_child(25, 0), p252, p252 + (llc - bps1)], patch.width * 2.);
	copy_children(&mut new_f, &[21, 20, 22, 23, 24, 25]);

	let (aaa, bbb) = (h_out[2], h_out[1] + (h_out[4] - h_out[5]));
	let ccc = rot_about(bbb, -FRAC_PI_3).transform_point2(aaa);
	let mut new_t = MetaTile::new(vec![bbb, ccc, aaa], patch.width * 2.);
	copy_children(&mut new_t, &[11]);

	for meta in [&mut new_h, &mut new_p, &mut new_f, &mut new_t] {
		meta.recentre();
	}

	(
		Rc::new(TileType::Meta(new_h)),
		Rc::new(TileType::Meta(new_t)),
		Rc::new(TileType::Meta(new_p)),
		Rc::new(TileType::Meta(new_f)),
	)
}

// Recursive hat collection
fn collect_hat_transforms(tile: &TileType, parent_xform: &DAffine2, out: &mut Vec<(DAffine2, TileLabel)>, viewport_bounds: &Option<[DVec2; 2]>) {
	match tile {
		TileType::Hat(label) => out.push((*parent_xform, *label)),
		TileType::Meta(meta) => {
			if let Some([vp_min, vp_max]) = viewport_bounds {
				let global_centroid = parent_xform.transform_point2(DVec2::ZERO);
				let scale = parent_xform.transform_vector2(DVec2::new(1., 0.)).length();
				let global_radius = meta.bound_radius * scale;

				if global_centroid.x + global_radius < vp_min.x
					|| global_centroid.x - global_radius > vp_max.x
					|| global_centroid.y + global_radius < vp_min.y
					|| global_centroid.y - global_radius > vp_max.y
				{
					return;
				}
			}

			for (xf, child_tile) in &meta.children {
				collect_hat_transforms(child_tile.as_ref(), &(*parent_xform * *xf), out, viewport_bounds);
			}
		}
	}
}

pub fn generate_hat_tiling(levels: u32, scale: f64, viewport_bounds: Option<[DVec2; 2]>) -> Vector {
	let (mut h, mut t, mut p, mut f) = build_init_metatiles();

	for _ in 1..levels {
		let patch = construct_patch(&h, &t, &p, &f);
		let next = construct_metatiles(&patch);
		h = next.0;
		t = next.1;
		p = next.2;
		f = next.3;
	}

	let mut transforms = Vec::new();
	let model_viewport = viewport_bounds.map(|[min, max]| [min / scale, max / scale]);
	collect_hat_transforms(h.as_ref(), &DAffine2::IDENTITY, &mut transforms, &model_viewport);

	let hat = hat_outline();
	let mut vector = Vector::default();

	for (xf, _) in &transforms {
		let mut vertices = [DVec2::ZERO; HAT_VERTEX_COUNT];
		for i in 0..HAT_VERTEX_COUNT {
			vertices[i] = xf.transform_point2(hat[i]) * scale;
		}

		if let Some([vp_min, vp_max]) = viewport_bounds {
			let tile_min = vertices.iter().copied().reduce(|a, b| a.min(b)).unwrap_or_default();
			let tile_max = vertices.iter().copied().reduce(|a, b| a.max(b)).unwrap_or_default();
			if tile_max.x < vp_min.x || tile_min.x > vp_max.x || tile_max.y < vp_min.y || tile_min.y > vp_max.y {
				continue;
			}
		}

		vector.append_subpath(subpath::Subpath::from_anchors(vertices, true), false);
	}

	vector
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn generates_tiles_at_level_1() {
		let vector = generate_hat_tiling(1, 10., None);
		assert_eq!(vector.region_domain.ids().len(), 4);
	}
}
