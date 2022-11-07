import { replaceBlobURLsWithBase64 } from "@/utility-functions/files";

// Rasterize the string of an SVG document at a given width and height and turn it into the blob data of an image file matching the given MIME type
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

	const image = new Image();
	image.src = url;
	await new Promise<void>((resolve) => {
		image.onload = (): void => resolve();
	});

	// Draw our SVG to the canvas
	context?.drawImage(image, 0, 0, width, height);

	// Clean up the SVG blob URL (once the URL is revoked, the SVG blob data itself is garbage collected after `svgBlob` goes out of scope)
	URL.revokeObjectURL(url);

	return canvas;
}

export async function rasterizeSVG(svg: string, width: number, height: number, mime: string, backgroundColor?: string): Promise<Blob> {
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
