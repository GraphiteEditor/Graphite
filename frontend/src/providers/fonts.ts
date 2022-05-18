import { reactive, readonly } from "vue";

import { Editor } from "@/interop/editor";
import { TriggerFontLoadDefault } from "@/interop/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createFontsState(editor: Editor) {
	const state = reactive({
		fontNames: [] as string[],
	});

	let fontList = [] as { family: string; variants: string[]; files: Map<string, string> }[];

	const loadDefaultFontCallback = (font: string, data: Uint8Array): void => editor.instance.on_font_load(font, data, true);
	editor.subscriptions.subscribeJsMessage(TriggerFontLoadDefault, loadDefaultFont);

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
			state.fontNames = fontList.map((value) => value.family);

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

	async function loadDefaultFont(): Promise<void> {
		const font = getFontFile("Merriweather", "Normal (400)");
		if (!font) return;

		const response = await fetch(font);
		const responseBuffer = await response.arrayBuffer();
		loadDefaultFontCallback?.(font, new Uint8Array(responseBuffer));
	}

	function getFontStyles(fontFamily: string): string[] {
		const font = fontList.find((value) => value.family === fontFamily);
		return font?.variants || [];
	}

	function getFontFile(fontFamily: string, fontStyle: string): string | undefined {
		const font = fontList.find((value) => value.family === fontFamily);
		const fontFile = font?.files.get(fontStyle);
		return fontFile?.replace("http://", "https://");
	}

	return {
		state: readonly(state),
		getFontStyles,
		getFontFile,
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
