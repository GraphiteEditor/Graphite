import { Editor } from "@/wasm-communication/editor";
import { UpdateImageData } from "@/wasm-communication/messages";

export function createBlobManager(editor: Editor): void {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(UpdateImageData, (updateImageData) => {
		updateImageData.image_data.forEach(async (element) => {
			// Using updateImageData.image_data.buffer returns undefined for some reason?
			const blob = new Blob([new Uint8Array(element.image_data.values()).buffer], { type: element.mime });

			const url = URL.createObjectURL(blob);

			const image = await createImageBitmap(blob);

			editor.instance.set_image_blob_url(element.path, url, image.width, image.height);
		});
	});
}
