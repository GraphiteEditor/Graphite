import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

type ApiResponse = { family: string; variants: string[]; files: Record<string, string> }[];

const FONT_LIST_API = "https://api.graphite.art/font-list";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorWrapper: EditorWrapper | undefined = undefined;
let abortController: AbortController | undefined = undefined;

export function createFontsManager(subscriptions: SubscriptionsRouter, editor: EditorWrapper) {
	destroyFontsManager();

	subscriptionsRouter = subscriptions;
	editorWrapper = editor;
	abortController = new AbortController();

	subscriptions.subscribeFrontendMessage("TriggerFontCatalogLoad", async () => {
		try {
			const response = await fetch(FONT_LIST_API, abortController ? { signal: abortController.signal } : undefined);
			if (!response.ok) throw new Error(`Font catalog request failed with status ${response.status}`);
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

			editor.onFontCatalogLoad(catalog);
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") return;
			throw error;
		}
	});

	subscriptions.subscribeFrontendMessage("TriggerFontDataLoad", async (data) => {
		const { fontFamily, fontStyle } = data.font;

		try {
			if (!data.url) throw new Error("No URL provided for font data load");
			const response = await fetch(data.url, abortController ? { signal: abortController.signal } : undefined);
			if (!response.ok) throw new Error(`Font data request failed with status ${response.status}`);
			const buffer = await response.arrayBuffer();
			const bytes = new Uint8Array(buffer);

			editor.onFontLoad(fontFamily, fontStyle, bytes);
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") return;
			// eslint-disable-next-line no-console
			console.error("Failed to load font:", error);
		}
	});
}

export function destroyFontsManager() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	abortController?.abort();
	subscriptions.unsubscribeFrontendMessage("TriggerFontCatalogLoad");
	subscriptions.unsubscribeFrontendMessage("TriggerFontDataLoad");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorWrapper) newModule?.createFontsManager(subscriptionsRouter, editorWrapper);
});
