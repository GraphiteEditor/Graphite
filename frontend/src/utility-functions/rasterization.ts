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

	// Create a blob URL for our SVG
	const svgBlob = new Blob([svg], { type: "image/svg+xml;charset=utf-8" });
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

	const context = canvas.getContext("2d", { willReadFrequently: true });
	if (!context) throw new Error("Could not create canvas context");
	context.drawImage(image, 0, 0, image.width, image.height, 0, 0, width, height);

	return context;
}
