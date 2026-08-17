use super::InstallAction;
use crate::cmd::prelude::*;
use crate::{install_dir, workspace_dir};

/// Pinned Binaryen release used by [`install_action`].
/// NOTICE: keep in sync with the `BINARYEN_VERSION` pinned across the CI workflows, and update [`SHA256`] below.
const VERSION: &str = "130";
const SHA256: &[(&str, &str)] = &[
	("x86_64-windows", "cc09c874f4332d00aa32ab72745a9b98c9a172f795762f21d03e70638a3f7f4c"),
	("arm64-windows", "b18c9cbe000562b1ee5d9cb60146616a949aca504903ad63f27fd9fd679898a7"),
	("arm64-macos", "79d3ab9f417d9e215f15f598f523d001a7d9ac1e59367e5c869fbdabd1cba72e"),
	("x86_64-macos", "d3e2d1235b70c93c54b52eabc1625ea960965152218754f1f4eeb0f873c48e03"),
	("x86_64-linux", "0a18362361ad05465118cd8eeb72edaeec89de6894bc283576ef4e07aa3babcc"),
	("aarch64-linux", "e6ae6e09ac40f4e14bc5be6f687c58e2995c84170013975fa641809dd3b480a0"),
];

fn url_for(platform: &str) -> String {
	format!("https://github.com/WebAssembly/binaryen/releases/download/version_{VERSION}/binaryen-version_{VERSION}-{platform}.tar.gz")
}

pub fn install_action() -> InstallAction {
	let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
		("windows", "x86_64") => "x86_64-windows",
		("windows", "aarch64") => "arm64-windows",
		("macos", "aarch64") => "arm64-macos",
		("macos", "x86_64") => "x86_64-macos",
		("linux", "x86_64") => "x86_64-linux",
		("linux", "aarch64") => "aarch64-linux",
		_ => return InstallAction::None,
	};
	let url = url_for(platform);
	let Some(sha256) = SHA256.iter().find_map(|(p, s)| (*p == platform).then_some(*s)) else {
		return InstallAction::None;
	};
	let out = install_dir().to_string_lossy().into_owned();
	let description = format!("Download wasm-opt {VERSION} from {url} (sha256 {sha256})");

	let args = [&url, sha256, &out, "--extract", "--strip", "1", "--include", "bin/wasm-opt"];

	#[cfg(target_os = "macos")]
	let args = args.into_iter().chain(["--include", "lib/libbinaryen.dylib"]).collect::<Vec<_>>();

	let expression = utils::internal("download").args(args).dir(workspace_dir());

	InstallAction::Expression { description, expression }
}
