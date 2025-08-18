use glam::DVec2;
use graphene_core::subpath::{ManipulatorGroup, Subpath};
use graphene_core::vector::PointId;

pub fn convert_usvg_path(path: &usvg::Path) -> Vec<Subpath<PointId>> {
	let mut subpaths = Vec::new();
	let mut manipulators_list = Vec::new();

	let mut points = path.data().points().iter();
	let to_vec = |p: &usvg::tiny_skia_path::Point| DVec2::new(p.x as f64, p.y as f64);

	for verb in path.data().verbs() {
		match verb {
			usvg::tiny_skia_path::PathVerb::Move => {
				subpaths.push(Subpath::new(std::mem::take(&mut manipulators_list), false));
				let Some(start) = points.next().map(to_vec) else { continue };
				manipulators_list.push(ManipulatorGroup::new(start, Some(start), Some(start)));
			}
			usvg::tiny_skia_path::PathVerb::Line => {
				let Some(end) = points.next().map(to_vec) else { continue };
				manipulators_list.push(ManipulatorGroup::new(end, Some(end), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Quad => {
				let Some(handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = manipulators_list.last_mut() {
					last.out_handle = Some(last.anchor + (2. / 3.) * (handle - last.anchor));
				}
				manipulators_list.push(ManipulatorGroup::new(end, Some(end + (2. / 3.) * (handle - end)), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Cubic => {
				let Some(first_handle) = points.next().map(to_vec) else { continue };
				let Some(second_handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = manipulators_list.last_mut() {
					last.out_handle = Some(first_handle);
				}
				manipulators_list.push(ManipulatorGroup::new(end, Some(second_handle), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Close => {
				subpaths.push(Subpath::new(std::mem::take(&mut manipulators_list), true));
			}
		}
	}
	subpaths.push(Subpath::new(manipulators_list, false));
	subpaths
}
