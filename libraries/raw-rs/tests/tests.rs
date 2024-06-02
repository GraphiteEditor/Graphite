use std::collections::HashMap;
use std::fmt::Write;
use std::fs::{read_dir, File};
use std::io::{Cursor, Read};
use std::path::Path;

use raw_rs::RawImage;

use downloader::{Download, Downloader};
use libraw::Processor;

const TEST_FILES: [&str; 1] = ["ILCE-7M3-ARW2.3.5-blossoms.arw"];
const BASE_URL: &str = "https://static.graphite.rs/test-data/libraries/raw-rs/";
const BASE_PATH: &str = "./tests/images";

#[test]
fn test_images_matches_with_libraw() {
	download_images();

	let mut failed_tests = 0;

	read_dir(BASE_PATH)
		.unwrap()
		.map(|dir_entry| dir_entry.unwrap().path())
		.filter(|path| path.is_file() && path.file_name().map(|file_name| file_name != ".gitkeep").unwrap_or(false))
		.for_each(|path| {
			let mut f = File::open(&path).unwrap();
			let mut content = vec![];
			f.read_to_end(&mut content).unwrap();

			print!("{} => ", path.display());

			let raw_image = match test_raw_data(&content) {
				Err(err_msg) => {
					failed_tests += 1;
					return println!("{}", err_msg);
				}
				Ok(raw_image) => raw_image,
			};

			// TODO: The code below is kept commented because raw data to final image processing is
			// incomplete. Remove this once it is done.

			// if let Err(err_msg) = test_final_image(&content, raw_image) {
			// 	failed_tests += 1;
			// 	return println!("{}", err_msg);
			// };

			println!("Passed");
		});

	if failed_tests != 0 {
		panic!("{} images have failed the tests", failed_tests);
	}
}

fn download_images() {
	let mut path = Path::new(BASE_PATH).to_owned();
	let mut downloads: Vec<Download> = Vec::new();

	for filename in TEST_FILES {
		path.push(filename);
		if !path.exists() {
			let url = BASE_URL.to_owned() + filename;
			downloads.push(Download::new(&url).file_name(Path::new(filename)));
		}
		path.pop();
	}

	let mut downloader = Downloader::builder().download_folder(Path::new(BASE_PATH)).build().unwrap();

	for download_summary in downloader.download(&downloads).unwrap() {
		download_summary.unwrap();
	}
}

fn test_raw_data(content: &[u8]) -> Result<RawImage, String> {
	let processor = Processor::new();
	let libraw_raw_image = processor.decode(content).unwrap();

	let mut content = Cursor::new(content);
	let raw_image = raw_rs::decode(&mut content).unwrap();

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

		if std::env::var("RAW_RS_TEST_PRINT_HISTOGRAM").is_ok() {
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

fn test_final_image(content: &[u8], raw_image: RawImage) -> Result<(), String> {
	let processor = Processor::new();
	let libraw_image = processor.process_8bit(content).unwrap();

	let image = raw_rs::process_8bit(raw_image);

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

		if std::env::var("RAW_RS_TEST_PRINT_HISTOGRAM").is_ok() {
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
