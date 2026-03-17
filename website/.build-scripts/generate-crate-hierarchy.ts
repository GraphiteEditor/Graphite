/* eslint-disable no-console */

import fs from "fs";

import { instance } from "@viz-js/viz";

const [inputFile, outputFile] = process.argv.slice(2);
if (!inputFile || !outputFile) {
	console.error("Usage: node generate-crate-hierarchy.ts <input.dot> <output.svg>");
	process.exit(1);
}

const dot = fs.readFileSync(inputFile, "utf-8");

const viz = await instance();
const svg = viz.renderString(dot, { format: "svg" });

fs.writeFileSync(outputFile, svg);
console.log(`SVG output written to: ${outputFile}`);
