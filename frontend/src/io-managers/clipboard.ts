import { type Editor } from "@graphite/wasm-communication/editor";
import { TriggerTextCopy } from "@graphite/wasm-communication/messages";
import { imageToPNG } from "~src/utility-functions/rasterization";

export function createClipboardManager(editor: Editor): void {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerTextCopy, (triggerTextCopy) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(triggerTextCopy.copyText);
	});
}

export async function copyToClipboardFileURL(url: string): Promise<void> {
	const response = await fetch(url);
	const blob = await response.blob();

	// TODO: Remove this if/when we end up returning PNG directly from the backend
	const pngBlob = await imageToPNG(blob);

	const clipboardItem: Record<string, Blob> = {};
	clipboardItem[pngBlob.type] = pngBlob;
	const data = [new ClipboardItem(clipboardItem)];

	navigator.clipboard.write(data);
}
