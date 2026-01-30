// TODO: Port this script to Rust as part of `tools/editor-message-tree/src/main.rs`

/* eslint-disable no-console */

import fs from "fs";
import path from "path";

type Entry = { level: number; text: string; link: string | undefined };

/// Parses a single line of the input text.
function parseLine(line: string) {
	const linkRegex = /`([^`]+)`$/;
	const linkMatch = line.match(linkRegex);
	let link = undefined;

	if (linkMatch) {
		const filePath = linkMatch[1].replace(/\\/g, "/");
		link = `https://github.com/GraphiteEditor/Graphite/blob/master/${filePath}`;
	}

	const textContent = line
		.replace(/^[\s│├└─]*/, "")
		.replace(linkRegex, "")
		.trim();
	const indentation = line.indexOf(textContent);
	// Each level of indentation is 4 characters.
	const level = Math.floor(indentation / 4);

	return { level, text: textContent, link };
}

/// Recursively builds the HTML list from the parsed nodes.
function buildHtmlList(nodes: Entry[], currentIndex: number, currentLevel: number) {
	if (currentIndex >= nodes.length) {
		return { html: "", nextIndex: currentIndex };
	}

	let html = "<ul>\n";
	let i = currentIndex;

	while (i < nodes.length && nodes[i].level >= currentLevel) {
		const node = nodes[i];

		if (node.level > currentLevel) {
			// This case handles malformed input, skip to next valid line
			i++;
			continue;
		}

		const hasDirectChildren = i + 1 < nodes.length && nodes[i + 1].level > node.level;
		const hasDeeperChildren = hasDirectChildren && i + 2 < nodes.length && nodes[i + 2].level > nodes[i + 1].level;

		const linkHtml = node.link ? `<a href="${node.link}" target="_blank">${path.basename(node.link)}</a>` : "";
		const fieldPieces = node.text.match(/([^:]*):(.*)/);
		let escapedText;
		if (fieldPieces && fieldPieces.length === 3) {
			escapedText = [escapeHtml(fieldPieces[1].trim()), escapeHtml(fieldPieces[2].trim())];
		} else {
			escapedText = [escapeHtml(node.text)];
		}

		let role = "message";
		if (node.link) role = "subsystem";
		else if (hasDeeperChildren) role = "submessage";
		else if (escapedText.length === 2) role = "field";

		const partOfMessageFromNamingConvention = ["Message", "MessageHandler", "MessageContext"].some((suffix) => node.text.replace(/(.*)<.*>/g, "$1").endsWith(suffix));
		const partOfMessageViolatesNamingConvention = node.link && !partOfMessageFromNamingConvention;
		const violatesNamingConvention = partOfMessageViolatesNamingConvention
			? "<span class=\"warn\">(violates naming convention — should end with 'Message', 'MessageHandler', or 'MessageContext')</span>"
			: "";

		if (hasDirectChildren) {
			html += `<li><span class="tree-node"><span class="${role}">${escapedText}</span>${linkHtml}${violatesNamingConvention}</span>`;
			const childResult = buildHtmlList(nodes, i + 1, node.level + 1);
			html += `<div class="nested">${childResult.html}</div></li>\n`;
			i = childResult.nextIndex;
		} else if (role === "field") {
			html += `<li><span class="tree-leaf field">${escapedText[0]}</span>: <span>${escapedText[1]}</span>${linkHtml}</li>\n`;
			i++;
		} else {
			html += `<li><span class="tree-leaf ${role}">${escapedText[0]}</span>${linkHtml}${violatesNamingConvention}</li>\n`;
			i++;
		}
	}

	html += "</ul>\n";
	return { html, nextIndex: i };
}

function escapeHtml(text: string) {
	return text.replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

const inputFile = process.argv[2];
const outputFile = process.argv[3];

if (!inputFile || !outputFile) {
	console.error("Error: Please provide the input text and output HTML file paths as arguments.");
	console.log("Usage: node generate-editor-structure.ts <input txt> <output html>");
	process.exit(1);
}

if (!fs.existsSync(inputFile)) {
	console.error(`Error: File not found at "${inputFile}"`);
	process.exit(1);
}

try {
	const fileContent = fs.readFileSync(inputFile, "utf-8");
	const lines = fileContent.split(/\r?\n/).filter((line) => line.trim() !== "" && !line.startsWith("// filepath:"));
	const parsedNodes = lines.map(parseLine);

	const { html } = buildHtmlList(parsedNodes, 0, 0);

	fs.writeFileSync(outputFile, html, "utf-8");

	console.log(`Successfully generated HTML outline at: ${outputFile}`);
} catch (error) {
	console.error("An error occurred during processing:", error);
	process.exit(1);
}
