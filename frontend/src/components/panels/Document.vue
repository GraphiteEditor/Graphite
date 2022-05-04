<template>
	<LayoutCol class="document">
		<LayoutRow class="options-bar" :scrollableX="true">
			<LayoutRow class="left side">
				<DropdownInput :menuEntries="documentModeEntries" v-model:selectedIndex="documentModeSelectionIndex" :drawIcon="true" />

				<Separator :type="'Section'" />

				<WidgetLayout :layout="toolOptionsLayout" class="tool-options" />
			</LayoutRow>

			<LayoutRow class="spacer"></LayoutRow>

			<WidgetLayout :layout="documentBarLayout" class="right side document-bar" />
		</LayoutRow>
		<LayoutRow class="shelf-and-viewport">
			<LayoutCol class="shelf">
				<LayoutCol class="tools" :scrollableY="true">
					<ShelfItemInput icon="GeneralSelectTool" title="Select Tool (V)" :active="activeTool === 'Select'" :action="() => selectTool('Select')" />
					<ShelfItemInput icon="GeneralArtboardTool" title="Artboard Tool" :active="activeTool === 'Artboard'" :action="() => selectTool('Artboard')" />
					<ShelfItemInput icon="GeneralNavigateTool" title="Navigate Tool (Z)" :active="activeTool === 'Navigate'" :action="() => selectTool('Navigate')" />
					<ShelfItemInput icon="GeneralEyedropperTool" title="Eyedropper Tool (I)" :active="activeTool === 'Eyedropper'" :action="() => selectTool('Eyedropper')" />
					<ShelfItemInput icon="GeneralFillTool" title="Fill Tool (F)" :active="activeTool === 'Fill'" :action="() => selectTool('Fill')" />
					<ShelfItemInput icon="GeneralGradientTool" title="Gradient Tool (H)" :active="activeTool === 'Gradient'" :action="() => selectTool('Gradient')" />

					<Separator :type="'Section'" :direction="'Vertical'" />

					<ShelfItemInput icon="VectorPathTool" title="Path Tool (A)" :active="activeTool === 'Path'" :action="() => selectTool('Path')" />
					<ShelfItemInput icon="VectorPenTool" title="Pen Tool (P)" :active="activeTool === 'Pen'" :action="() => selectTool('Pen')" />
					<ShelfItemInput icon="VectorFreehandTool" title="Freehand Tool (N)" :active="activeTool === 'Freehand'" :action="() => selectTool('Freehand')" />
					<ShelfItemInput icon="VectorSplineTool" title="Spline Tool" :active="activeTool === 'Spline'" :action="() => selectTool('Spline')" />
					<ShelfItemInput icon="VectorLineTool" title="Line Tool (L)" :active="activeTool === 'Line'" :action="() => selectTool('Line')" />
					<ShelfItemInput icon="VectorRectangleTool" title="Rectangle Tool (M)" :active="activeTool === 'Rectangle'" :action="() => selectTool('Rectangle')" />
					<ShelfItemInput icon="VectorEllipseTool" title="Ellipse Tool (E)" :active="activeTool === 'Ellipse'" :action="() => selectTool('Ellipse')" />
					<ShelfItemInput icon="VectorShapeTool" title="Shape Tool (Y)" :active="activeTool === 'Shape'" :action="() => selectTool('Shape')" />
					<ShelfItemInput icon="VectorTextTool" title="Text Tool (T)" :active="activeTool === 'Text'" :action="() => selectTool('Text')" />

					<Separator :type="'Section'" :direction="'Vertical'" />

					<ShelfItemInput icon="RasterBrushTool" title="Coming Soon: Brush Tool (B)" :active="activeTool === 'Brush'" :action="() => (dialog.comingSoon(), false) && selectTool('Brush')" />
					<ShelfItemInput icon="RasterHealTool" title="Coming Soon: Heal Tool (J)" :active="activeTool === 'Heal'" :action="() => (dialog.comingSoon(), false) && selectTool('Heal')" />
					<ShelfItemInput icon="RasterCloneTool" title="Coming Soon: Clone Tool (C)" :active="activeTool === 'Clone'" :action="() => (dialog.comingSoon(), false) && selectTool('Clone')" />
					<ShelfItemInput icon="RasterPatchTool" title="Coming Soon: Patch Tool" :active="activeTool === 'Patch'" :action="() => (dialog.comingSoon(), false) && selectTool('Patch')" />
					<ShelfItemInput
						icon="RasterDetailTool"
						title="Coming Soon: Detail Tool (D)"
						:active="activeTool === 'Detail'"
						:action="() => (dialog.comingSoon(), false) && selectTool('Detail')"
					/>
					<ShelfItemInput
						icon="RasterRelightTool"
						title="Coming Soon: Relight Tool (O)"
						:active="activeTool === 'Relight'"
						:action="() => (dialog.comingSoon(), false) && selectTool('Relight')"
					/>
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
							data-canvas
							ref="canvas"
							:style="{ cursor: canvasCursor }"
							@pointerdown="(e: PointerEvent) => canvasPointerDown(e)"
							@dragover="(e) => e.preventDefault()"
							@drop="(e) => pasteFile(e)"
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

		.side {
			height: 100%;
			flex: 0 0 auto;
			align-items: center;
			margin: 0 4px;
		}

		.spacer {
			min-width: 40px;
		}
	}

	.shelf-and-viewport {
		.shelf {
			flex: 0 0 auto;

			.tools {
				flex: 0 1 auto;

				.shelf-item-input[title^="Coming Soon"] {
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

import {
	UpdateDocumentArtwork,
	UpdateDocumentOverlays,
	UpdateDocumentScrollbars,
	UpdateDocumentRulers,
	UpdateActiveTool,
	UpdateCanvasZoom,
	UpdateCanvasRotation,
	ToolName,
	UpdateDocumentArtboards,
	UpdateMouseCursor,
	UpdateToolOptionsLayout,
	defaultWidgetLayout,
	UpdateDocumentBarLayout,
	UpdateImageData,
	TriggerTextCommit,
	TriggerTextCopy,
	TriggerViewportResize,
	DisplayRemoveEditableTextbox,
	DisplayEditableTextbox,
	TriggerFontLoad,
	TriggerDefaultFontLoad,
	TriggerWindowOpen,
} from "@/dispatcher/js-messages";

import { textInputCleanup } from "@/lifetime/input";

import { loadDefaultFont, setLoadDefaultFontCallback } from "@/utilities/fonts";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconButton from "@/components/widgets/buttons/IconButton.vue";
import { SectionsOfMenuListEntries } from "@/components/widgets/floating-menus/MenuList.vue";
import DropdownInput from "@/components/widgets/inputs/DropdownInput.vue";
import { RadioEntries } from "@/components/widgets/inputs/RadioInput.vue";
import ShelfItemInput from "@/components/widgets/inputs/ShelfItemInput.vue";
import SwatchPairInput from "@/components/widgets/inputs/SwatchPairInput.vue";
import CanvasRuler from "@/components/widgets/rulers/CanvasRuler.vue";
import PersistentScrollbar from "@/components/widgets/scrollbars/PersistentScrollbar.vue";
import Separator from "@/components/widgets/separators/Separator.vue";
import WidgetLayout from "@/components/widgets/WidgetLayout.vue";

export default defineComponent({
	inject: ["editor", "dialog"],
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
			rulerHorizontal?.handleResize();
			rulerVertical?.handleResize();
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
		selectTool(toolName: string) {
			this.editor.instance.select_tool(toolName);
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
		this.editor.dispatcher.subscribeJsMessage(UpdateDocumentArtwork, (UpdateDocumentArtwork) => {
			this.artworkSvg = UpdateDocumentArtwork.svg;

			nextTick((): void => {
				if (this.textInput) {
					const canvas = this.$refs.canvas as HTMLElement;
					const foreignObject = canvas.getElementsByTagName("foreignObject")[0] as SVGForeignObjectElement;
					if (foreignObject.children.length > 0) return;

					const addedInput = foreignObject.appendChild(this.textInput);

					nextTick((): void => {
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
					});

					window.dispatchEvent(
						new CustomEvent("modifyinputfield", {
							detail: addedInput,
						})
					);
				}
			});
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateDocumentOverlays, (updateDocumentOverlays) => {
			this.overlaysSvg = updateDocumentOverlays.svg;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateDocumentArtboards, (updateDocumentArtboards) => {
			this.artboardSvg = updateDocumentArtboards.svg;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateDocumentScrollbars, (updateDocumentScrollbars) => {
			this.scrollbarPos = updateDocumentScrollbars.position;
			this.scrollbarSize = updateDocumentScrollbars.size;
			this.scrollbarMultiplier = updateDocumentScrollbars.multiplier;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateDocumentRulers, (updateDocumentRulers) => {
			this.rulerOrigin = updateDocumentRulers.origin;
			this.rulerSpacing = updateDocumentRulers.spacing;
			this.rulerInterval = updateDocumentRulers.interval;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateActiveTool, (updateActiveTool) => {
			this.activeTool = updateActiveTool.tool_name;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateCanvasZoom, (updateCanvasZoom) => {
			this.documentZoom = updateCanvasZoom.factor * 100;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateCanvasRotation, (updateCanvasRotation) => {
			const newRotation = updateCanvasRotation.angle_radians * (180 / Math.PI);
			this.documentRotation = (360 + (newRotation % 360)) % 360;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateMouseCursor, (updateMouseCursor) => {
			this.canvasCursor = updateMouseCursor.cursor;
		});
		this.editor.dispatcher.subscribeJsMessage(TriggerTextCommit, () => {
			if (this.textInput) {
				const textCleaned = textInputCleanup(this.textInput.innerText);
				this.editor.instance.on_change_text(textCleaned);
			}
		});
		this.editor.dispatcher.subscribeJsMessage(TriggerFontLoad, async (triggerFontLoad) => {
			const response = await fetch(triggerFontLoad.font);
			const responseBuffer = await response.arrayBuffer();
			this.editor.instance.on_font_load(triggerFontLoad.font, new Uint8Array(responseBuffer), false);
		});
		this.editor.dispatcher.subscribeJsMessage(TriggerWindowOpen, async (triggerWindowOpen) => {
			window.open(triggerWindowOpen.url, "_blank");
		});
		this.editor.dispatcher.subscribeJsMessage(TriggerDefaultFontLoad, loadDefaultFont);
		this.editor.dispatcher.subscribeJsMessage(TriggerTextCopy, (triggerTextCopy) => {
			// If the Clipboard API is supported in the browser, copy text to the clipboard
			navigator.clipboard?.writeText?.(triggerTextCopy.copy_text);
		});

		this.editor.dispatcher.subscribeJsMessage(DisplayEditableTextbox, (displayEditableTextbox) => {
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

		this.editor.dispatcher.subscribeJsMessage(DisplayRemoveEditableTextbox, () => {
			this.textInput = undefined;
			window.dispatchEvent(
				new CustomEvent("modifyinputfield", {
					detail: undefined,
				})
			);
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateToolOptionsLayout, (updateToolOptionsLayout) => {
			this.toolOptionsLayout = updateToolOptionsLayout;
		});

		this.editor.dispatcher.subscribeJsMessage(UpdateDocumentBarLayout, (updateDocumentBarLayout) => {
			this.documentBarLayout = updateDocumentBarLayout;
		});
		this.editor.dispatcher.subscribeJsMessage(TriggerViewportResize, this.viewportResize);

		this.editor.dispatcher.subscribeJsMessage(UpdateImageData, (updateImageData) => {
			updateImageData.image_data.forEach(async (element) => {
				// Using updateImageData.image_data.buffer returns undefined for some reason?
				const blob = new Blob([new Uint8Array(element.image_data.values()).buffer], { type: element.mime });

				const url = URL.createObjectURL(blob);

				const image = await createImageBitmap(blob);

				this.editor.instance.set_image_blob_url(element.path, url, image.width, image.height);
			});
		});

		const loadAboutGraphite = (): void => {
			const release = process.env.VUE_APP_RELEASE_SERIES;
			let timestamp = "";
			const hash = (process.env.VUE_APP_COMMIT_HASH || "").substring(0, 12);
			const branch = process.env.VUE_APP_COMMIT_BRANCH;
			{
				const date = new Date(process.env.VUE_APP_COMMIT_DATE || "");
				const dateString = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
				const timeString = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
				const timezoneName = Intl.DateTimeFormat(undefined, { timeZoneName: "long" })
					.formatToParts(new Date())
					.find((part) => part.type === "timeZoneName");
				const timezoneNameString = timezoneName?.value;
				timestamp = `${dateString} ${timeString} ${timezoneNameString}`;
			}

			this.editor.instance.populate_about_graphite(release || "", timestamp, hash, branch || "");
		};

		// TODO(mfish33): Replace with initialization system Issue:#524
		// Get initial Document Bar
		this.editor.instance.init_document_bar();
		setLoadDefaultFontCallback((font: string, data: Uint8Array) => this.editor.instance.on_font_load(font, data, true));
		loadAboutGraphite();
	},
	data() {
		const documentModeEntries: SectionsOfMenuListEntries = [
			[
				{ label: "Design Mode", icon: "ViewportDesignMode" },
				{ label: "Select Mode", icon: "ViewportSelectMode", action: (): void => this.dialog.comingSoon(330) },
				{ label: "Guide Mode", icon: "ViewportGuideMode", action: (): void => this.dialog.comingSoon(331) },
			],
		];
		const viewModeEntries: RadioEntries = [
			{ value: "normal", icon: "ViewModeNormal", tooltip: "View Mode: Normal", action: (): void => this.setViewMode("Normal") },
			{ value: "outline", icon: "ViewModeOutline", tooltip: "View Mode: Outline", action: (): void => this.setViewMode("Outline") },
			{ value: "pixels", icon: "ViewModePixels", tooltip: "View Mode: Pixels", action: (): void => this.dialog.comingSoon(320) },
		];

		return {
			artworkSvg: "",
			artboardSvg: "",
			overlaysSvg: "",
			canvasSvgWidth: "100%",
			canvasSvgHeight: "100%",
			canvasCursor: "default",
			activeTool: "Select" as ToolName,
			toolOptionsLayout: defaultWidgetLayout(),
			documentBarLayout: defaultWidgetLayout(),
			documentModeEntries,
			viewModeEntries,
			documentModeSelectionIndex: 0,
			viewModeIndex: 0,
			snappingEnabled: true,
			gridEnabled: true,
			overlaysEnabled: true,
			documentRotation: 0,
			documentZoom: 100,
			scrollbarPos: { x: 0.5, y: 0.5 },
			scrollbarSize: { x: 0.5, y: 0.5 },
			scrollbarMultiplier: { x: 0, y: 0 },
			rulerOrigin: { x: 0, y: 0 },
			rulerSpacing: 100,
			rulerInterval: 100,
			textInput: undefined as undefined | HTMLDivElement,
		};
	},
	components: {
		LayoutRow,
		LayoutCol,
		SwatchPairInput,
		ShelfItemInput,
		Separator,
		PersistentScrollbar,
		CanvasRuler,
		IconButton,
		DropdownInput,
		WidgetLayout,
	},
});
</script>
