const fontListAPI = "https://api.graphite.rs/font-list";
let fontList = [] as { family: string; variants: string[]; files: { [name: string]: string } }[];

fetch(fontListAPI)
	.then((response) => response.json())
	.then((json) => {
		fontList = json.items;
	});

export function fontNames(): string[] {
	return fontList.map((value) => value.family);
}

export function getFontVariants(name: string): string[] {
	const font = fontList.find((value) => value.family === name);
	return font ? font.variants : [];
}

export function getFontFile(name: string, variant: string): string | undefined {
	const font = fontList.find((value) => value.family === name);
	return font && font.files[variant].replace("http://", "https://");
}
