use crate::path_boolean::{self, FillRule, PathBooleanOperation};
use crate::path_data::{path_from_path_data, path_to_path_data};
use core::panic;
use glob::glob;
use image::{DynamicImage, GenericImageView, RgbaImage};
use resvg::render;
use resvg::tiny_skia::Transform;
use resvg::usvg::{Options, Tree};
use std::fs;
use std::path::PathBuf;
use svg::parser::Event;

const TOLERANCE: u8 = 84;

fn get_fill_rule(fill_rule: &str) -> FillRule {
	match fill_rule {
		"evenodd" => FillRule::EvenOdd,
		_ => FillRule::NonZero,
	}
}

#[test]
fn visual_tests() {
	let ops = [
		("union", PathBooleanOperation::Union),
		("difference", PathBooleanOperation::Difference),
		("intersection", PathBooleanOperation::Intersection),
		("exclusion", PathBooleanOperation::Exclusion),
		("division", PathBooleanOperation::Division),
		("fracture", PathBooleanOperation::Fracture),
	];

	let folders: Vec<(String, PathBuf, &str, PathBooleanOperation)> = glob("visual-tests/*/")
		.expect("Failed to read glob pattern")
		.flat_map(|entry| {
			let dir = entry.expect("Failed to get directory entry");
			ops.iter()
				.map(move |(op_name, op)| (dir.file_name().unwrap().to_string_lossy().into_owned(), dir.clone(), *op_name, *op))
		})
		.collect();

	let mut failure = false;

	for (name, dir, op_name, op) in folders {
		let test_name = format!("{} {}", name, op_name);
		println!("Running test: {}", test_name);

		fs::create_dir_all(dir.join("test-results")).expect("Failed to create test-results directory");

		let original_path = dir.join("original.svg");

		let mut content = String::new();
		let svg_tree = svg::open(&original_path, &mut content).expect("Failed to parse SVG");

		let mut paths = Vec::new();
		let mut first_path_attributes = String::new();
		let mut width = String::new();
		let mut height = String::new();
		let mut view_box = String::new();
		let mut transform = String::new();
		for event in svg_tree {
			match event {
				Event::Tag("svg", svg::node::element::tag::Type::Start, attributes) => {
					width = attributes.get("width").map(|s| s.to_string()).unwrap_or_default();
					height = attributes.get("height").map(|s| s.to_string()).unwrap_or_default();
					view_box = attributes.get("viewBox").map(|s| s.to_string()).unwrap_or_default();
				}
				Event::Tag("g", svg::node::element::tag::Type::Start, attributes) => {
					if let Some(transform_attr) = attributes.get("transform") {
						transform = transform_attr.to_string();
					}
				}
				Event::Tag("path", svg::node::element::tag::Type::Empty, attributes) => {
					let data = attributes.get("d").map(|s| s.to_string()).expect("Path data not found");
					let fill_rule = attributes.get("fill-rule").map(|v| v.to_string()).unwrap_or_else(|| "nonzero".to_string());
					paths.push((data, fill_rule));

					// Store attributes of the first path
					if first_path_attributes.is_empty() {
						for (key, value) in attributes.iter() {
							if key != "d" && key != "id" {
								first_path_attributes.push_str(&format!("{}=\"{}\" ", key, value));
							}
						}
					}
				}
				_ => {}
			}
		}

		if (width.is_empty() || height.is_empty()) && !view_box.is_empty() {
			let vb: Vec<&str> = view_box.split_whitespace().collect();
			if vb.len() == 4 {
				width = vb[2].to_string();
				height = vb[3].to_string();
			}
		}

		if width.is_empty() || height.is_empty() {
			panic!("Failed to extract width and height from SVG");
		}

		let a_node = paths[0].clone();
		let b_node = paths[1].clone();

		let a = path_from_path_data(&a_node.0).unwrap();
		let b = path_from_path_data(&b_node.0).unwrap();

		let a_fill_rule = get_fill_rule(&a_node.1);
		let b_fill_rule = get_fill_rule(&b_node.1);

		let result = path_boolean::path_boolean(&a, a_fill_rule, &b, b_fill_rule, op).unwrap();

		// Create the result SVG with correct dimensions and transform
		let mut result_svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{}\">", width, height, view_box);
		if !transform.is_empty() {
			result_svg.push_str(&format!("<g transform=\"{}\">", transform));
		}
		for path in &result {
			result_svg.push_str(&format!("<path d=\"{}\" {}/>", path_to_path_data(path, 1e-4), first_path_attributes));
		}
		if !transform.is_empty() {
			result_svg.push_str("</g>");
		}
		result_svg.push_str("</svg>");

		// Save the result SVG
		let destination_path = dir.join("test-results").join(format!("{}-ours.svg", op_name));
		fs::write(&destination_path, &result_svg).expect("Failed to write result SVG");

		// Render and compare images
		let ground_truth_path = dir.join(format!("{}.svg", op_name));
		let ground_truth_svg = fs::read_to_string(&ground_truth_path).expect("Failed to read ground truth SVG");

		let ours_image = render_svg(&result_svg);
		let ground_truth_image = render_svg(&ground_truth_svg);

		let ours_png_path = dir.join("test-results").join(format!("{}-ours.png", op_name));
		ours_image.save(&ours_png_path).expect("Failed to save our PNG");

		let ground_truth_png_path = dir.join("test-results").join(format!("{}.png", op_name));
		ground_truth_image.save(&ground_truth_png_path).expect("Failed to save ground truth PNG");

		failure |= compare_images(&ours_image, &ground_truth_image, TOLERANCE);

		// Check the number of paths
		let result_path_count = result.len();
		let ground_truth_path_count = ground_truth_svg.matches("<path").count();
		if result_path_count != ground_truth_path_count {
			failure = true;
			eprintln!("Number of paths doesn't match for test: {}", test_name);
		}
	}
	if failure {
		panic!("Some tests have failed");
	}
}

fn render_svg(svg_code: &str) -> DynamicImage {
	let opts = Options::default();
	let tree = Tree::from_str(svg_code, &opts).unwrap();
	let pixmap_size = tree.size();
	let (width, height) = (pixmap_size.width() as u32, pixmap_size.height() as u32);
	let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).unwrap();
	let mut pixmap_mut = pixmap.as_mut();
	render(&tree, Transform::default(), &mut pixmap_mut);
	DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, pixmap.data().to_vec()).unwrap())
}

fn compare_images(img1: &DynamicImage, img2: &DynamicImage, tolerance: u8) -> bool {
	assert_eq!(img1.dimensions(), img2.dimensions(), "Image dimensions do not match");

	for (x, y, pixel1) in img1.pixels() {
		let pixel2 = img2.get_pixel(x, y);
		for i in 0..4 {
			let difference = (pixel1[i] as i32 - pixel2[i] as i32).unsigned_abs() as u8;
			if difference > tolerance {
				println!("Difference {} larger than tolerance {} at [{}, {}], channel {}.", difference, tolerance, x, y, i);
				return true;
			}

			assert!(
				difference <= tolerance,
				"Difference {} larger than tolerance {} at [{}, {}], channel {}.",
				difference,
				tolerance,
				x,
				y,
				i
			);
		}
	}
	false
}
