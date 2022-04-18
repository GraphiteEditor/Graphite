const fontListAPI = "https://api.graphite.rs/font-list";

type fontCallbackType = (font: string, data: Uint8Array) => void;

let fontList = [] as { family: string; variants: string[]; files: { [name: string]: string } }[];
let loadDefaultFontCallback = undefined as fontCallbackType | undefined;

fetch(fontListAPI)
	.then((response) => response.json())
	.then((json) => {
		fontList = json.items;
		loadDefaultFont();
	});

export function loadDefaultFont(): void {
	const font = getFontFile("Merriweather", "regular");

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
	return font && font.files[fontStyle].replace("http://", "https://");
}
