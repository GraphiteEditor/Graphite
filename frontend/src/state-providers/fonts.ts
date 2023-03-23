import { writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import { TriggerFontLoad } from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createFontsState(editor: Editor) {
	// TODO: Do some code cleanup to remove the need for this empty store
	const { subscribe } = writable({});

	function createURL(font: string): URL {
		const url = new URL("https://fonts.googleapis.com/css2");
		url.searchParams.set("display", "swap");
		url.searchParams.set("family", font);
		url.searchParams.set("text", font);
		return url;
	}

	async function fontNames(): Promise<{ name: string; url: URL | undefined }[]> {
		return (await fontList).map((font) => ({ name: font.family, url: createURL(font.family) }));
	}

	async function getFontStyles(fontFamily: string): Promise<{ name: string; url: URL | undefined }[]> {
		const font = (await fontList).find((value) => value.family === fontFamily);
		return font?.variants.map((variant) => ({ name: variant, url: undefined })) || [];
	}

	async function getFontFileUrl(fontFamily: string, fontStyle: string): Promise<string | undefined> {
		const font = (await fontList).find((value) => value.family === fontFamily);
		const fontFileUrl = font?.files.get(fontStyle);
		return fontFileUrl?.replace("http://", "https://");
	}

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

	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(TriggerFontLoad, async (triggerFontLoad) => {
		const url = await getFontFileUrl(triggerFontLoad.font.fontFamily, triggerFontLoad.font.fontStyle);
		if (url) {
			const response = await (await fetch(url)).arrayBuffer();
			editor.instance.onFontLoad(triggerFontLoad.font.fontFamily, triggerFontLoad.font.fontStyle, url, new Uint8Array(response), triggerFontLoad.isDefault);
		} else {
			editor.instance.errorDialog("Failed to load font", `The font ${triggerFontLoad.font.fontFamily} with style ${triggerFontLoad.font.fontStyle} does not exist`);
		}
	});

	const fontList = new Promise<{ family: string; variants: string[]; files: Map<string, string> }[]>((resolve) => {
		fetch(fontListAPI)
			.then((response) => response.json())
			.then((fontListResponse) => {
				const fontListData = fontListResponse.items as { family: string; variants: string[]; files: Record<string, string> }[];
				const result = fontListData.map((font) => {
					const { family } = font;
					const variants = font.variants.map(formatFontStyleName);
					const files = new Map(font.variants.map((x) => [formatFontStyleName(x), font.files[x]]));
					return { family, variants, files };
				});

				resolve(result);
			});
	});

	return {
		subscribe,
		fontNames,
		getFontStyles,
		getFontFileUrl,
	};
}
export type FontsState = ReturnType<typeof createFontsState>;

const fontListAPI = "https://api.graphite.rs/font-list";

// From https://developer.mozilla.org/en-US/docs/Web/CSS/font-weight#common_weight_name_mapping
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
