import { reactive, readonly } from "vue";

import { Editor } from "@/wasm-communication/editor";
import { TriggerFontLoad } from "@/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createFontsState(editor: Editor) {
	const state = reactive({
		fontNames: [] as string[],
	});

	async function getFontStyles(fontFamily: string): Promise<string[]> {
		const font = (await fontList).find((value) => value.family === fontFamily);
		return font?.variants || [];
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
		const url = await getFontFileUrl(triggerFontLoad.font.font_family, triggerFontLoad.font.font_style);
		if (url) {
			const response = await (await fetch(url)).arrayBuffer();
			editor.instance.on_font_load(triggerFontLoad.font.font_family, triggerFontLoad.font.font_style, url, new Uint8Array(response), triggerFontLoad.is_default);
		} else {
			editor.instance.error_dialog("Failed to load font", `The font ${triggerFontLoad.font.font_family} with style ${triggerFontLoad.font.font_style} does not exist`);
		}
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
				state.fontNames = result.map((value) => value.family);

				resolve(result);
			});
	});

	return {
		state: readonly(state) as typeof state,
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
