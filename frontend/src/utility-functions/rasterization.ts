// Rasterize the string of an SVG document at a given width and height and turn it into the blob data of an image file matching the given MIME type
export function rasterizeSVG(svg: string, width: number, height: number, mime: string, backgroundColor?: string): Promise<Blob> {
	let promiseResolve: (value: Blob | PromiseLike<Blob>) => void | undefined;
	let promiseReject: () => void | undefined;
	const promise = new Promise<Blob>((resolve, reject) => {
		promiseResolve = resolve;
		promiseReject = reject;
	});

	// A canvas to render our svg to in order to get a raster image
	// https://stackoverflow.com/questions/3975499/convert-svg-to-image-jpeg-png-etc-in-the-browser
	const canvas = document.createElement("canvas");
	canvas.width = width;
	canvas.height = height;
	const context = canvas.getContext("2d");
	if (!context) return Promise.reject();

	// Apply a background fill color if one is given
	if (backgroundColor) {
		context.fillStyle = backgroundColor;
		context.fillRect(0, 0, width, height);
	}

	// Create a blob URL for our SVG
	const image = new Image();
	const svgBlob = new Blob([svg], { type: "image/svg+xml;charset=utf-8" });
	const url = URL.createObjectURL(svgBlob);
	image.onload = (): void => {
		// Draw our SVG to the canvas
		context?.drawImage(image, 0, 0, width, height);

		// Clean up the SVG blob URL (once the URL is revoked, the SVG blob data itself is garbage collected after `svgBlob` goes out of scope)
		URL.revokeObjectURL(url);

		// Convert the canvas to an image of the correct MIME type
		canvas.toBlob((blob) => {
			if (blob !== null) promiseResolve(blob);
			else promiseReject();
		}, mime);
	};
	image.src = url;

	return promise;
}
