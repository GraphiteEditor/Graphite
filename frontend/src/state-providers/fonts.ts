import { Editor } from "@/wasm-communication/editor";
import { TriggerFontLoad, TriggerFontLoadDefault } from "@/wasm-communication/messages";

const DEFAULT_FONT = "Merriweather";
const DEFAULT_FONT_STYLE = "Normal (400)";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createFontsState(editor: Editor) {
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
	editor.subscriptions.subscribeJsMessage(TriggerFontLoadDefault, async (): Promise<void> => {
		const fontFileUrl = await getFontFileUrl(DEFAULT_FONT, DEFAULT_FONT_STYLE);
		if (!fontFileUrl) return;

		const response = await fetch(fontFileUrl);
		const responseBuffer = await response.arrayBuffer();
		editor.instance.on_font_load(fontFileUrl, new Uint8Array(responseBuffer), true);
	});
	editor.subscriptions.subscribeJsMessage(TriggerFontLoad, async (triggerFontLoad) => {
		const response = await (await fetch(triggerFontLoad.font_file_url)).arrayBuffer();
		editor.instance.on_font_load(triggerFontLoad.font_file_url, new Uint8Array(response), false);
	});

	const fontList: Promise<{ family: string; variants: string[]; files: Map<string, string> }[]> = new Promise((resolve) => {
		fetch(fontListAPI)
			.then((response) => response.json())
			.then((fontListResponse) => {
				const fontListData = fontListResponse.items as { family: string; variants: string[]; files: { [name: string]: string } }[];
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
