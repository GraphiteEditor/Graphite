/* eslint-disable no-console */

import fs from "fs";
import type { IncomingMessage } from "http";
import https from "https";
import path from "path";

import * as tar from "tar";

// Define basePath as the directory of the current script
const basePath = import.meta.dirname;

// Define files to copy as [source, destination] pairs
// Files with the same destination will be concatenated
const FILES_TO_COPY = [
	["../node_modules/@fontsource-variable/inter/opsz.css", "../static/fonts/common.css"],
	["../node_modules/@fontsource-variable/inter/opsz-italic.css", "../static/fonts/common.css"],
	["../node_modules/@fontsource/bona-nova/700.css", "../static/fonts/common.css"],
];

// Define directories to copy recursively as [source, destination] pairs
const DIRECTORIES_TO_COPY = [
	["../node_modules/@fontsource-variable/inter/files", "../static/fonts/files"],
	["../node_modules/@fontsource/bona-nova/files", "../static/fonts/files"],
];

// Track processed destination files and CSS content
const processedDestinations = new Set();
const cssDestinations = new Set<string>();
const allCopiedFiles = new Set<string>();

// Process each file
FILES_TO_COPY.forEach(([source, dest]) => {
	// Convert relative paths to absolute paths
	const sourcePath = path.join(basePath, source);
	const destPath = path.join(basePath, dest);

	// Track CSS destinations for later analysis
	if (dest.endsWith(".css")) {
		cssDestinations.add(destPath);
	}

	// Ensure destination directory exists
	const destDir = path.dirname(destPath);
	if (!fs.existsSync(destDir)) {
		fs.mkdirSync(destDir, { recursive: true });
		console.log(`Created directory: ${destDir}`);
	}

	try {
		// Read source file content
		const content = fs.readFileSync(sourcePath, "utf8");

		// Check if destination has been processed before
		if (processedDestinations.has(destPath)) {
			// Append to existing file
			fs.appendFileSync(destPath, "\n\n" + content);
			console.log(`Appended: ${sourcePath} → ${destPath}`);
		} else {
			// First time writing to this destination - copy the file
			fs.writeFileSync(destPath, content);
			processedDestinations.add(destPath);
			console.log(`Copied: ${sourcePath} → ${destPath}`);
		}

		// Replace all occurrences of "./files" with "/fonts" in the destination file
		let destFileContent = fs.readFileSync(destPath, "utf8");
		destFileContent = destFileContent.replaceAll("./files/", "/fonts/files/");
		fs.writeFileSync(destPath, destFileContent);
	} catch (error) {
		console.error(`Error processing ${sourcePath} to ${destPath}:`, error);
		process.exit(1);
	}
});

function copyDirectoryRecursive(source: string, destination: string) {
	// Ensure destination directory exists
	if (!fs.existsSync(destination)) {
		fs.mkdirSync(destination, { recursive: true });
		console.log(`Created directory: ${destination}`);
	}

	// Get all items in the source directory
	const items = fs.readdirSync(source);

	// Process each item
	items.forEach((item) => {
		const sourcePath = path.join(source, item);
		const destPath = path.join(destination, item);

		// Check if item is a directory or file
		const stats = fs.statSync(sourcePath);
		if (stats.isDirectory()) {
			// Recursively copy subdirectory
			copyDirectoryRecursive(sourcePath, destPath);
		} else {
			// Copy file and track it
			fs.copyFileSync(sourcePath, destPath);
			allCopiedFiles.add(destPath);
			console.log(`Copied: ${sourcePath} → ${destPath}`);
		}
	});
}

// Process each directory
DIRECTORIES_TO_COPY.forEach(([source, dest]) => {
	// Convert relative paths to absolute paths
	const sourcePath = path.join(basePath, source);
	const destPath = path.join(basePath, dest);

	try {
		copyDirectoryRecursive(sourcePath, destPath);
		console.log(`Copied directory: ${sourcePath} → ${destPath}`);
	} catch (error) {
		console.error(`Error copying directory ${sourcePath} to ${destPath}:`, error);
		process.exit(1);
	}
});

console.log("All files and directories copied successfully!");

// Now check which of the copied files are actually referenced in CSS
console.log("\nChecking for unused font files...");

// Read all CSS content and join it
let allCssContent = "";
cssDestinations.forEach((cssPath) => {
	try {
		const content = fs.readFileSync(cssPath, "utf8");
		allCssContent += content;
	} catch (error) {
		console.error(`Error reading CSS file ${cssPath}:`, error);
	}
});

// Filter files that aren't referenced in CSS
const unusedFiles: string[] = [];
allCopiedFiles.forEach((filePath) => {
	const fileName = path.basename(filePath);

	// Check if the file name is mentioned in any CSS
	if (!allCssContent.includes(fileName)) {
		unusedFiles.push(filePath);
	}
});

// Delete unused files
if (unusedFiles.length > 0) {
	console.log(`Found ${unusedFiles.length} unused font files to delete:`);
	unusedFiles.forEach((filePath) => {
		try {
			fs.unlinkSync(filePath);
			console.log(`Deleted unused file: ${filePath}`);
		} catch (error) {
			console.error(`Error deleting file ${filePath}:`, error);
		}
	});
} else {
	console.log("No unused font files found.");
}

console.log("\nFont installation complete!");

// Fetch and save text-balancer.js, which we don't commit to the repo so we're not version controlling dependency code
const textBalancerUrl = "https://static.graphite.art/text-balancer/text-balancer.js";
const textBalancerDest = path.join(basePath, "../static", "text-balancer.js");
console.log("\nDownloading text-balancer.js...");
https
	.get(textBalancerUrl, (res) => {
		if (res.statusCode !== 200) {
			console.error(`Failed to download text-balancer.js. Status code: ${res.statusCode}`);
			res.resume();
			return;
		}

		let data = "";
		res.on("data", (chunk) => {
			data += chunk;
		});

		res.on("end", () => {
			try {
				// Ensure destination directory exists
				const destDir = path.dirname(textBalancerDest);
				if (!fs.existsSync(destDir)) {
					fs.mkdirSync(destDir, { recursive: true });
					console.log(`Created directory: ${destDir}`);
				}
				fs.writeFileSync(textBalancerDest, data, "utf8");
				console.log(`Downloaded and saved: ${textBalancerDest}`);
				res.destroy(); // Close the connection
			} catch (error) {
				console.error("Error saving text-balancer.js:", error);
				res.destroy(); // Close the connection
			}
		});
	})
	.on("error", (err) => {
		console.error("Error downloading text-balancer.js:", err);
	});

// Fetch all favicon files from the /favicons directory of the Graphite Branded Assets repo and save them within ../static/
// The URL of the repo is the first line of ../../.branding which is a .tar.gz file that we extract with the "tar" npm package
const brandingFilePath = path.join(basePath, "..", "..", ".branding");
if (!fs.existsSync(brandingFilePath)) console.error("\nThe `.branding` file was not found");
const brandingRepoUrl = fs.readFileSync(brandingFilePath, "utf8").split("\n")[0].trim();
console.log("\nFetching favicons from branding repo:", brandingRepoUrl);
downloadWithRedirects(
	brandingRepoUrl,
	(res) => {
		if (res.statusCode !== 200) {
			console.error(`Failed to download branding repo. Status code: ${res.statusCode}`);
			res.resume();
			return;
		}

		// Pipe the response stream into tar to extract only the /favicons directory
		const extract = tar.extract({
			cwd: path.join(basePath, "../static/"),
			filter: (path) => path.includes("/favicons/"),
			strip: 2, // Remove leading directory components
		});

		res.pipe(extract);

		extract.on("finish", () => {
			console.log("Favicons extracted to ../static/ successfully!");
			res.destroy(); // Close the connection
		});

		extract.on("error", (err) => {
			console.error("Error extracting favicons:", err);
			res.destroy(); // Close the connection
		});
	},
	(err) => console.error("Error downloading branding repo:", err),
);
function downloadWithRedirects(url: string, callback: (res: IncomingMessage) => void, errorCallback: (err: Error) => void) {
	https
		.get(url, (res) => {
			if (res.statusCode === 302 || res.statusCode === 301) {
				console.log("Redirected to:", res.headers.location);
				res.destroy(); // Close the connection

				// Follow the redirect
				return downloadWithRedirects(res.headers.location || "", callback, errorCallback);
			}
			console.log("Final URL reached:", url);
			callback(res);
		})
		.on("error", errorCallback);
}
