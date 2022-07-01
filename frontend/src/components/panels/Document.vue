<template>
	<LayoutCol class="document">
		<LayoutRow class="options-bar" :scrollableX="true">
			<WidgetLayout :layout="documentModeLayout" />
			<WidgetLayout :layout="toolOptionsLayout" />

			<LayoutRow class="spacer"></LayoutRow>

			<WidgetLayout :layout="documentBarLayout" />
		</LayoutRow>
		<LayoutRow class="shelf-and-viewport">
			<LayoutCol class="shelf">
				<LayoutCol class="tools" :scrollableY="true">
					<WidgetLayout :layout="toolShelfLayout" />
				</LayoutCol>

				<LayoutCol class="spacer"></LayoutCol>

				<LayoutCol class="working-colors">
					<SwatchPairInput />
					<LayoutRow class="swap-and-reset">
						<!-- TODO: Remember to make these tooltip input hints customized to macOS also -->
						<IconButton :action="swapWorkingColors" :icon="'Swap'" title="Swap (Shift+X)" :size="16" />
						<IconButton :action="resetWorkingColors" :icon="'ResetColors'" title="Reset (Ctrl+Shift+X)" :size="16" />
					</LayoutRow>
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
					<LayoutCol class="canvas-area">
						<div
							class="canvas"
							:style="{ cursor: canvasCursor }"
							@pointerdown="(e: PointerEvent) => canvasPointerDown(e)"
							@dragover="(e) => e.preventDefault()"
							@drop="(e) => pasteFile(e)"
							ref="canvas"
							data-canvas
						>
							<svg class="artboards" v-html="artboardSvg" :style="{ width: canvasSvgWidth, height: canvasSvgHeight }"></svg>
							<svg
								class="artwork"
								xmlns="http://www.w3.org/2000/svg"
								xmlns:xlink="http://www.w3.org/1999/xlink"
								v-html="artworkSvg"
								:style="{ width: canvasSvgWidth, height: canvasSvgHeight }"
							></svg>
							<svg class="overlays" v-html="overlaysSvg" :style="{ width: canvasSvgWidth, height: canvasSvgHeight }"></svg>
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

			.spacer {
				flex: 1 0 auto;
				min-height: 8px;
			}

			.working-colors {
				flex: 0 0 auto;

				.swap-and-reset {
					flex: 0 0 auto;
				}
			}
		}

		.viewport {
			flex: 1 1 100%;

			.canvas-area {
				flex: 1 1 100%;
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
						background: none;
						cursor: text;
						border: none;
						margin: 0;
						padding: 0;
						overflow: visible;
						white-space: pre-wrap;
						display: inline-block;
						// Workaround to force Chrome to display the flashing text entry cursor when text is empty
						padding-left: 1px;
						margin-left: -1px;
					}

					div:focus {
						border: none;
						outline: none;
						margin: -1px;
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
import {
	UpdateDocumentArtwork,
	UpdateDocumentOverlays,
	UpdateDocumentScrollbars,
	UpdateDocumentRulers,
	UpdateDocumentArtboards,
	UpdateMouseCursor,
	UpdateDocumentModeLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	defaultWidgetLayout,
	UpdateDocumentBarLayout,
	UpdateImageData,
	TriggerTextCommit,
	TriggerViewportResize,
	DisplayRemoveEditableTextbox,
	DisplayEditableTextbox,
} from "@/wasm-communication/messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.vue";
import CanvasRuler from "@/components/widgets/metrics/CanvasRuler.vue";
import PersistentScrollbar from "@/components/widgets/metrics/PersistentScrollbar.vue";
import WidgetLayout from "@/components/widgets/WidgetLayout.vue";

export default defineComponent({
	inject: ["editor"],
	methods: {
		viewportResize() {
			// Resize the canvas

			const canvas = this.$refs.canvas as HTMLElement;

			// Get the width and height rounded up to the nearest even number because resizing is centered and dividing an odd number by 2 for centering causes antialiasing
			let width = Math.ceil(parseFloat(getComputedStyle(canvas).width));
			if (width % 2 === 1) width += 1;
			let height = Math.ceil(parseFloat(getComputedStyle(canvas).height));
			if (height % 2 === 1) height += 1;

			this.canvasSvgWidth = `${width}px`;
			this.canvasSvgHeight = `${height}px`;

			// Resize the rulers
			const rulerHorizontal = this.$refs.rulerHorizontal as typeof CanvasRuler;
			const rulerVertical = this.$refs.rulerVertical as typeof CanvasRuler;
			rulerHorizontal?.resize();
			rulerVertical?.resize();
		},
		pasteFile(e: DragEvent) {
			const { dataTransfer } = e;
			if (!dataTransfer) return;
			e.preventDefault();

			Array.from(dataTransfer.items).forEach(async (item) => {
				const file = item.getAsFile();
				if (file?.type.startsWith("image")) {
					const buffer = await file.arrayBuffer();
					const u8Array = new Uint8Array(buffer);

					this.editor.instance.paste_image(file.type, u8Array, e.clientX, e.clientY);
				}
			});
		},
		translateCanvasX(newValue: number) {
			const delta = newValue - this.scrollbarPos.x;
			this.scrollbarPos.x = newValue;
			this.editor.instance.translate_canvas(-delta * this.scrollbarMultiplier.x, 0);
		},
		translateCanvasY(newValue: number) {
			const delta = newValue - this.scrollbarPos.y;
			this.scrollbarPos.y = newValue;
			this.editor.instance.translate_canvas(0, -delta * this.scrollbarMultiplier.y);
		},
		pageX(delta: number) {
			const move = delta < 0 ? 1 : -1;
			this.editor.instance.translate_canvas_by_fraction(move, 0);
		},
		pageY(delta: number) {
			const move = delta < 0 ? 1 : -1;
			this.editor.instance.translate_canvas_by_fraction(0, move);
		},
		swapWorkingColors() {
			this.editor.instance.swap_colors();
		},
		resetWorkingColors() {
			this.editor.instance.reset_colors();
		},
		canvasPointerDown(e: PointerEvent) {
			const onEditbox = e.target instanceof HTMLDivElement && e.target.contentEditable;
			if (!onEditbox) {
				const canvas = this.$refs.canvas as HTMLElement;
				canvas.setPointerCapture(e.pointerId);
			}
		},
	},
	mounted() {
		this.editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, async (UpdateDocumentArtwork) => {
			this.artworkSvg = UpdateDocumentArtwork.svg;

			await nextTick();

			if (this.textInput) {
				const canvas = this.$refs.canvas as HTMLElement;
				const foreignObject = canvas.getElementsByTagName("foreignObject")[0] as SVGForeignObjectElement;
				if (foreignObject.children.length > 0) return;

				const addedInput = foreignObject.appendChild(this.textInput);

				window.dispatchEvent(
					new CustomEvent("modifyinputfield", {
						detail: addedInput,
					})
				);

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
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateDocumentOverlays, (updateDocumentOverlays) => {
			this.overlaysSvg = updateDocumentOverlays.svg;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateDocumentArtboards, (updateDocumentArtboards) => {
			this.artboardSvg = updateDocumentArtboards.svg;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, (updateDocumentScrollbars) => {
			this.scrollbarPos = updateDocumentScrollbars.position;
			this.scrollbarSize = updateDocumentScrollbars.size;
			this.scrollbarMultiplier = updateDocumentScrollbars.multiplier;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, (updateDocumentRulers) => {
			this.rulerOrigin = updateDocumentRulers.origin;
			this.rulerSpacing = updateDocumentRulers.spacing;
			this.rulerInterval = updateDocumentRulers.interval;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, (updateMouseCursor) => {
			this.canvasCursor = updateMouseCursor.cursor;
		});

		this.editor.subscriptions.subscribeJsMessage(TriggerTextCommit, () => {
			if (this.textInput) {
				const textCleaned = textInputCleanup(this.textInput.innerText);
				this.editor.instance.on_change_text(textCleaned);
			}
		});

		this.editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, (displayEditableTextbox) => {
			this.textInput = document.createElement("DIV") as HTMLDivElement;

			if (displayEditableTextbox.text === "") this.textInput.textContent = "";
			else this.textInput.textContent = `${displayEditableTextbox.text}\n`;

			this.textInput.contentEditable = "true";
			this.textInput.style.width = displayEditableTextbox.line_width ? `${displayEditableTextbox.line_width}px` : "max-content";
			this.textInput.style.height = "auto";
			this.textInput.style.fontSize = `${displayEditableTextbox.font_size}px`;
			this.textInput.style.color = displayEditableTextbox.color.toRgbaCSS();

			this.textInput.oninput = (): void => {
				if (this.textInput) this.editor.instance.update_bounds(textInputCleanup(this.textInput.innerText));
			};
		});

		this.editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, () => {
			this.textInput = undefined;
			window.dispatchEvent(
				new CustomEvent("modifyinputfield", {
					detail: undefined,
				})
			);
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, (updateDocumentModeLayout) => {
			this.documentModeLayout = updateDocumentModeLayout;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, (updateToolOptionsLayout) => {
			this.toolOptionsLayout = updateToolOptionsLayout;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, (updateDocumentBarLayout) => {
			this.documentBarLayout = updateDocumentBarLayout;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, (updateToolShelfLayout) => {
			this.toolShelfLayout = updateToolShelfLayout;
		});

		this.editor.subscriptions.subscribeJsMessage(TriggerViewportResize, this.viewportResize);

		this.editor.subscriptions.subscribeJsMessage(UpdateImageData, (updateImageData) => {
			updateImageData.image_data.forEach(async (element) => {
				// Using updateImageData.image_data.buffer returns undefined for some reason?
				const blob = new Blob([new Uint8Array(element.image_data.values()).buffer], { type: element.mime });

				const url = URL.createObjectURL(blob);

				const image = await createImageBitmap(blob);

				this.editor.instance.set_image_blob_url(element.path, url, image.width, image.height);
			});
		});

		// trigger resize on mount to send document bounds to the backend
		window.dispatchEvent(new Event("resize"));
	},
	data() {
		return {
			// Interactive text editing
			textInput: undefined as undefined | HTMLDivElement,

			// CSS properties
			canvasSvgWidth: "100%",
			canvasSvgHeight: "100%",
			canvasCursor: "default",

			// Scrollbars
			scrollbarPos: { x: 0.5, y: 0.5 },
			scrollbarSize: { x: 0.5, y: 0.5 },
			scrollbarMultiplier: { x: 0, y: 0 },

			// Rulers
			rulerOrigin: { x: 0, y: 0 },
			rulerSpacing: 100,
			rulerInterval: 100,

			// Rendered SVG viewport data
			artworkSvg: "",
			artboardSvg: "",
			overlaysSvg: "",

			// Layouts
			documentModeLayout: defaultWidgetLayout(),
			toolOptionsLayout: defaultWidgetLayout(),
			documentBarLayout: defaultWidgetLayout(),
			toolShelfLayout: defaultWidgetLayout(),
		};
	},
	components: {
		LayoutRow,
		LayoutCol,
		SwatchPairInput,
		PersistentScrollbar,
		CanvasRuler,
		IconButton,
		WidgetLayout,
	},
});
</script>
