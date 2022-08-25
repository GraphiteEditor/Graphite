import { type Editor } from "@/wasm-communication/editor";
import { UpdateImageData } from "@/wasm-communication/messages";

export function createBlobManager(editor: Editor): void {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(UpdateImageData, (updateImageData) => {
		updateImageData.imageData.forEach(async (element) => {
			// Using updateImageData.imageData.buffer returns undefined for some reason?
			const blob = new Blob([new Uint8Array(element.imageData.values()).buffer], { type: element.mime });

			const url = URL.createObjectURL(blob);

			const image = await createImageBitmap(blob);

			editor.instance.setImageBlobUrl(element.path, url, image.width, image.height);
		});
	});
}
