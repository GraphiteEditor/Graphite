import { type Editor } from "@graphite/editor";
import { TriggerFontCatalogLoad, TriggerFontDataLoad } from "@graphite/messages";

type ApiResponse = { family: string; variants: string[]; files: Record<string, string> }[];

const FONT_LIST_API = "https://api.graphite.art/font-list";

export function createFontsManager(editor: Editor) {
	// Subscribe to process backend events
	editor.subscriptions.subscribeJsMessage(TriggerFontCatalogLoad, async () => {
		const response = await fetch(FONT_LIST_API);
		const fontListResponse = (await response.json()) as { items: ApiResponse };
		const fontListData = fontListResponse.items;

		const catalog = fontListData.map((font) => {
			const styles = font.variants.map((variant) => {
				const weight = variant === "regular" || variant === "italic" ? 400 : parseInt(variant, 10);
				const italic = variant.endsWith("italic");
				const url = font.files[variant].replace("http://", "https://");

				return { weight, italic, url };
			});
			return { name: font.family, styles };
		});

		editor.handle.onFontCatalogLoad(catalog);
	});

	editor.subscriptions.subscribeJsMessage(TriggerFontDataLoad, async (data) => {
		const { fontFamily, fontStyle } = data.font;

		try {
			if (!data.url) throw new Error("No URL provided for font data load");
			const response = await fetch(data.url);
			const buffer = await response.arrayBuffer();
			const bytes = new Uint8Array(buffer);

			editor.handle.onFontLoad(fontFamily, fontStyle, bytes);
		} catch (error) {
			// eslint-disable-next-line no-console
			console.error("Failed to load font:", error);
		}
	});
}
