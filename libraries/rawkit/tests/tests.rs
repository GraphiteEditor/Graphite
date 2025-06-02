// Only compile this file if the feature "rawkit-tests" is enabled
#![cfg(feature = "rawkit-tests")]

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ColorType, ImageEncoder};
use libraw::Processor;
use rawkit::RawImage;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fmt::Write;
use std::fs::{File, create_dir, metadata, read_dir};
use std::io::{BufWriter, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const TEST_FILES: [&str; 3] = ["ILCE-7M3-ARW2.3.5-blossoms.arw", "ILCE-7RM4-ARW2.3.5-kestrel.arw", "ILCE-6000-ARW2.3.1-windsock.arw"];
const BASE_URL: &str = "https://static.graphite.rs/test-data/libraries/rawkit/";
const BASE_PATH: &str = "./tests/images/";

#[test]
fn test_images_match_with_libraw() {
	download_images();

	let paths: Vec<_> = read_dir(BASE_PATH)
		.unwrap()
		.map(|dir_entry| dir_entry.unwrap().path())
		.filter(|path| path.is_file() && path.file_name().map(|file_name| file_name != ".gitkeep").unwrap_or(false))
		.collect();

	let failed_tests = if std::env::var("RAWKIT_TEST_RUN_SEQUENTIALLY").is_ok() {
		let mut failed_tests = 0;

		paths.iter().for_each(|path| {
			if !test_image(path) {
				failed_tests += 1;
			}
		});

		failed_tests
	} else {
		let failed_tests = AtomicUsize::new(0);

		paths.par_iter().for_each(|path| {
			if !test_image(path) {
				failed_tests.fetch_add(1, Ordering::SeqCst);
			}
		});

		failed_tests.load(Ordering::SeqCst)
	};

	if failed_tests != 0 {
		panic!("{} images have failed the tests", failed_tests);
	}
}

fn test_image(path: &Path) -> bool {
	let mut f = File::open(path).unwrap();
	let mut content = vec![];
	f.read_to_end(&mut content).unwrap();

	let raw_image = match test_raw_data(&content) {
		Err(err_msg) => {
			println!("{} => {}", path.display(), err_msg);
			return false;
		}
		Ok(raw_image) => raw_image,
	};

	// TODO: The code below is kept commented because raw data to final image processing is
	// incomplete. Remove this once it is done.

	// if let Err(err_msg) = test_final_image(&content, raw_image) {
	// 	failed_tests += 1;
	// 	return println!("{}", err_msg);
	// };

	println!("{} => Passed", path.display());

	// TODO: Remove this later
	let mut image = raw_image.process_8bit();
	store_image(path, "rawkit", &mut image.data, image.width, image.height);

	let processor = Processor::new();
	let libraw_image = processor.process_8bit(&content).unwrap();
	let mut data = Vec::from_iter(libraw_image.iter().copied());
	store_image(path, "libraw_rs", &mut data[..], libraw_image.width() as usize, libraw_image.height() as usize);

	true
}

fn store_image(path: &Path, suffix: &str, data: &mut [u8], width: usize, height: usize) {
	let mut output_path = PathBuf::new();
	if let Some(parent) = path.parent() {
		output_path.push(parent);
	}
	output_path.push("output");

	if metadata(&output_path).is_err() {
		create_dir(&output_path).unwrap();
	}

	if let Some(filename) = path.file_stem() {
		let new_filename = format!("{}_{}.{}", filename.to_string_lossy(), suffix, "png");
		output_path.push(new_filename);
	}
	output_path.set_extension("png");

	let file = BufWriter::new(File::create(output_path).unwrap());
	let png_encoder = PngEncoder::new_with_quality(file, CompressionType::Fast, FilterType::Adaptive);
	png_encoder.write_image(data, width as u32, height as u32, ColorType::Rgb8.into()).unwrap();
}

fn download_images() {
	let mut path = Path::new(BASE_PATH).to_owned();
	let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(60 * 5)).build().unwrap();

	for filename in TEST_FILES {
		path.push(filename);
		if !path.exists() {
			let url = BASE_URL.to_owned() + filename;
			let mut response = client.get(url).send().unwrap();
			let mut file = File::create(BASE_PATH.to_owned() + filename).unwrap();
			std::io::copy(&mut response, &mut file).unwrap();
		}
		path.pop();
	}
}

fn test_raw_data(content: &[u8]) -> Result<RawImage, String> {
	let processor = libraw::Processor::new();
	let libraw_raw_image = processor.decode(content).unwrap();

	let mut content = Cursor::new(content);
	let raw_image = RawImage::decode(&mut content).unwrap();

	if libraw_raw_image.sizes().raw_height as usize != raw_image.height {
		return Err(format!(
			"The height of raw image is {} but the expected value was {}",
			raw_image.height,
			libraw_raw_image.sizes().raw_height
		));
	}

	if libraw_raw_image.sizes().raw_width as usize != raw_image.width {
		return Err(format!(
			"The width of raw image is {} but the expected value was {}",
			raw_image.width,
			libraw_raw_image.sizes().raw_width
		));
	}

	if (*libraw_raw_image).len() != raw_image.data.len() {
		return Err(format!(
			"The size of data of raw image is {} but the expected value was {}",
			raw_image.data.len(),
			(*libraw_raw_image).len()
		));
	}

	if (*libraw_raw_image) != raw_image.data {
		let mut err_msg = String::new();

		write!(&mut err_msg, "The raw data does not match").unwrap();

		if std::env::var("RAWKIT_TEST_PRINT_HISTOGRAM").is_ok() {
			writeln!(err_msg).unwrap();

			let mut histogram: HashMap<i32, usize> = HashMap::new();
			let mut non_zero_count: usize = 0;

			(*libraw_raw_image)
				.iter()
				.zip(raw_image.data.iter())
				.map(|(&a, &b)| {
					let a: i32 = a.into();
					let b: i32 = b.into();
					a - b
				})
				.filter(|&x| x != 0)
				.for_each(|x| {
					*histogram.entry(x).or_default() += 1;
					non_zero_count += 1;
				});

			let total_pixels = raw_image.height * raw_image.width;
			writeln!(err_msg, "{} ({:.5}%) pixels are different from expected", non_zero_count, non_zero_count as f64 / total_pixels as f64).unwrap();

			writeln!(err_msg, "Diff Histogram:").unwrap();
			let mut items: Vec<_> = histogram.iter().map(|(&a, &b)| (a, b)).collect();
			items.sort();
			for (key, value) in items {
				writeln!(err_msg, "{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64).unwrap();
			}
		}

		return Err(err_msg);
	}

	Ok(raw_image)
}

fn _test_final_image(content: &[u8], raw_image: RawImage) -> Result<(), String> {
	let processor = libraw::Processor::new();
	let libraw_image = processor.process_8bit(content).unwrap();

	let image = raw_image.process_8bit();

	if libraw_image.height() as usize != image.height {
		return Err(format!("The height of image is {} but the expected value was {}", image.height, libraw_image.height()));
	}

	if libraw_image.width() as usize != image.width {
		return Err(format!("The width of image is {} but the expected value was {}", image.width, libraw_image.width()));
	}

	if (*libraw_image).len() != image.data.len() {
		return Err(format!("The size of data of image is {} but the expected value was {}", image.data.len(), (*libraw_image).len()));
	}

	if (*libraw_image) != image.data {
		let mut err_msg = String::new();

		write!(&mut err_msg, "The final image does not match").unwrap();

		if std::env::var("RAWKIT_TEST_PRINT_HISTOGRAM").is_ok() {
			writeln!(err_msg).unwrap();

			let mut histogram_red: HashMap<i16, usize> = HashMap::new();
			let mut histogram_green: HashMap<i16, usize> = HashMap::new();
			let mut histogram_blue: HashMap<i16, usize> = HashMap::new();
			let mut non_zero_count: usize = 0;
			let mut non_zero_count_red: usize = 0;
			let mut non_zero_count_green: usize = 0;
			let mut non_zero_count_blue: usize = 0;

			(*libraw_image)
				.chunks_exact(3)
				.zip(image.data.chunks_exact(3))
				.map(|(a, b)| {
					let a: [u8; 3] = a.try_into().unwrap();
					let b: [u8; 3] = b.try_into().unwrap();
					(a, b)
				})
				.map(|([r1, g1, b1], [r2, g2, b2])| {
					let r1: i16 = r1.into();
					let g1: i16 = g1.into();
					let b1: i16 = b1.into();
					let r2: i16 = r2.into();
					let g2: i16 = g2.into();
					let b2: i16 = b2.into();
					[r1 - r2, g1 - g2, b1 - b2]
				})
				.filter(|&[r, g, b]| r != 0 || g != 0 || b != 0)
				.for_each(|[r, g, b]| {
					non_zero_count += 1;
					if r != 0 {
						*histogram_red.entry(r).or_default() += 1;
						non_zero_count_red += 1;
					}
					if g != 0 {
						*histogram_green.entry(g).or_default() += 1;
						non_zero_count_green += 1;
					}
					if b != 0 {
						*histogram_blue.entry(b).or_default() += 1;
						non_zero_count_blue += 1;
					}
				});

			let total_pixels = image.height * image.width;
			writeln!(err_msg, "{} ({:.5}%) pixels are different from expected", non_zero_count, non_zero_count as f64 / total_pixels as f64,).unwrap();

			writeln!(
				err_msg,
				"{} ({:.5}%) red pixels are different from expected",
				non_zero_count_red,
				non_zero_count_red as f64 / total_pixels as f64,
			)
			.unwrap();

			writeln!(
				err_msg,
				"{} ({:.5}%) green pixels are different from expected",
				non_zero_count_green,
				non_zero_count_green as f64 / total_pixels as f64,
			)
			.unwrap();

			writeln!(
				err_msg,
				"{} ({:.5}%) blue pixels are different from expected",
				non_zero_count_blue,
				non_zero_count_blue as f64 / total_pixels as f64,
			)
			.unwrap();

			writeln!(err_msg, "Diff Histogram for Red pixels:").unwrap();
			let mut items: Vec<_> = histogram_red.iter().map(|(&a, &b)| (a, b)).collect();
			items.sort();
			for (key, value) in items {
				writeln!(err_msg, "{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64).unwrap();
			}

			writeln!(err_msg, "Diff Histogram for Green pixels:").unwrap();
			let mut items: Vec<_> = histogram_green.iter().map(|(&a, &b)| (a, b)).collect();
			items.sort();
			for (key, value) in items {
				writeln!(err_msg, "{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64).unwrap();
			}

			writeln!(err_msg, "Diff Histogram for Blue pixels:").unwrap();
			let mut items: Vec<_> = histogram_blue.iter().map(|(&a, &b)| (a, b)).collect();
			items.sort();
			for (key, value) in items {
				writeln!(err_msg, "{:05}: {:05} ({:02.5}%)", key, value, value as f64 / total_pixels as f64).unwrap();
			}
		}

		return Err(err_msg);
	}

	Ok(())
}

#[ignore]
#[test]
fn extract_data_from_dng_images() {
	read_dir(BASE_PATH)
		.unwrap()
		.map(|dir_entry| dir_entry.unwrap().path())
		.filter(|path| path.is_file() && path.file_name().map(|file_name| file_name != ".gitkeep").unwrap_or(false))
		.for_each(|path| {
			extract_data_from_dng_image(&path);
		});
}

fn extract_data_from_dng_image(path: &Path) {
	use rawkit::tiff::Ifd;
	use rawkit::tiff::file::TiffRead;
	use rawkit::tiff::tags::{ColorMatrix2, Make, Model};
	use rawkit::tiff::values::ToFloat;
	use std::io::{BufReader, Write};

	let reader = BufReader::new(File::open(path).unwrap());
	let mut file = TiffRead::new(reader).unwrap();
	let ifd = Ifd::new_first_ifd(&mut file).unwrap();

	let make = ifd.get_value::<Make, _>(&mut file).unwrap();
	let model = ifd.get_value::<Model, _>(&mut file).unwrap();
	let matrix = ifd.get_value::<ColorMatrix2, _>(&mut file).unwrap();

	if model == "MODEL-NAME" {
		println!("{}", path.display());
		return;
	}

	let output_folder = path.parent().unwrap().join(make);
	std::fs::create_dir_all(&output_folder).unwrap();
	let mut output_file = File::create(output_folder.join(model + ".toml")).unwrap();
	let matrix: Vec<_> = matrix.iter().map(|x| x.to_float()).collect();
	writeln!(output_file, "camera_to_xyz = {:.4?}", matrix).unwrap();
}
