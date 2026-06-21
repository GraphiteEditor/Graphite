use cargo_run::Error;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

pub fn usage() {
	eprintln!();
	eprintln!("USAGE:");
	eprintln!("  cargo run -p cargo-run-internal-download -- <URL> <SHA256> <OUT> \\");
	eprintln!("      [--extract] [--strip <N>] [--include <PREFIX>]...");
	eprintln!();
	eprintln!("Args:");
	eprintln!("  <URL>               HTTPS source to download");
	eprintln!("  <SHA256>            Expected SHA-256 of the response body (64 hex digits)");
	eprintln!("  <OUT>               Destination file (default) or directory (with --extract)");
	eprintln!("  --extract           Decompress the body as tar.gz and extract into <OUT>");
	eprintln!("  --strip <N>         Strip N leading path components from each entry (requires --extract)");
	eprintln!("  --include <PREFIX>  Only extract entries whose stripped path starts with PREFIX, repeatable (requires --extract)");
	eprintln!();
}

fn main() -> ExitCode {
	if let Err(e) = parse_args().inspect_err(|_| usage()).and_then(run) {
		eprintln!("Error: {e}");
		return ExitCode::FAILURE;
	}
	ExitCode::SUCCESS
}

struct Args {
	url: String,
	sha256: String,
	out: PathBuf,
	mode: Mode,
}

enum Mode {
	File,
	ExtractTarGz { strip: usize, include: Vec<String> },
}

fn run(Args { url, sha256, out, mode }: Args) -> Result<(), Error> {
	eprintln!("Downloading {url}");
	let mut body = Vec::new();
	ureq::get(&url)
		.call()
		.map_err(|e| Error::Io(io::Error::other(e), format!("HTTP GET {url}")))?
		.into_body()
		.into_with_config()
		.reader()
		.read_to_end(&mut body)
		.map_err(|e| Error::Io(io::Error::other(e), "reading response body".into()))?;

	let actual = Sha256::digest(&body).iter().map(|b| format!("{b:02x}")).collect::<String>();
	if !actual.eq_ignore_ascii_case(&sha256) {
		eprintln!("SHA-256 mismatch:");
		eprintln!("  expected: {sha256}");
		eprintln!("  actual:   {actual}");
		return Err(Error::Io(io::Error::other("SHA-256 mismatch"), "verifying download".into()));
	}

	match mode {
		Mode::File => write_to_file(&body, &out),
		Mode::ExtractTarGz { strip, include } => extract_tar_gz(&body, &out, strip, &include),
	}
}

fn write_to_file(body: &[u8], path: &Path) -> Result<(), Error> {
	if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
		std::fs::create_dir_all(parent).map_err(|e| Error::Io(e, format!("creating '{}'", parent.display())))?;
	}
	std::fs::write(path, body).map_err(|e| Error::Io(e, format!("writing '{}'", path.display())))?;
	eprintln!("Wrote {}", path.display());
	Ok(())
}

// TODO: support other compression/archive formats
fn extract_tar_gz(body: &[u8], dir: &Path, strip: usize, include: &[String]) -> Result<(), Error> {
	std::fs::create_dir_all(dir).map_err(|e| Error::Io(e, format!("creating '{}'", dir.display())))?;
	let mut archive = tar::Archive::new(GzDecoder::new(body));
	archive.set_preserve_permissions(true);
	for entry in archive.entries().map_err(|e| Error::Io(e, "reading tar archive".into()))? {
		let mut entry = entry.map_err(|e| Error::Io(e, "reading tar entry".into()))?;
		let entry_path = entry.path().map_err(|e| Error::Io(e, "reading tar entry path".into()))?.into_owned();
		let Some(stripped) = strip_components(&entry_path, strip) else {
			continue;
		};
		if stripped.as_os_str().is_empty() || stripped.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
			continue;
		}
		if !include.is_empty() {
			let s = stripped.to_string_lossy().replace('\\', "/");
			if !include.iter().any(|p| s.starts_with(p.as_str())) {
				continue;
			}
		}
		let entry_type = entry.header().entry_type();
		if entry_type.is_symlink() || entry_type.is_hard_link() {
			continue;
		}
		let target = dir.join(&stripped);
		if let Some(parent) = target.parent() {
			std::fs::create_dir_all(parent).map_err(|e| Error::Io(e, format!("creating '{}'", parent.display())))?;
		}
		entry.unpack(&target).map_err(|e| Error::Io(e, format!("unpacking '{}'", target.display())))?;
	}
	Ok(())
}

fn strip_components(path: &Path, n: usize) -> Option<PathBuf> {
	let mut comps = path.components();
	for _ in 0..n {
		comps.next()?;
	}
	Some(comps.as_path().to_path_buf())
}

fn parse_args() -> Result<Args, Error> {
	fn arg_err(msg: impl Into<String>) -> Error {
		Error::Io(io::Error::new(io::ErrorKind::InvalidInput, msg.into()), "invalid arguments".into())
	}

	let mut args = std::env::args().skip(1);
	let url = args.next().ok_or_else(|| arg_err("URL is required (first positional argument)"))?;
	let sha256 = args.next().ok_or_else(|| arg_err("SHA-256 is required (second positional argument)"))?;
	if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
		return Err(arg_err("SHA-256 must be 64 hex digits"));
	}
	let out = PathBuf::from(args.next().ok_or_else(|| arg_err("OUT is required (third positional argument)"))?);

	let mut extract = false;
	let mut strip: usize = 0;
	let mut include: Vec<String> = Vec::new();

	while let Some(arg) = args.next() {
		match arg.as_str() {
			"--extract" => extract = true,
			"--strip" => {
				let v = args.next().ok_or_else(|| arg_err("'--strip' requires a value"))?;
				strip = v.parse().map_err(|_| arg_err(format!("--strip must be a non-negative integer, got '{v}'")))?;
			}
			"--include" => include.push(args.next().ok_or_else(|| arg_err("'--include' requires a value"))?),
			other => return Err(arg_err(format!("unknown flag '{other}'"))),
		}
	}

	let mode = if extract {
		Mode::ExtractTarGz { strip, include }
	} else {
		if strip != 0 {
			return Err(arg_err("--strip is only valid with --extract"));
		}
		if !include.is_empty() {
			return Err(arg_err("--include is only valid with --extract"));
		}
		Mode::File
	};

	Ok(Args { url, sha256, out, mode })
}
