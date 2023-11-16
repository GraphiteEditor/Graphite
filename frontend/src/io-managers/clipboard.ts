import { imageToPNG } from "@graphite/utility-functions/rasterization";
import { type Editor } from "@graphite/wasm-communication/editor";
import { TriggerTextCopy } from "@graphite/wasm-communication/messages";

export function createClipboardManager(editor: Editor) {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerTextCopy, (triggerTextCopy) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(triggerTextCopy.copyText);
	});
}

export async function copyToClipboardFileURL(url: string) {
	const response = await fetch(url);
	const blob = await response.blob();

	// TODO: Remove this if/when we end up returning PNG directly from the backend
	const pngBlob = await imageToPNG(blob);

	const clipboardItem: Record<string, Blob> = {};
	clipboardItem[pngBlob.type] = pngBlob;
	const data = [new ClipboardItem(clipboardItem)];

	// Note: if this image has transparency, it will be lost and appear as black due to limitations of the way browsers handle copying transparent images
	// This even happens if you just open a regular transparent PNG file in a browser tab, right click > copy, and paste it somewhere (the transparency will show up as black)
	// This is true, at least, on Windows (it's worth checking on other OSs though)
	navigator.clipboard.write(data);
}
