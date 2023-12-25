<script lang="ts">
	import { getContext, onMount, tick } from "svelte";

	import type { DocumentState } from "@graphite/state-providers/document";
	import { textInputCleanup } from "@graphite/utility-functions/keyboard-entry";
	import { extractPixelData, rasterizeSVGCanvas } from "@graphite/utility-functions/rasterization";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import {
		type MouseCursorIcon,
		type XY,
		DisplayEditableTextbox,
		DisplayEditableTextboxTransform,
		DisplayRemoveEditableTextbox,
		TriggerTextCommit,
		TriggerViewportResize,
		UpdateDocumentArtwork,
		UpdateDocumentRulers,
		UpdateDocumentScrollbars,
		UpdateEyedropperSamplingState,
		UpdateMouseCursor,
		isWidgetSpanRow,
	} from "@graphite/wasm-communication/messages";

	import EyedropperPreview, { ZOOM_WINDOW_DIMENSIONS } from "@graphite/components/floating-menus/EyedropperPreview.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import Graph from "@graphite/components/views/Graph.svelte";
	import RulerInput from "@graphite/components/widgets/inputs/RulerInput.svelte";
	import ScrollbarInput from "@graphite/components/widgets/inputs/ScrollbarInput.svelte";
	import WidgetLayout from "@graphite/components/widgets/WidgetLayout.svelte";

	let rulerHorizontal: RulerInput | undefined;
	let rulerVertical: RulerInput | undefined;
	let viewport: HTMLDivElement | undefined;

	const editor = getContext<Editor>("editor");
	const document = getContext<DocumentState>("document");

	// Interactive text editing
	let textInput: undefined | HTMLDivElement = undefined;
	let showTextInput: boolean;
	let textInputMatrix: number[];

	// Scrollbars
	let scrollbarPos: XY = { x: 0.5, y: 0.5 };
	let scrollbarSize: XY = { x: 0.5, y: 0.5 };
	let scrollbarMultiplier: XY = { x: 0, y: 0 };

	// Rulers
	let rulerOrigin: XY = { x: 0, y: 0 };
	let rulerSpacing = 100;
	let rulerInterval = 100;
	let rulersVisible = true;

	// Rendered SVG viewport data
	let artworkSvg = "";

	// Rasterized SVG viewport data, or none if it's not up-to-date
	let rasterizedCanvas: HTMLCanvasElement | undefined = undefined;
	let rasterizedContext: CanvasRenderingContext2D | undefined = undefined;

	// Cursor icon to display while hovering over the canvas
	let canvasCursor = "default";

	// Cursor position for cursor floating menus like the Eyedropper tool zoom
	let cursorLeft = 0;
	let cursorTop = 0;
	let cursorEyedropper = false;
	let cursorEyedropperPreviewImageData: ImageData | undefined = undefined;
	let cursorEyedropperPreviewColorChoice = "";
	let cursorEyedropperPreviewColorPrimary = "";
	let cursorEyedropperPreviewColorSecondary = "";

	// Canvas dimensions
	let canvasSvgWidth: number | undefined = undefined;
	let canvasSvgHeight: number | undefined = undefined;

	// Used to set the canvas rendering dimensions.
	// Dimension is rounded up to the nearest even number because resizing is centered, and dividing an odd number by 2 for centering causes antialiasing
	$: canvasWidthRoundedToEven = canvasSvgWidth && (canvasSvgWidth % 2 === 1 ? canvasSvgWidth + 1 : canvasSvgWidth);
	$: canvasHeightRoundedToEven = canvasSvgHeight && (canvasSvgHeight % 2 === 1 ? canvasSvgHeight + 1 : canvasSvgHeight);
	// Used to set the canvas element size on the page.
	// The value above in pixels, or if undefined, we fall back to 100% as a non-pixel-perfect backup that's hopefully short-lived
	$: canvasWidthCSS = canvasWidthRoundedToEven ? `${canvasWidthRoundedToEven}px` : "100%";
	$: canvasHeightCSS = canvasHeightRoundedToEven ? `${canvasHeightRoundedToEven}px` : "100%";

	$: toolShelfTotalToolsAndSeparators = ((layoutGroup) => {
		if (!isWidgetSpanRow(layoutGroup)) return undefined;

		let totalSeparators = 0;
		let totalToolRowsFor1Columns = 0;
		let totalToolRowsFor2Columns = 0;
		let totalToolRowsFor3Columns = 0;

		const tally = () => {
			totalToolRowsFor1Columns += toolsInCurrentGroup;
			totalToolRowsFor2Columns += Math.ceil(toolsInCurrentGroup / 2);
			totalToolRowsFor3Columns += Math.ceil(toolsInCurrentGroup / 3);
			toolsInCurrentGroup = 0;
		};

		let toolsInCurrentGroup = 0;
		layoutGroup.rowWidgets.forEach((widget) => {
			if (widget.props.kind === "Separator") {
				totalSeparators += 1;
				tally();
			} else {
				toolsInCurrentGroup += 1;
			}
		});
		tally();

		return {
			totalSeparators,
			totalToolRowsFor1Columns,
			totalToolRowsFor2Columns,
			totalToolRowsFor3Columns,
		};
	})($document.toolShelfLayout.layout[0]);

	function pasteFile(e: DragEvent) {
		const { dataTransfer } = e;
		if (!dataTransfer) return;
		e.preventDefault();

		Array.from(dataTransfer.items).forEach(async (item) => {
			const file = item.getAsFile();
			if (file?.type.includes("svg")) {
				const svgData = await file.text();
				editor.instance.pasteSvg(svgData, e.clientX, e.clientY);

				return;
			}

			if (file?.type.startsWith("image")) {
				const imageData = await extractPixelData(file);
				editor.instance.pasteImage(new Uint8Array(imageData.data), imageData.width, imageData.height, e.clientX, e.clientY);
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

		if (!onEditbox) viewport?.setPointerCapture(e.pointerId);
	}

	// Update rendered SVGs
	export async function updateDocumentArtwork(svg: string) {
		// TODO: Sort this out so we're either sending only the SVG inner contents from the backend or not setting the width/height attributes here
		// TODO: (but preserving the rounding-up-to-the-next-even-number to prevent antialiasing).
		artworkSvg = svg
			.trim()
			.replace(/<svg[^>]*>/, "")
			.slice(0, -"</svg>".length);
		rasterizedCanvas = undefined;

		await tick();

		const placeholders = window.document.querySelectorAll("[data-viewport] [data-canvas-placeholder]");
		// Replace the placeholders with the actual canvas elements
		placeholders.forEach((placeholder) => {
			const canvasName = placeholder.getAttribute("data-canvas-placeholder");
			if (!canvasName) return;
			// Get the canvas element from the global storage
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			const canvas = (window as any).imageCanvases[canvasName];
			placeholder.replaceWith(canvas);
		});
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

		const outsideArtboardsColor = getComputedStyle(window.document.documentElement).getPropertyValue("--color-2-mildblack");
		const outsideArtboards = `<rect x="0" y="0" width="100%" height="100%" fill="${outsideArtboardsColor}" />`;

		const svg = `
			<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">${outsideArtboards}${artworkSvg}</svg>
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
			ZOOM_WINDOW_DIMENSIONS,
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

	export function updateDocumentRulers(origin: XY, spacing: number, interval: number, visible: boolean) {
		rulerOrigin = origin;
		rulerSpacing = spacing;
		rulerInterval = interval;
		rulersVisible = visible;
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

	export async function displayEditableTextbox(displayEditableTextbox: DisplayEditableTextbox) {
		showTextInput = true;

		await tick();

		if (!textInput) {
			return;
		}

		if (displayEditableTextbox.text === "") textInput.textContent = "";
		else textInput.textContent = `${displayEditableTextbox.text}\n`;

		textInput.contentEditable = "true";
		textInput.style.transformOrigin = "0 0";
		textInput.style.width = displayEditableTextbox.lineWidth ? `${displayEditableTextbox.lineWidth}px` : "max-content";
		textInput.style.height = "auto";
		textInput.style.fontSize = `${displayEditableTextbox.fontSize}px`;
		textInput.style.color = displayEditableTextbox.color.toHexOptionalAlpha() || "transparent";

		textInput.oninput = () => {
			if (!textInput) return;
			editor.instance.updateBounds(textInputCleanup(textInput.innerText));
		};
		textInputMatrix = displayEditableTextbox.transform;
		const newFont = new FontFace("text-font", `url(${displayEditableTextbox.url})`);
		window.document.fonts.add(newFont);
		textInput.style.fontFamily = "text-font";

		// Necessary to select contenteditable: https://stackoverflow.com/questions/6139107/programmatically-select-text-in-a-contenteditable-html-element/6150060#6150060

		const range = window.document.createRange();
		range.selectNodeContents(textInput);

		const selection = window.getSelection();
		if (selection) {
			selection.removeAllRanges();
			selection.addRange(range);
		}

		textInput.focus();
		textInput.click();

		// Sends the text input element used for interactively editing with the text tool in a custom event
		window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: textInput }));
	}

	export function displayRemoveEditableTextbox() {
		window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: undefined }));
		showTextInput = false;
	}

	// Resize elements to render the new viewport size
	export function viewportResize() {
		if (!viewport) return;

		// Resize the canvas
		canvasSvgWidth = Math.ceil(parseFloat(getComputedStyle(viewport).width));
		canvasSvgHeight = Math.ceil(parseFloat(getComputedStyle(viewport).height));

		// Resize the rulers
		rulerHorizontal?.resize();
		rulerVertical?.resize();
	}

	onMount(() => {
		// Update rendered SVGs
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, async (data) => {
			await tick();

			updateDocumentArtwork(data.svg);
		});
		editor.subscriptions.subscribeJsMessage(UpdateEyedropperSamplingState, async (data) => {
			await tick();

			const { mousePosition, primaryColor, secondaryColor, setColorChoice } = data;
			const rgb = await updateEyedropperSamplingState(mousePosition, primaryColor, secondaryColor);

			if (setColorChoice && rgb) {
				if (setColorChoice === "Primary") editor.instance.updatePrimaryColor(...rgb, 1);
				if (setColorChoice === "Secondary") editor.instance.updateSecondaryColor(...rgb, 1);
			}
		});

		// Update scrollbars and rulers
		editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, async (data) => {
			await tick();

			const { position, size, multiplier } = data;
			updateDocumentScrollbars(position, size, multiplier);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, async (data) => {
			await tick();

			const { origin, spacing, interval, visible } = data;
			updateDocumentRulers(origin, spacing, interval, visible);
		});

		// Update mouse cursor icon
		editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, async (data) => {
			await tick();

			const { cursor } = data;
			updateMouseCursor(cursor);
		});

		// Text entry
		editor.subscriptions.subscribeJsMessage(TriggerTextCommit, async () => {
			await tick();

			triggerTextCommit();
		});
		editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, async (data) => {
			await tick();

			displayEditableTextbox(data);
		});
		editor.subscriptions.subscribeJsMessage(DisplayEditableTextboxTransform, async (data) => {
			textInputMatrix = data.transform;
		});
		editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, async () => {
			await tick();

			displayRemoveEditableTextbox();
		});

		// Resize elements to render the new viewport size
		editor.subscriptions.subscribeJsMessage(TriggerViewportResize, async () => {
			await tick();

			viewportResize();
		});

		// Once this component is mounted, we want to resend the document bounds to the backend via the resize event handler which does that
		window.dispatchEvent(new Event("resize"));
	});
</script>

<LayoutCol class="document">
	<LayoutRow class="options-bar" classes={{ "for-graph": $document.graphViewOverlayOpen }} scrollableX={true}>
		{#if !$document.graphViewOverlayOpen}
			<WidgetLayout layout={$document.documentModeLayout} />
			<WidgetLayout layout={$document.toolOptionsLayout} />
			<LayoutRow class="spacer" />
			<WidgetLayout layout={$document.documentBarLayout} />
		{:else}
			<WidgetLayout layout={$document.nodeGraphBarLayout} />
		{/if}
	</LayoutRow>
	<LayoutRow
		class="shelf-and-table"
		styles={toolShelfTotalToolsAndSeparators && {
			"--total-separators": toolShelfTotalToolsAndSeparators.totalSeparators,
			"--total-tool-rows-for-1-columns": toolShelfTotalToolsAndSeparators.totalToolRowsFor1Columns,
			"--total-tool-rows-for-2-columns": toolShelfTotalToolsAndSeparators.totalToolRowsFor2Columns,
			"--total-tool-rows-for-3-columns": toolShelfTotalToolsAndSeparators.totalToolRowsFor3Columns,
		}}
	>
		<LayoutCol class="shelf">
			{#if !$document.graphViewOverlayOpen}
				<LayoutCol class="tools" scrollableY={true}>
					<WidgetLayout layout={$document.toolShelfLayout} />
				</LayoutCol>
			{:else}
				<LayoutRow class="spacer" />
			{/if}
			<LayoutCol class="shelf-bottom-widgets">
				<WidgetLayout class={"working-colors-input-area"} layout={$document.workingColorsLayout} />
			</LayoutCol>
		</LayoutCol>
		<LayoutCol class="table">
			{#if rulersVisible}
				<LayoutRow class="ruler-or-scrollbar top-ruler">
					<RulerInput origin={rulerOrigin.x} majorMarkSpacing={rulerSpacing} numberInterval={rulerInterval} direction="Horizontal" bind:this={rulerHorizontal} />
				</LayoutRow>
			{/if}
			<LayoutRow class="viewport-container">
				{#if rulersVisible}
					<LayoutCol class="ruler-or-scrollbar">
						<RulerInput origin={rulerOrigin.y} majorMarkSpacing={rulerSpacing} numberInterval={rulerInterval} direction="Vertical" bind:this={rulerVertical} />
					</LayoutCol>
				{/if}
				<LayoutCol class="viewport-container" styles={{ cursor: canvasCursor }}>
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
					<div class="viewport" on:pointerdown={(e) => canvasPointerDown(e)} on:dragover={(e) => e.preventDefault()} on:drop={(e) => pasteFile(e)} bind:this={viewport} data-viewport>
						<svg class="artboards" style:width={canvasWidthCSS} style:height={canvasHeightCSS}>
							{@html artworkSvg}
						</svg>
						<div class="text-input" style:width={canvasWidthCSS} style:height={canvasHeightCSS} style:pointer-events={showTextInput ? "auto" : ""}>
							{#if showTextInput}
								<div bind:this={textInput} style:transform="matrix({textInputMatrix})" />
							{/if}
						</div>
						<canvas class="overlays" width={canvasWidthRoundedToEven} height={canvasHeightRoundedToEven} style:width={canvasWidthCSS} style:height={canvasHeightCSS} data-overlays-canvas>
						</canvas>
					</div>
					<div class="graph-view" class:open={$document.graphViewOverlayOpen} style:--fade-artwork="80%" data-graph>
						<Graph />
					</div>
				</LayoutCol>
				<LayoutCol class="ruler-or-scrollbar right-scrollbar">
					<ScrollbarInput
						direction="Vertical"
						handleLength={scrollbarSize.y}
						handlePosition={scrollbarPos.y}
						on:handlePosition={({ detail }) => translateCanvasY(detail)}
						on:pressTrack={({ detail }) => pageY(detail)}
					/>
				</LayoutCol>
			</LayoutRow>
			<LayoutRow class="ruler-or-scrollbar bottom-scrollbar">
				<ScrollbarInput
					direction="Horizontal"
					handleLength={scrollbarSize.x}
					handlePosition={scrollbarPos.x}
					on:handlePosition={({ detail }) => translateCanvasX(detail)}
					on:pressTrack={({ detail }) => pageX(detail)}
				/>
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.document {
		height: 100%;

		&.document.document {
			padding-bottom: 0;
		}

		.options-bar {
			height: 32px;
			flex: 0 0 auto;
			margin: 0 4px;

			.spacer {
				min-width: 40px;
			}

			&.for-graph {
				justify-content: space-between;
			}
		}

		.shelf-and-table {
			// Enables usage of the `100cqh` unit to reference the height of this container element.
			container-type: size;

			// Update this if the tool icons change width in the future.
			--tool-width: 32;
			// Update this if the items below the tools (i.e. the working colors) change height in the future.
			--height-of-elements-below-tools: 72px;
			// Update this if the height changes as set in `Separator.svelte`.
			--height-of-separator: calc(12px + 1px + 12px);

			// Target height for the tools within the container above the lower elements.
			--available-height: calc(100cqh - var(--height-of-elements-below-tools));
			// The least height required to fit all the tools in 1 column and 2 columns, which the available space must exceed in order for the fewest needed columns to be used.
			--1-col-required-height: calc(var(--total-tool-rows-for-1-columns) * calc(var(--tool-width) * 1px) + var(--total-separators) * var(--height-of-separator));
			--2-col-required-height: calc(var(--total-tool-rows-for-2-columns) * calc(var(--tool-width) * 1px) + var(--total-separators) * var(--height-of-separator));

			// These evaluate to 0px (if false) or 1px (if true). (We multiply by 1000000 to force the result to be a discrete integer 0 or 1 and not interpolate values in-between.)
			--needs-at-least-1-column: 1px; // Always true
			--needs-at-least-2-columns: calc(1px - clamp(0px, calc((var(--available-height) - Min(var(--available-height), var(--1-col-required-height))) * 1000000), 1px));
			--needs-at-least-3-columns: calc(1px - clamp(0px, calc((var(--available-height) - Min(var(--available-height), var(--2-col-required-height))) * 1000000), 1px));
			--columns: calc(var(--needs-at-least-1-column) + var(--needs-at-least-2-columns) + var(--needs-at-least-3-columns));
			--columns-width: calc(var(--columns) * var(--tool-width));
			--columns-width-max: calc(3px * var(--tool-width));

			.shelf {
				flex: 0 0 auto;
				justify-content: space-between;
				// A precaution in case the variables above somehow fail
				max-width: var(--columns-width-max);

				.tools {
					flex: 0 1 auto;

					// Firefox-specific workaround for this bug causing the scrollbar to cover up the toolbar instead of widening to accommodate the scrollbar:
					// <https://bugzilla.mozilla.org/show_bug.cgi?id=764076>
					// <https://stackoverflow.com/questions/63278303/firefox-does-not-take-vertical-scrollbar-width-into-account-when-calculating-par>
					// Remove this when the Firefox bug is fixed.
					@-moz-document url-prefix() {
						--available-height-plus-1: calc(var(--available-height) + 1px);
						--3-col-required-height: calc(var(--total-tool-rows-for-3-columns) * calc(var(--tool-width) * 1px) + var(--total-separators) * var(--separator-height));
						--overflows-with-3-columns: calc(1px - clamp(0px, calc((var(--available-height-plus-1) - Min(var(--available-height-plus-1), var(--3-col-required-height))) * 1000000), 1px));
						--firefox-scrollbar-width-space-occupied: 8; // Might change someday, or on different platforms, but this is the value in FF 120 on Windows
						padding-right: calc(var(--firefox-scrollbar-width-space-occupied) * var(--overflows-with-3-columns));
					}

					.widget-span {
						flex-wrap: wrap;
						width: var(--columns-width);

						.icon-button {
							margin: 0;

							&[title^="Coming Soon"] {
								opacity: 0.25;
								transition: opacity 0.2s;

								&:hover {
									opacity: 1;
								}
							}

							&:not(.active) {
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

						.separator {
							min-height: 0;
						}
					}
				}

				.shelf-bottom-widgets {
					flex: 0 0 auto;
					align-items: center;

					.working-colors-input-area {
						height: auto;
						margin: 0;
						min-height: 0;

						.working-colors-input {
							margin: 0;
						}

						.icon-button {
							--widget-height: 0;
						}
					}
				}
			}

			.table {
				flex: 1 1 100%;

				.ruler-or-scrollbar {
					flex: 0 0 auto;
				}

				.top-ruler .ruler-input {
					padding-left: 16px;
					margin-right: 16px;
				}

				.right-scrollbar .scrollbar-input {
					margin-top: -16px;
				}

				.bottom-scrollbar .scrollbar-input {
					margin-right: 16px;
				}

				.viewport-container {
					flex: 1 1 100%;
					position: relative;

					.viewport {
						background: var(--color-2-mildblack);
						width: 100%;
						height: 100%;
						// Allows the SVG to be placed at explicit integer values of width and height to prevent non-pixel-perfect SVG scaling
						position: relative;
						overflow: hidden;

						.artwork,
						.text-input,
						.overlays {
							position: absolute;
							top: 0;
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

						.text-input div {
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

					.graph-view {
						pointer-events: none;
						transition: opacity 0.2s ease-in-out;
						opacity: 0;

						&.open {
							cursor: auto;
							pointer-events: auto;
							opacity: 1;
						}

						&::before {
							content: "";
							position: absolute;
							top: 0;
							left: 0;
							width: 100%;
							height: 100%;
							background: var(--color-2-mildblack);
							opacity: var(--fade-artwork);
							pointer-events: none;
						}
					}

					.fade-artwork,
					.graph {
						position: absolute;
						top: 0;
						left: 0;
						width: 100%;
						height: 100%;
					}
				}
			}
		}
	}
</style>
