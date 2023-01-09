<script lang="ts">
	import { getContext, onMount, tick } from "svelte";

	import { textInputCleanup } from "@/utility-functions/keyboard-entry";
	import { rasterizeSVGCanvas } from "@/utility-functions/rasterization";
	import { type DisplayEditableTextbox, type MouseCursorIcon, type XY } from "@/wasm-communication/messages";

	import EyedropperPreview, { ZOOM_WINDOW_DIMENSIONS } from "@/components/floating-menus/EyedropperPreview.svelte";
	import LayoutCol from "@/components/layout/LayoutCol.svelte";
	import LayoutRow from "@/components/layout/LayoutRow.svelte";
	import CanvasRuler from "@/components/widgets/metrics/CanvasRuler.svelte";
	import PersistentScrollbar from "@/components/widgets/metrics/PersistentScrollbar.svelte";
	import WidgetLayout from "@/components/widgets/WidgetLayout.svelte";
	import { type Editor } from "@/wasm-communication/editor";
	import { type PanelsState } from "@/state-providers/panels";
	import { type DocumentState } from "@/state-providers/document";

	let rulerHorizontal: CanvasRuler;
	let rulerVertical: CanvasRuler;
	let canvasDiv: HTMLDivElement;

	const editor = getContext<Editor>("editor");
	const panels = getContext<PanelsState>("panels");
	const document = getContext<DocumentState>("document");

	// Interactive text editing
	let textInput: undefined | HTMLDivElement = undefined;

	// CSS properties
	let canvasSvgWidth: number | undefined = undefined;
	let canvasSvgHeight: number | undefined = undefined;
	let canvasCursor = "default";

	// Scrollbars
	let scrollbarPos: XY = { x: 0.5, y: 0.5 };
	let scrollbarSize: XY = { x: 0.5, y: 0.5 };
	let scrollbarMultiplier: XY = { x: 0, y: 0 };

	// Rulers
	let rulerOrigin: XY = { x: 0, y: 0 };
	let rulerSpacing: number = 100;
	let rulerInterval: number = 100;

	// Rendered SVG viewport data
	let artworkSvg: string = "";
	let artboardSvg: string = "";
	let overlaysSvg: string = "";

	// Rasterized SVG viewport data, or none if it's not up-to-date
	let rasterizedCanvas: HTMLCanvasElement | undefined = undefined;
	let rasterizedContext: CanvasRenderingContext2D | undefined = undefined;

	// Cursor position for cursor floating menus like the Eyedropper tool zoom
	let cursorLeft = 0;
	let cursorTop = 0;
	let cursorEyedropper = false;
	let cursorEyedropperPreviewImageData: ImageData | undefined = undefined;
	let cursorEyedropperPreviewColorChoice = "";
	let cursorEyedropperPreviewColorPrimary = "";
	let cursorEyedropperPreviewColorSecondary = "";

	$: canvasWidthCSS = canvasDimensionCSS(canvasSvgWidth);
	$: canvasHeightCSS = canvasDimensionCSS(canvasSvgHeight);

	onMount(() => {
		panels.registerPanel("Document", this);

		// Once this component is mounted, we want to resend the document bounds to the backend via the resize event handler which does that
		window.dispatchEvent(new Event("resize"));
	});

	function pasteFile(e: DragEvent) {
		const { dataTransfer } = e;
		if (!dataTransfer) return;
		e.preventDefault();

		Array.from(dataTransfer.items).forEach(async (item) => {
			const file = item.getAsFile();
			if (file?.type.startsWith("image")) {
				const buffer = await file.arrayBuffer();
				const u8Array = new Uint8Array(buffer);

				editor.instance.pasteImage(file.type, u8Array, e.clientX, e.clientY);
			}
		});
	}

	function translateCanvasX(newValue: number) {
		const delta = newValue - scrollbarPos.x;
		scrollbarPos.x = newValue;
		editor.instance.translateCanvas(-delta * scrollbarMultiplier.x, 0);
	}

	function translateCanvasY(newValue: number) {
		const delta = newValue - scrollbarPos.y;
		scrollbarPos.y = newValue;
		editor.instance.translateCanvas(0, -delta * scrollbarMultiplier.y);
	}

	function pageX(delta: number) {
		const move = delta < 0 ? 1 : -1;
		editor.instance.translateCanvasByFraction(move, 0);
	}

	function pageY(delta: number) {
		const move = delta < 0 ? 1 : -1;
		editor.instance.translateCanvasByFraction(0, move);
	}

	function canvasPointerDown(e: PointerEvent) {
		const onEditbox = e.target instanceof HTMLDivElement && e.target.contentEditable;

		if (!onEditbox) canvasDiv?.setPointerCapture(e.pointerId);
	}

	// Update rendered SVGs
	export async function updateDocumentArtwork(svg: string) {
		artworkSvg = svg;
		rasterizedCanvas = undefined;

		await tick();

		if (textInput) {
			const foreignObject = canvasDiv.getElementsByTagName("foreignObject")[0] as SVGForeignObjectElement;
			if (foreignObject.children.length > 0) return;

			const addedInput = foreignObject.appendChild(textInput);
			window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: addedInput }));

			await tick();

			// Necessary to select contenteditable: https://stackoverflow.com/questions/6139107/programmatically-select-text-in-a-contenteditable-html-element/6150060#6150060

			const range = document.createRange();
			range.selectNodeContents(addedInput);

			const selection = window.getSelection();
			if (selection) {
				selection.removeAllRanges();
				selection.addRange(range);
			}

			addedInput.focus();
			addedInput.click();
		}
	}

	export function updateDocumentOverlays(svg: string) {
		overlaysSvg = svg;
	}

	export function updateDocumentArtboards(svg: string) {
		artboardSvg = svg;
		rasterizedCanvas = undefined;
	}

	export async function updateEyedropperSamplingState(mousePosition: XY | undefined, colorPrimary: string, colorSecondary: string): Promise<[number, number, number] | undefined> {
		if (mousePosition === undefined) {
			cursorEyedropper = false;
			return undefined;
		}
		cursorEyedropper = true;

		if (canvasSvgWidth === undefined || canvasSvgHeight === undefined) return undefined;

		cursorLeft = mousePosition.x;
		cursorTop = mousePosition.y;

		// This works nearly perfectly, but sometimes at odd DPI scale factors like 1.25, the anti-aliasing color can yield slightly incorrect colors (potential room for future improvement)
		const dpiFactor = window.devicePixelRatio;
		const [width, height] = [canvasSvgWidth, canvasSvgHeight];

		const outsideArtboardsColor = getComputedStyle(document.documentElement).getPropertyValue("--color-2-mildblack");
		const outsideArtboards = `<rect x="0" y="0" width="100%" height="100%" fill="${outsideArtboardsColor}" />`;
		const artboards = artboardSvg;
		const artwork = artworkSvg;
		const svg = `
				<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">${outsideArtboards}${artboards}${artwork}</svg>
				`.trim();

		if (!rasterizedCanvas) {
			rasterizedCanvas = await rasterizeSVGCanvas(svg, width * dpiFactor, height * dpiFactor, "image/png");
			rasterizedContext = rasterizedCanvas.getContext("2d") || undefined;
		}
		if (!rasterizedContext) return undefined;

		const rgbToHex = (r: number, g: number, b: number): string => `#${[r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("")}`;

		const pixel = rasterizedContext.getImageData(mousePosition.x * dpiFactor, mousePosition.y * dpiFactor, 1, 1).data;
		const hex = rgbToHex(pixel[0], pixel[1], pixel[2]);
		const rgb: [number, number, number] = [pixel[0] / 255, pixel[1] / 255, pixel[2] / 255];

		cursorEyedropperPreviewColorChoice = hex;
		cursorEyedropperPreviewColorPrimary = colorPrimary;
		cursorEyedropperPreviewColorSecondary = colorSecondary;

		const previewRegion = rasterizedContext.getImageData(
			mousePosition.x * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
			mousePosition.y * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
			ZOOM_WINDOW_DIMENSIONS,
			ZOOM_WINDOW_DIMENSIONS
		);
		cursorEyedropperPreviewImageData = previewRegion;

		return rgb;
	}

	// Update scrollbars and rulers
	export function updateDocumentScrollbars(position: XY, size: XY, multiplier: XY) {
		scrollbarPos = position;
		scrollbarSize = size;
		scrollbarMultiplier = multiplier;
	}

	export function updateDocumentRulers(origin: XY, spacing: number, interval: number) {
		rulerOrigin = origin;
		rulerSpacing = spacing;
		rulerInterval = interval;
	}

	// Update mouse cursor icon
	export function updateMouseCursor(cursor: MouseCursorIcon) {
		let cursorString: string = cursor;

		// This isn't very clean but it's good enough for now until we need more icons, then we can build something more robust (consider blob URLs)
		if (cursor === "custom-rotate") {
			const svg = `
					<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" width="20" height="20">
						<path transform="translate(2 2)" fill="black" stroke="black" stroke-width="2px" d="
						M8,15.2C4,15.2,0.8,12,0.8,8C0.8,4,4,0.8,8,0.8c2,0,3.9,0.8,5.3,2.3l-1,1C11.2,2.9,9.6,2.2,8,2.2C4.8,2.2,2.2,4.8,2.2,8s2.6,5.8,5.8,5.8s5.8-2.6,5.8-5.8h1.4C15.2,12,12,15.2,8,15.2z
						" />
						<polygon transform="translate(2 2)" fill="black" stroke="black" stroke-width="2px" points="12.6,0 15.5,5 9.7,5" />
						<path transform="translate(2 2)" fill="white" d="
						M8,15.2C4,15.2,0.8,12,0.8,8C0.8,4,4,0.8,8,0.8c2,0,3.9,0.8,5.3,2.3l-1,1C11.2,2.9,9.6,2.2,8,2.2C4.8,2.2,2.2,4.8,2.2,8s2.6,5.8,5.8,5.8s5.8-2.6,5.8-5.8h1.4C15.2,12,12,15.2,8,15.2z
						" />
						<polygon transform="translate(2 2)" fill="white" points="12.6,0 15.5,5 9.7,5" />
					</svg>
					`
				.split("\n")
				.map((line) => line.trim())
				.join("");

			cursorString = `url('data:image/svg+xml;utf8,${svg}') 8 8, alias`;
		}

		canvasCursor = cursorString;
	}

	// Text entry
	export function triggerTextCommit() {
		if (!textInput) return;
		const textCleaned = textInputCleanup(textInput.innerText);
		editor.instance.onChangeText(textCleaned);
	}

	export function displayEditableTextbox(displayEditableTextbox: DisplayEditableTextbox) {
		textInput = document.createElement("div") as HTMLDivElement;

		if (displayEditableTextbox.text === "") textInput.textContent = "";
		else textInput.textContent = `${displayEditableTextbox.text}\n`;

		textInput.contentEditable = "true";
		textInput.style.width = displayEditableTextbox.lineWidth ? `${displayEditableTextbox.lineWidth}px` : "max-content";
		textInput.style.height = "auto";
		textInput.style.fontSize = `${displayEditableTextbox.fontSize}px`;
		textInput.style.color = displayEditableTextbox.color.toHexOptionalAlpha() || "transparent";

		textInput.oninput = (): void => {
			if (!textInput) return;
			editor.instance.updateBounds(textInputCleanup(textInput.innerText));
		};
	}

	export function displayRemoveEditableTextbox() {
		textInput = undefined;
		window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: undefined }));
	}

	// Resize elements to render the new viewport size
	export function viewportResize() {
		// Resize the canvas
		canvasSvgWidth = Math.ceil(parseFloat(getComputedStyle(canvasDiv).width));
		canvasSvgHeight = Math.ceil(parseFloat(getComputedStyle(canvasDiv).height));

		// Resize the rulers
		rulerHorizontal?.resize();
		rulerVertical?.resize();
	}

	function canvasDimensionCSS(dimension: number | undefined): string {
		// Temporary placeholder until the first actual value is populated
		// This at least gets close to the correct value but an actual number is required to prevent CSS from causing non-integer sizing making the SVG render with anti-aliasing
		if (dimension === undefined) return "100%";

		// Dimension is rounded up to the nearest even number because resizing is centered, and dividing an odd number by 2 for centering causes antialiasing
		return `${dimension % 2 === 1 ? dimension + 1 : dimension}px`;
	}
</script>

<LayoutCol class="document">
	<LayoutRow class="options-bar" scrollableX={true}>
		<WidgetLayout layout={$document.documentModeLayout} />
		<WidgetLayout layout={$document.toolOptionsLayout} />

		<LayoutRow class="spacer" />

		<WidgetLayout layout={$document.documentBarLayout} />
	</LayoutRow>
	<LayoutRow class="shelf-and-viewport">
		<LayoutCol class="shelf">
			<LayoutCol class="tools" scrollableY={true}>
				<WidgetLayout layout={$document.toolShelfLayout} />
			</LayoutCol>

			<LayoutCol class="spacer" />

			<LayoutCol class="working-colors">
				<WidgetLayout layout={$document.workingColorsLayout} />
			</LayoutCol>
		</LayoutCol>
		<LayoutCol class="viewport">
			<LayoutRow class="bar-area">
				<CanvasRuler origin={rulerOrigin.x} majorMarkSpacing={rulerSpacing} numberInterval={rulerInterval} direction="Horizontal" class="top-ruler" bind:this={rulerHorizontal} />
			</LayoutRow>
			<LayoutRow class="canvas-area">
				<LayoutCol class="bar-area">
					<CanvasRuler origin={rulerOrigin.y} majorMarkSpacing={rulerSpacing} numberInterval={rulerInterval} direction="Vertical" bind:this={rulerVertical} />
				</LayoutCol>
				<LayoutCol class="canvas-area" styles={{ cursor: canvasCursor }}>
					{#if cursorEyedropper}
						<EyedropperPreview
							colorChoice={cursorEyedropperPreviewColorChoice}
							primaryColor={cursorEyedropperPreviewColorPrimary}
							secondaryColor={cursorEyedropperPreviewColorSecondary}
							imageData={cursorEyedropperPreviewImageData}
							x={cursorLeft}
							y={cursorTop}
						/>
					{/if}
					<div class="canvas" on:pointerdown={(e) => canvasPointerDown(e)} on:dragover={(e) => e.preventDefault()} on:drop={(e) => pasteFile(e)} bind:this={canvasDiv} data-canvas>
						<svg class="artboards" style:width={canvasWidthCSS} style:height={canvasHeightCSS}>
							{@html artboardSvg}
						</svg>
						<svg class="artwork" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style:width={canvasWidthCSS} style:height={canvasHeightCSS}>
							{@html artworkSvg}
						</svg>
						<svg class="overlays" style:width={canvasWidthCSS} style:height={canvasHeightCSS}>
							{@html overlaysSvg}
						</svg>
					</div>
				</LayoutCol>
				<LayoutCol class="bar-area right-scrollbar">
					<PersistentScrollbar
						direction="Vertical"
						handlePosition={scrollbarPos.y}
						handleLength={scrollbarSize.y}
						on:handlePosition={(newValue) => translateCanvasY(newValue)}
						on:pressTrack={(delta) => pageY(delta)}
					/>
				</LayoutCol>
			</LayoutRow>
			<LayoutRow class="bar-area bottom-scrollbar">
				<PersistentScrollbar
					direction="Horizontal"
					handlePosition={scrollbarPos.x}
					handleLength={scrollbarSize.x}
					on:handlePosition={(newValue) => translateCanvasX(newValue)}
					on:pressTrack={(delta) => pageX(delta)}
				/>
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.document {
		height: 100%;

		.options-bar {
			height: 32px;
			flex: 0 0 auto;
			margin: 0 4px;

			.spacer {
				min-width: 40px;
			}
		}

		.shelf-and-viewport {
			.shelf {
				flex: 0 0 auto;

				.tools {
					flex: 0 1 auto;

					.icon-button[title^="Coming Soon"] {
						opacity: 0.25;
						transition: opacity 0.25s;

						&:hover {
							opacity: 1;
						}
					}

					.icon-button:not(.active) {
						.color-solid {
							fill: var(--color-f-white);
						}

						.color-general {
							fill: var(--color-data-general);
						}

						.color-vector {
							fill: var(--color-data-vector);
						}

						.color-raster {
							fill: var(--color-data-raster);
						}
					}
				}

				.spacer {
					flex: 1 0 auto;
					min-height: 8px;
				}

				.working-colors {
					flex: 0 0 auto;

					.widget-row {
						min-height: 0;

						.swatch-pair {
							margin: 0;
						}

						.icon-button {
							--widget-height: 0;
						}
					}
				}
			}

			.viewport {
				flex: 1 1 100%;

				.canvas-area {
					flex: 1 1 100%;
					position: relative;
				}

				.bar-area {
					flex: 0 0 auto;
				}

				.top-ruler {
					padding-left: 16px;
					margin-right: 16px;
				}

				.right-scrollbar .persistent-scrollbar {
					margin-top: -16px;
				}

				.bottom-scrollbar .persistent-scrollbar {
					margin-right: 16px;
				}

				.canvas {
					background: var(--color-2-mildblack);
					width: 100%;
					height: 100%;
					// Allows the SVG to be placed at explicit integer values of width and height to prevent non-pixel-perfect SVG scaling
					position: relative;
					overflow: hidden;

					svg {
						position: absolute;
						// Fallback values if JS hasn't set these to integers yet
						width: 100%;
						height: 100%;
						// Allows dev tools to select the artwork without being blocked by the SVG containers
						pointer-events: none;

						// Prevent inheritance from reaching the child elements
						> * {
							pointer-events: auto;
						}
					}

					foreignObject {
						width: 10000px;
						height: 10000px;
						overflow: visible;

						div {
							cursor: text;
							background: none;
							border: none;
							margin: 0;
							padding: 0;
							overflow: visible;
							white-space: pre-wrap;
							display: inline-block;
							// Workaround to force Chrome to display the flashing text entry cursor when text is empty
							padding-left: 1px;
							margin-left: -1px;

							&:focus {
								border: none;
								outline: none; // Ok for contenteditable element
								margin: -1px;
							}
						}
					}
				}
			}
		}
	}
</style>
