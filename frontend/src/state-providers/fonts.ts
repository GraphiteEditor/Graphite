import { reactive, readonly } from "vue";

import { Editor } from "@/interop/editor";
import { TriggerFontLoad, TriggerFontLoadDefault } from "@/interop/messages";

const DEFAULT_FONT = "Merriweather";
const DEFAULT_FONT_STYLE = "Normal (400)";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export async function createFontsState(editor: Editor) {
	const state = reactive({
		fontNames: [] as string[],
	});

	function getFontStyles(fontFamily: string): string[] {
		const font = fontList.find((value) => value.family === fontFamily);
		return font?.variants || [];
	}

	function getFontFileUrl(fontFamily: string, fontStyle: string): string | undefined {
		const font = fontList.find((value) => value.family === fontFamily);
		const fontFile = font?.files.get(fontStyle);
		return fontFile?.replace("http://", "https://");
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

	editor.subscriptions.subscribeJsMessage(TriggerFontLoadDefault, loadDefaultFont);
	async function loadDefaultFont(): Promise<void> {
		const fontFileUrl = getFontFileUrl(DEFAULT_FONT, DEFAULT_FONT_STYLE);
		if (!fontFileUrl) return;

		const response = await fetch(fontFileUrl);
		const responseBuffer = await response.arrayBuffer();
		editor.instance.on_font_load(fontFileUrl, new Uint8Array(responseBuffer), true);
	}
	editor.subscriptions.subscribeJsMessage(TriggerFontLoad, async (triggerFontLoad) => {
		const response = await (await fetch(triggerFontLoad.font_file_url)).arrayBuffer();
		editor.instance.on_font_load(triggerFontLoad.font_file_url, new Uint8Array(response), false);
	});

	const response = await (await fetch(fontListAPI)).json();
	const loadedFonts = response.items as { family: string; variants: string[]; files: { [name: string]: string } }[];

	const fontList: { family: string; variants: string[]; files: Map<string, string> }[] = loadedFonts.map((font) => {
		const { family } = font;
		const variants = font.variants.map(formatFontStyleName);
		const files = new Map(font.variants.map((x) => [formatFontStyleName(x), font.files[x]]));
		return { family, variants, files };
	});
	state.fontNames = fontList.map((value) => value.family);

	await loadDefaultFont();

	return {
		state: readonly(state),
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
