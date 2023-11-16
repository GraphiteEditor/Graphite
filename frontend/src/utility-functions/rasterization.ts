import { replaceBlobURLsWithBase64 } from "@graphite/utility-functions/files";

// Rasterize the string of an SVG document at a given width and height and return the canvas it was drawn onto during the rasterization process
export async function rasterizeSVGCanvas(svg: string, width: number, height: number, backgroundColor?: string): Promise<HTMLCanvasElement> {
	// A canvas to render our SVG to in order to get a raster image
	const canvas = document.createElement("canvas");
	canvas.width = width;
	canvas.height = height;
	const context = canvas.getContext("2d", { willReadFrequently: true });
	if (!context) throw new Error("Can't create 2D context from canvas during SVG rasterization");

	// Apply a background fill color if one is given
	if (backgroundColor) {
		context.fillStyle = backgroundColor;
		context.fillRect(0, 0, width, height);
	}

	// This SVG rasterization scheme has the limitation that it cannot access blob URLs, so they must be inlined to base64 URLs
	const svgWithBase64Images = await replaceBlobURLsWithBase64(svg);

	// Create a blob URL for our SVG
	const svgBlob = new Blob([svgWithBase64Images], { type: "image/svg+xml;charset=utf-8" });
	const url = URL.createObjectURL(svgBlob);

	// Load the Image from the URL and wait until it's done
	const image = new Image();
	image.src = url;
	await new Promise<void>((resolve) => {
		image.onload = () => resolve();
	});

	// Draw our SVG to the canvas
	context?.drawImage(image, 0, 0, width, height);

	// Clean up the SVG blob URL (once the URL is revoked, the SVG blob data itself is garbage collected after `svgBlob` goes out of scope)
	URL.revokeObjectURL(url);

	return canvas;
}

// Rasterize the string of an SVG document at a given width and height and turn it into the blob data of an image file matching the given MIME type
export async function rasterizeSVG(svg: string, width: number, height: number, mime: string, backgroundColor?: string): Promise<Blob> {
	if (!width || !height) throw new Error("Width and height must be nonzero when given to rasterizeSVG()");

	const canvas = await rasterizeSVGCanvas(svg, width, height, backgroundColor);

	// Convert the canvas to an image of the correct MIME type
	const blob = await new Promise<Blob | undefined>((resolve) => {
		canvas.toBlob((blob) => {
			resolve(blob || undefined);
		}, mime);
	});

	if (!blob) throw new Error("Converting canvas to blob data failed in rasterizeSVG()");

	return blob;
}

/// Convert an image source (e.g. PNG document) into pixel data, a width, and a height
export async function extractPixelData(imageData: ImageBitmapSource): Promise<ImageData> {
	const canvasContext = await imageToCanvasContext(imageData);
	const width = canvasContext.canvas.width;
	const height = canvasContext.canvas.height;

	return canvasContext.getImageData(0, 0, width, height);
}

/// Convert an image source (e.g. BMP document) into a PNG blob
export async function imageToPNG(imageData: ImageBitmapSource): Promise<Blob> {
	const canvasContext = await imageToCanvasContext(imageData);

	return new Promise((resolve, reject) => {
		canvasContext.canvas.toBlob((pngBlob) => {
			if (pngBlob) resolve(pngBlob);
			else reject("Converting canvas to blob data failed in imageToPNG()");
		}, "image/png");
	});
}

export async function imageToCanvasContext(imageData: ImageBitmapSource): Promise<CanvasRenderingContext2D> {
	// Special handling to rasterize an SVG file
	let svgImageData;
	if (imageData instanceof File && imageData.type === "image/svg+xml") {
		const svgSource = await imageData.text();
		const svgElement = new DOMParser().parseFromString(svgSource, "image/svg+xml").querySelector("svg");
		if (!svgElement) throw new Error("Error reading SVG file");

		let bounds = svgElement.viewBox.baseVal;

		// If the bounds are zero (which will happen if the `viewBox` is not provided), set bounds to the artwork's bounding box
		if (bounds.width === 0 || bounds.height === 0) {
			// It's necessary to measure while the element is in the DOM, otherwise the dimensions are zero
			const toRemove = document.body.insertAdjacentElement("beforeend", svgElement);
			bounds = svgElement.getBBox();
			toRemove?.remove();
		}

		svgImageData = await rasterizeSVGCanvas(svgSource, bounds.width, bounds.height);
	}

	// Decode the image file binary data
	const image = await createImageBitmap(svgImageData || imageData);

	let { width, height } = image;
	width = Math.floor(width);
	height = Math.floor(height);

	// Render image to canvas
	const canvas = document.createElement("canvas");
	canvas.width = width;
	canvas.height = height;

	const context = canvas.getContext("2d");
	if (!context) throw new Error("Could not create canvas context");
	context.drawImage(image, 0, 0, image.width, image.height, 0, 0, width, height);

	return context;
}
