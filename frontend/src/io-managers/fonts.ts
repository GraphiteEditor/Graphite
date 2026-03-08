import type { Editor } from "@graphite/editor";

type ApiResponse = { family: string; variants: string[]; files: Record<string, string> }[];

const FONT_LIST_API = "https://api.graphite.art/font-list";

export function createFontsManager(editor: Editor): () => void {
	const abortController = new AbortController();

	// Subscribe to process backend events
	editor.subscriptions.subscribeFrontendMessage("TriggerFontCatalogLoad", async () => {
		try {
			const response = await fetch(FONT_LIST_API, { signal: abortController.signal });
			const fontListResponse: { items: ApiResponse } = await response.json();
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
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") return;
			throw error;
		}
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerFontDataLoad", async (data) => {
		const { fontFamily, fontStyle } = data.font;

		try {
			if (!data.url) throw new Error("No URL provided for font data load");
			const response = await fetch(data.url, { signal: abortController.signal });
			const buffer = await response.arrayBuffer();
			const bytes = new Uint8Array(buffer);

			editor.handle.onFontLoad(fontFamily, fontStyle, bytes);
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") return;
			// eslint-disable-next-line no-console
			console.error("Failed to load font:", error);
		}
	});

	return () => {
		abortController.abort();
		editor.subscriptions.unsubscribeFrontendMessage("TriggerFontCatalogLoad");
		editor.subscriptions.unsubscribeFrontendMessage("TriggerFontDataLoad");
	};
}
