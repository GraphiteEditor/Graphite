/// Convert an image source (e.g. PNG document) into pixel data, a width and a height
export async function extractPixelData(imageData: ImageBitmapSource): Promise<ImageData> {
	// Get image size
	const image = await createImageBitmap(imageData);
	let { width, height } = image;

	// Halve the image size until the editor lag is somewhat usable.
	// TODO: Fix lag.
	const maxImageSize = 512;
	while (width > maxImageSize || height > maxImageSize) {
		width /= 2;
		height /= 2;
	}

	// Render image to canvas
	const canvas = document.createElement("canvas");
	canvas.width = width;
	canvas.height = height;
	const ctx = canvas.getContext("2d");
	if (!ctx) throw new Error("Could not create canvas context");
	ctx.drawImage(image, 0, 0, image.width, image.height, 0, 0, width, height);
	return ctx.getImageData(0, 0, width, height);
}
