const fontListAPI = "https://api.graphite.rs/font-list";

// Taken from https://developer.mozilla.org/en-US/docs/Web/CSS/font-weight#common_weight_name_mapping
const weightNameMapping = new Map([
	[100, "Thin"],
	[200, "Extra Light"],
	[300, "Light"],
	[400, "Normal"],
	[500, "Medium"],
	[600, "Semi Bold"],
	[700, "Bold"],
	[800, "Extra Bold"],
	[900, "Black"],
	[950, "Extra Black"],
]);

type fontCallbackType = (font: string, data: Uint8Array) => void;

let fontList = [] as { family: string; variants: string[]; files: Map<string, string> }[];
let loadDefaultFontCallback = undefined as fontCallbackType | undefined;

fetch(fontListAPI)
	.then((response) => response.json())
	.then((json) => {
		const loadedFonts = json.items as { family: string; variants: string[]; files: { [name: string]: string } }[];
		fontList = loadedFonts.map((font) => {
			const { family } = font;
			const variants = font.variants.map(formatVariantName);
			const files = new Map(font.variants.map((x) => [formatVariantName(x), font.files[x]]));
			return { family, variants, files };
		});
		loadDefaultFont();
	});

function formatVariantName(name: string): string {
	const italic = name.endsWith("italic");
	const weight = name === "regular" || name === "italic" ? 400 : parseInt(name, 10);
	let weightName = "";
	{
		let bestWeight = Infinity;
		weightNameMapping.forEach((nameChecking, weightChecking) => {
			if (Math.abs(weightChecking - weight) < bestWeight) {
				bestWeight = Math.abs(weightChecking - weight);
				weightName = nameChecking;
			}
		});
	}
	return `${italic ? "Italic " : ""}${weightName} (${weight})`;
}

export function loadDefaultFont(): void {
	const font = getFontFile("Merriweather", "Normal (400)");

	if (font)
		fetch(font)
			.then((response) => response.arrayBuffer())
			.then((response) => {
				if (loadDefaultFontCallback) loadDefaultFontCallback(font, new Uint8Array(response));
			});
}

export function setLoadDefaultFontCallback(callback: fontCallbackType): void {
	loadDefaultFontCallback = callback;
	loadDefaultFont();
}

export function fontNames(): string[] {
	return fontList.map((value) => value.family);
}

export function getFontStyles(name: string): string[] {
	const font = fontList.find((value) => value.family === name);
	return font ? font.variants : [];
}

export function getFontFile(name: string, fontStyle: string): string | undefined {
	const font = fontList.find((value) => value.family === name);
	const file = font && font.files.get(fontStyle);
	return file && file.replace("http://", "https://");
}
