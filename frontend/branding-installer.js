/* eslint-disable no-console */

import crypto from "crypto";
import fs from "fs";
import http from "http";
import https from "https";
import path from "path";
import { fileURLToPath } from "url";
import zlib from "zlib";

import * as tar from "tar";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const BRANDING_INFO_FILE = path.join(__dirname, "../.branding");
const BRANDING_DIR = path.join(__dirname, "../branding");
const INSTALLED_BRANDING_INFO_FILE = path.join(BRANDING_DIR, ".branding");
const TEMP_FILE = path.join(__dirname, "branding_download.tar.gz");

function downloadFile(url, dest) {
	return new Promise((resolve, reject) => {
		const file = fs.createWriteStream(dest);
		const protocol = url.startsWith("https") ? https : http;

		const request = protocol.get(url, (response) => {
			if (response.statusCode === 301 || response.statusCode === 302 || response.statusCode === 307) {
				file.close();
				fs.unlink(dest, () => {});
				if (response.headers.location) {
					downloadFile(response.headers.location, dest).then(resolve).catch(reject);
				} else {
					reject(new Error("Redirect location missing"));
				}
				return;
			}

			if (response.statusCode !== 200) {
				file.close();
				fs.unlink(dest, () => {});
				reject(new Error(`Failed to download: ${response.statusCode}`));
				return;
			}

			response.pipe(file);
			file.on("finish", () => {
				file.close(resolve);
			});
		});

		request.on("error", (err) => {
			fs.unlink(dest, () => {});
			reject(err);
		});
	});
}

async function main() {
	if (!fs.existsSync(BRANDING_INFO_FILE)) {
		console.error(`Branding info file not found at ${BRANDING_INFO_FILE}`);
		process.exit(1);
	}

	const content = fs.readFileSync(BRANDING_INFO_FILE, "utf8");

	if (fs.existsSync(INSTALLED_BRANDING_INFO_FILE)) {
		const installedContent = fs.readFileSync(INSTALLED_BRANDING_INFO_FILE, "utf8");
		if (content === installedContent) {
			console.log("Branding assets are up to date.");
			return;
		}
	}

	const lines = content
		.split("\n")
		.map((l) => l.trim())
		.filter((l) => l.length > 0);

	if (lines.length < 2) {
		console.error("Branding file must contain at least two lines: URL and Hash");
		process.exit(1);
	}

	const url = lines[0];
	const expectedHash = lines[1];

	console.log(`Downloading branding assets from <${url}>...`);

	try {
		await downloadFile(url, TEMP_FILE);
	} catch (err) {
		console.error("Download failed:", err);
		process.exit(1);
	}

	console.log("Download complete. Verifying hash...");

	const fileBuffer = fs.readFileSync(TEMP_FILE);
	const hashSum = crypto.createHash("sha256");
	hashSum.update(fileBuffer);
	const hex = hashSum.digest("hex");

	if (hex !== expectedHash) {
		console.error("Hash mismatch!");
		console.error(`Expected: ${expectedHash}`);
		console.error(`Actual:   ${hex}`);
		if (fs.existsSync(TEMP_FILE)) fs.unlinkSync(TEMP_FILE);
		process.exit(1);
	}

	console.log("Hash verified. Extracting...");

	if (fs.existsSync(BRANDING_DIR)) {
		fs.rmSync(BRANDING_DIR, { recursive: true, force: true });
	}
	fs.mkdirSync(BRANDING_DIR, { recursive: true });

	try {
		// Extract the tar.gz file
		await new Promise((resolve, reject) => {
			fs.createReadStream(TEMP_FILE)
				.pipe(zlib.createGunzip())
				.pipe(
					tar.x({
						cwd: BRANDING_DIR,
						strip: 1,
					}),
				)
				.on("error", reject)
				.on("finish", resolve);
		});
		fs.copyFileSync(BRANDING_INFO_FILE, INSTALLED_BRANDING_INFO_FILE);
		console.log("Extraction complete.");
	} catch (error) {
		console.error("Failed to extract archive:", error);
	} finally {
		if (fs.existsSync(TEMP_FILE)) {
			fs.unlinkSync(TEMP_FILE);
		}
	}
}

main()
	.then(() => process.exit(0))
	.catch((err) => {
		console.error("An error occurred:", err);
		process.exit(1);
	});
