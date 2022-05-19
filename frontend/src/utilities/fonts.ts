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
			const variants = font.variants.map(formatFontStyleName);
			const files = new Map(font.variants.map((x) => [formatFontStyleName(x), font.files[x]]));
			return { family, variants, files };
		});

		loadDefaultFont();
	});

function formatFontStyleName(fontStyle: string): string {
	const isItalic = fontStyle.endsWith("italic");
	const weight = fontStyle === "regular" || fontStyle === "italic" ? 400 : parseInt(fontStyle, 10);
	let weightName = "";

	let bestWeight = Infinity;
	weightNameMapping.forEach((nameChecking, weightChecking) => {
		if (Math.abs(weightChecking - weight) < bestWeight) {
			bestWeight = Math.abs(weightChecking - weight);
			weightName = nameChecking;
		}
	});

	return `${weightName}${isItalic ? " Italic" : ""} (${weight})`;
}

export async function loadDefaultFont(): Promise<void> {
	const font = getFontFile("Merriweather", "Normal (400)");
	if (!font) return;

	const response = await fetch(font);
	const responseBuffer = await response.arrayBuffer();
	loadDefaultFontCallback?.(font, new Uint8Array(responseBuffer));
}

export function setLoadDefaultFontCallback(callback: fontCallbackType): void {
	loadDefaultFontCallback = callback;
	loadDefaultFont();
}

function createURL(font: string): URL {
	const url = new URL("https://fonts.googleapis.com/css2");
	url.searchParams.set("display", "swap");
	url.searchParams.set("family", font);
	url.searchParams.set("text", font);
	return url;
}

export function fontNames(): { name: string; url: URL | undefined }[] {
	return fontList.map((font) => ({ name: font.family, url: createURL(font.family) }));
}

export function getFontStyles(fontFamily: string): { name: string; url: URL | undefined }[] {
	const font = fontList.find((value) => value.family === fontFamily);
	return font?.variants.map((variant) => ({ name: variant, url: undefined })) || [];
}

export function getFontFile(fontFamily: string, fontStyle: string): string | undefined {
	const font = fontList.find((value) => value.family === fontFamily);
	const fontFile = font?.files.get(fontStyle);
	return fontFile?.replace("http://", "https://");
}
