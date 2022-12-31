<template>
	<LayoutCol class="document">
		<LayoutRow class="options-bar" :scrollableX="true">
			<WidgetLayout :layout="document.state.documentModeLayout" />
			<WidgetLayout :layout="document.state.toolOptionsLayout" />

			<LayoutRow class="spacer"></LayoutRow>

			<WidgetLayout :layout="document.state.documentBarLayout" />
		</LayoutRow>
		<LayoutRow class="shelf-and-viewport">
			<LayoutCol class="shelf">
				<LayoutCol class="tools" :scrollableY="true">
					<WidgetLayout :layout="document.state.toolShelfLayout" />
				</LayoutCol>

				<LayoutCol class="spacer"></LayoutCol>

				<LayoutCol class="working-colors">
					<WidgetLayout :layout="document.state.workingColorsLayout" />
				</LayoutCol>
			</LayoutCol>
			<LayoutCol class="viewport">
				<LayoutRow class="bar-area">
					<CanvasRuler :origin="rulerOrigin.x" :majorMarkSpacing="rulerSpacing" :numberInterval="rulerInterval" :direction="'Horizontal'" class="top-ruler" ref="rulerHorizontal" />
				</LayoutRow>
				<LayoutRow class="canvas-area">
					<LayoutCol class="bar-area">
						<CanvasRuler :origin="rulerOrigin.y" :majorMarkSpacing="rulerSpacing" :numberInterval="rulerInterval" :direction="'Vertical'" ref="rulerVertical" />
					</LayoutCol>
					<LayoutCol class="canvas-area" :style="{ cursor: canvasCursor }">
						<EyedropperPreview
							v-if="cursorEyedropper"
							:colorChoice="cursorEyedropperPreviewColorChoice"
							:primaryColor="cursorEyedropperPreviewColorPrimary"
							:secondaryColor="cursorEyedropperPreviewColorSecondary"
							:imageData="cursorEyedropperPreviewImageData"
							:style="{ left: cursorLeft + 'px', top: cursorTop + 'px' }"
						/>
						<div class="canvas" @pointerdown="(e: PointerEvent) => canvasPointerDown(e)" @dragover="(e) => e.preventDefault()" @drop="(e) => pasteFile(e)" ref="canvasDiv" data-canvas>
							<svg class="artboards" v-html="artboardSvg" :style="{ width: canvasWidthCSS, height: canvasHeightCSS }"></svg>
							<svg
								class="artwork"
								xmlns="http://www.w3.org/2000/svg"
								xmlns:xlink="http://www.w3.org/1999/xlink"
								v-html="artworkSvg"
								:style="{ width: canvasWidthCSS, height: canvasHeightCSS }"
							></svg>
							<svg class="overlays" v-html="overlaysSvg" :style="{ width: canvasWidthCSS, height: canvasHeightCSS }"></svg>
						</div>
					</LayoutCol>
					<LayoutCol class="bar-area">
						<PersistentScrollbar
							:direction="'Vertical'"
							:handlePosition="scrollbarPos.y"
							@update:handlePosition="(newValue: number) => translateCanvasY(newValue)"
							v-model:handleLength="scrollbarSize.y"
							@pressTrack="(delta: number) => pageY(delta)"
							class="right-scrollbar"
						/>
					</LayoutCol>
				</LayoutRow>
				<LayoutRow class="bar-area">
					<PersistentScrollbar
						:direction="'Horizontal'"
						:handlePosition="scrollbarPos.x"
						@update:handlePosition="(newValue: number) => translateCanvasX(newValue)"
						v-model:handleLength="scrollbarSize.x"
						@pressTrack="(delta: number) => pageX(delta)"
						class="bottom-scrollbar"
					/>
				</LayoutRow>
			</LayoutCol>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
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

			.right-scrollbar {
				margin-top: -16px;
			}

			.bottom-scrollbar {
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

<script lang="ts">
import { defineComponent, nextTick } from "vue";

import { textInputCleanup } from "@/utility-functions/keyboard-entry";
import { rasterizeSVGCanvas } from "@/utility-functions/rasterization";
import { type DisplayEditableTextbox, type MouseCursorIcon, type XY } from "@/wasm-communication/messages";

import EyedropperPreview, { ZOOM_WINDOW_DIMENSIONS } from "@/components/floating-menus/EyedropperPreview.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import CanvasRuler from "@/components/widgets/metrics/CanvasRuler.vue";
import PersistentScrollbar from "@/components/widgets/metrics/PersistentScrollbar.vue";
import WidgetLayout from "@/components/widgets/WidgetLayout.vue";

export default defineComponent({
	inject: ["editor", "panels", "document"],
	data() {
		const scrollbarPos: XY = { x: 0.5, y: 0.5 };
		const scrollbarSize: XY = { x: 0.5, y: 0.5 };
		const scrollbarMultiplier: XY = { x: 0, y: 0 };

		const rulerOrigin: XY = { x: 0, y: 0 };

		return {
			// Interactive text editing
			textInput: undefined as undefined | HTMLDivElement,

			// CSS properties
			canvasSvgWidth: undefined as number | undefined,
			canvasSvgHeight: undefined as number | undefined,
			canvasCursor: "default",

			// Scrollbars
			scrollbarPos,
			scrollbarSize,
			scrollbarMultiplier,

			// Rulers
			rulerOrigin,
			rulerSpacing: 100 as number,
			rulerInterval: 100 as number,

			// Rendered SVG viewport data
			artworkSvg: "" as string,
			artboardSvg: "" as string,
			overlaysSvg: "" as string,

			// Rasterized SVG viewport data, or none if it's not up-to-date
			rasterizedCanvas: undefined as HTMLCanvasElement | undefined,
			rasterizedContext: undefined as CanvasRenderingContext2D | undefined,

			// Cursor position for cursor floating menus like the Eyedropper tool zoom
			cursorLeft: 0,
			cursorTop: 0,
			cursorEyedropper: false,
			cursorEyedropperPreviewImageData: undefined as ImageData | undefined,
			cursorEyedropperPreviewColorChoice: "",
			cursorEyedropperPreviewColorPrimary: "",
			cursorEyedropperPreviewColorSecondary: "",
		};
	},
	mounted() {
		this.panels.registerPanel("Document", this);

		// Once this component is mounted, we want to resend the document bounds to the backend via the resize event handler which does that
		window.dispatchEvent(new Event("resize"));
	},
	methods: {
		pasteFile(e: DragEvent) {
			const { dataTransfer } = e;
			if (!dataTransfer) return;
			e.preventDefault();

			Array.from(dataTransfer.items).forEach(async (item) => {
				const file = item.getAsFile();
				if (file?.type.startsWith("image")) {
					const buffer = await file.arrayBuffer();
					const u8Array = new Uint8Array(buffer);

					this.editor.instance.pasteImage(file.type, u8Array, e.clientX, e.clientY);
				}
			});
		},
		translateCanvasX(newValue: number) {
			const delta = newValue - this.scrollbarPos.x;
			this.scrollbarPos.x = newValue;
			this.editor.instance.translateCanvas(-delta * this.scrollbarMultiplier.x, 0);
		},
		translateCanvasY(newValue: number) {
			const delta = newValue - this.scrollbarPos.y;
			this.scrollbarPos.y = newValue;
			this.editor.instance.translateCanvas(0, -delta * this.scrollbarMultiplier.y);
		},
		pageX(delta: number) {
			const move = delta < 0 ? 1 : -1;
			this.editor.instance.translateCanvasByFraction(move, 0);
		},
		pageY(delta: number) {
			const move = delta < 0 ? 1 : -1;
			this.editor.instance.translateCanvasByFraction(0, move);
		},
		canvasPointerDown(e: PointerEvent) {
			const onEditbox = e.target instanceof HTMLDivElement && e.target.contentEditable;

			if (!onEditbox) (this.$refs.canvasDiv as HTMLDivElement | undefined)?.setPointerCapture(e.pointerId);
		},
		// Update rendered SVGs
		async updateDocumentArtwork(svg: string) {
			this.artworkSvg = svg;
			this.rasterizedCanvas = undefined;

			await nextTick();

			if (this.textInput) {
				const canvasDiv = this.$refs.canvasDiv as HTMLDivElement | undefined;
				if (!canvasDiv) return;

				const foreignObject = canvasDiv.getElementsByTagName("foreignObject")[0] as SVGForeignObjectElement;
				if (foreignObject.children.length > 0) return;

				const addedInput = foreignObject.appendChild(this.textInput);
				window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: addedInput }));

				await nextTick();

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
		},
		updateDocumentOverlays(svg: string) {
			this.overlaysSvg = svg;
		},
		updateDocumentArtboards(svg: string) {
			this.artboardSvg = svg;
			this.rasterizedCanvas = undefined;
		},
		async updateEyedropperSamplingState(mousePosition: XY | undefined, colorPrimary: string, colorSecondary: string): Promise<[number, number, number] | undefined> {
			if (mousePosition === undefined) {
				this.cursorEyedropper = false;
				return undefined;
			}
			this.cursorEyedropper = true;

			if (this.canvasSvgWidth === undefined || this.canvasSvgHeight === undefined) return undefined;

			this.cursorLeft = mousePosition.x;
			this.cursorTop = mousePosition.y;

			// This works nearly perfectly, but sometimes at odd DPI scale factors like 1.25, the anti-aliasing color can yield slightly incorrect colors (potential room for future improvement)
			const dpiFactor = window.devicePixelRatio;
			const [width, height] = [this.canvasSvgWidth, this.canvasSvgHeight];

			const outsideArtboardsColor = getComputedStyle(document.documentElement).getPropertyValue("--color-2-mildblack");
			const outsideArtboards = `<rect x="0" y="0" width="100%" height="100%" fill="${outsideArtboardsColor}" />`;
			const artboards = this.artboardSvg;
			const artwork = this.artworkSvg;
			const svg = `
				<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">${outsideArtboards}${artboards}${artwork}</svg>
				`.trim();

			if (!this.rasterizedCanvas) {
				this.rasterizedCanvas = await rasterizeSVGCanvas(svg, width * dpiFactor, height * dpiFactor, "image/png");
				this.rasterizedContext = this.rasterizedCanvas.getContext("2d") || undefined;
			}
			if (!this.rasterizedContext) return undefined;

			const rgbToHex = (r: number, g: number, b: number): string => `#${[r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("")}`;

			const pixel = this.rasterizedContext.getImageData(mousePosition.x * dpiFactor, mousePosition.y * dpiFactor, 1, 1).data;
			const hex = rgbToHex(pixel[0], pixel[1], pixel[2]);
			const rgb: [number, number, number] = [pixel[0] / 255, pixel[1] / 255, pixel[2] / 255];

			this.cursorEyedropperPreviewColorChoice = hex;
			this.cursorEyedropperPreviewColorPrimary = colorPrimary;
			this.cursorEyedropperPreviewColorSecondary = colorSecondary;

			const previewRegion = this.rasterizedContext.getImageData(
				mousePosition.x * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
				mousePosition.y * dpiFactor - (ZOOM_WINDOW_DIMENSIONS - 1) / 2,
				ZOOM_WINDOW_DIMENSIONS,
				ZOOM_WINDOW_DIMENSIONS
			);
			this.cursorEyedropperPreviewImageData = previewRegion;

			return rgb;
		},
		// Update scrollbars and rulers
		updateDocumentScrollbars(position: XY, size: XY, multiplier: XY) {
			this.scrollbarPos = position;
			this.scrollbarSize = size;
			this.scrollbarMultiplier = multiplier;
		},
		updateDocumentRulers(origin: XY, spacing: number, interval: number) {
			this.rulerOrigin = origin;
			this.rulerSpacing = spacing;
			this.rulerInterval = interval;
		},
		// Update mouse cursor icon
		updateMouseCursor(cursor: MouseCursorIcon) {
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

			this.canvasCursor = cursorString;
		},
		// Text entry
		triggerTextCommit() {
			if (!this.textInput) return;
			const textCleaned = textInputCleanup(this.textInput.innerText);
			this.editor.instance.onChangeText(textCleaned);
		},
		displayEditableTextbox(displayEditableTextbox: DisplayEditableTextbox) {
			this.textInput = document.createElement("div") as HTMLDivElement;

			if (displayEditableTextbox.text === "") this.textInput.textContent = "";
			else this.textInput.textContent = `${displayEditableTextbox.text}\n`;

			this.textInput.contentEditable = "true";
			this.textInput.style.width = displayEditableTextbox.lineWidth ? `${displayEditableTextbox.lineWidth}px` : "max-content";
			this.textInput.style.height = "auto";
			this.textInput.style.fontSize = `${displayEditableTextbox.fontSize}px`;
			this.textInput.style.color = displayEditableTextbox.color.toHexOptionalAlpha() || "transparent";

			this.textInput.oninput = (): void => {
				if (!this.textInput) return;
				this.editor.instance.updateBounds(textInputCleanup(this.textInput.innerText));
			};
		},
		displayRemoveEditableTextbox() {
			this.textInput = undefined;
			window.dispatchEvent(new CustomEvent("modifyinputfield", { detail: undefined }));
		},
		// Resize elements to render the new viewport size
		viewportResize() {
			// Resize the canvas
			const canvasDiv = this.$refs.canvasDiv as HTMLDivElement | undefined;
			if (!canvasDiv) return;

			this.canvasSvgWidth = Math.ceil(parseFloat(getComputedStyle(canvasDiv).width));
			this.canvasSvgHeight = Math.ceil(parseFloat(getComputedStyle(canvasDiv).height));

			// Resize the rulers
			(this.$refs.rulerHorizontal as typeof CanvasRuler | undefined)?.resize();
			(this.$refs.rulerVertical as typeof CanvasRuler | undefined)?.resize();
		},
		canvasDimensionCSS(dimension: number | undefined): string {
			// Temporary placeholder until the first actual value is populated
			// This at least gets close to the correct value but an actual number is required to prevent CSS from causing non-integer sizing making the SVG render with anti-aliasing
			if (dimension === undefined) return "100%";

			// Dimension is rounded up to the nearest even number because resizing is centered, and dividing an odd number by 2 for centering causes antialiasing
			return `${dimension % 2 === 1 ? dimension + 1 : dimension}px`;
		},
	},
	computed: {
		canvasWidthCSS(): string {
			return this.canvasDimensionCSS(this.canvasSvgWidth);
		},
		canvasHeightCSS(): string {
			return this.canvasDimensionCSS(this.canvasSvgHeight);
		},
	},
	components: {
		CanvasRuler,
		LayoutCol,
		LayoutRow,
		PersistentScrollbar,
		WidgetLayout,
		EyedropperPreview,
	},
});
</script>
