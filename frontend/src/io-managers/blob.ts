import { type Editor } from "@/wasm-communication/editor";
import { UpdateImageData } from "@/wasm-communication/messages";

export function createBlobManager(editor: Editor): void {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(UpdateImageData, (updateImageData) => {
		updateImageData.imageData.forEach(async (element) => {
			// Using updateImageData.imageData.buffer returns undefined for some reason?
			const buffer = new Uint8Array(element.imageData.values()).buffer;
			const blob = new Blob([buffer], { type: element.mime });

			// TODO: Call `URL.revokeObjectURL` at the appropriate time to avoid a memory leak
			const blobURL = URL.createObjectURL(blob);

			const image = await createImageBitmap(blob);

			editor.instance.setImageBlobUrl(element.path, blobURL, image.width, image.height);
		});
	});
}
