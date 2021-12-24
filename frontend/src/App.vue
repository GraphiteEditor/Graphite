<template>
	<MainWindow />
	<div class="unsupported-modal-backdrop" v-if="showUnsupportedModal">
		<div class="unsupported-modal">
			<h2>Your browser currently doesn't support Graphite</h2>
			<p>Unfortunately, some features won't work properly. Please upgrade to a modern browser such as Firefox, Chrome, Edge, or Safari version 15 or later.</p>
			<p>
				Your browser is missing support for the
				<a href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt64Array#browser_compatibility" target="_blank"><code>BigInt64Array</code></a> JavaScript
				API which is required for using the editor. However, you can still explore the user interface.
			</p>
			<LayoutRow>
				<button class="unsupported-modal-button" @click="closeModal()">I understand, let's just see the interface</button>
			</LayoutRow>
		</div>
	</div>
</template>

<style lang="scss">
:root {
	// Replace usage of `-rgb` variants with CSS color() function to calculate alpha when browsers support it
	// See https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/color() and https://caniuse.com/css-color-function
	--color-0-black: #000;
	--color-0-black-rgb: 0, 0, 0;
	--color-1-nearblack: #111;
	--color-1-nearblack-rgb: 17, 17, 17;
	--color-2-mildblack: #222;
	--color-2-mildblack-rgb: 34, 34, 34;
	--color-3-darkgray: #333;
	--color-3-darkgray-rgb: 51, 51, 51;
	--color-4-dimgray: #444;
	--color-4-dimgray-rgb: 68, 68, 68;
	--color-5-dullgray: #555;
	--color-5-dullgray-rgb: 85, 85, 85;
	--color-6-lowergray: #666;
	--color-6-lowergray-rgb: 102, 102, 102;
	--color-7-middlegray: #777;
	--color-7-middlegray-rgb: 109, 109, 109;
	--color-8-uppergray: #888;
	--color-8-uppergray-rgb: 136, 136, 136;
	--color-9-palegray: #999;
	--color-9-palegray-rgb: 153, 153, 153;
	--color-a-softgray: #aaa;
	--color-a-softgray-rgb: 170, 170, 170;
	--color-b-lightgray: #bbb;
	--color-b-lightgray-rgb: 187, 187, 187;
	--color-c-brightgray: #ccc;
	--color-c-brightgray-rgb: 204, 204, 204;
	--color-d-mildwhite: #ddd;
	--color-d-mildwhite-rgb: 221, 221, 221;
	--color-e-nearwhite: #eee;
	--color-e-nearwhite-rgb: 238, 238, 238;
	--color-f-white: #fff;
	--color-f-white-rgb: 255, 255, 255;
	--color-accent: #3194d6;
	--color-accent-rgb: 49, 148, 214;
	--color-accent-hover: #49a5e2;
	--color-accent-hover-rgb: 73, 165, 226;
	--color-accent-disabled: #416277;
	--color-accent-disabled-rgb: 65, 98, 119;

	--color-data-raster: #e4bb72;
	--color-data-raster-rgb: 228, 187, 114;
}

html,
body,
#app {
	margin: 0;
	height: 100%;
	background: var(--color-2-mildblack);
	user-select: none;
	overscroll-behavior: none;
	outline: none;
}

html,
body,
input,
textarea,
button {
	font-family: "Source Sans Pro", Arial, sans-serif;
	font-weight: 400;
	font-size: 14px;
	line-height: 1;
	color: var(--color-e-nearwhite);
}

svg,
img {
	display: block;
}

.scrollable,
.scrollable-x,
.scrollable-y {
	// Standard
	scrollbar-width: thin;
	scrollbar-width: 6px;
	scrollbar-gutter: 6px;
	scrollbar-color: var(--color-5-dullgray) transparent;

	&:not(:hover) {
		scrollbar-width: none;
	}

	// WebKit
	&::-webkit-scrollbar {
		width: calc(2px + 6px + 2px);
		height: calc(2px + 6px + 2px);
	}

	&:not(:hover)::-webkit-scrollbar {
		width: 0;
		height: 0;
	}

	&::-webkit-scrollbar-track {
		box-shadow: inset 0 0 0 1px var(--color-5-dullgray);
		border: 2px solid transparent;
		border-radius: 10px;

		&:hover {
			box-shadow: inset 0 0 0 1px var(--color-6-lowergray);
		}
	}

	&::-webkit-scrollbar-thumb {
		background-clip: padding-box;
		background-color: var(--color-5-dullgray);
		border: 2px solid transparent;
		border-radius: 10px;
		margin: 2px;

		&:hover {
			background-color: var(--color-6-lowergray);
		}
	}
}

.scrollable {
	// Standard
	overflow: auto;
	// WebKit
	overflow: overlay;
}

.scrollable-x {
	// Standard
	overflow-x: auto;
	// WebKit
	overflow-x: overlay;
}

.scrollable-y {
	// Standard
	overflow-y: auto;
	// WebKit
	overflow-y: overlay;
}

// For placeholder messages (remove eventually)
.floating-menu {
	h1,
	h2,
	h3,
	h4,
	h5,
	h6,
	p {
		margin: 0;
	}

	p {
		margin-top: 8px;
	}
}

.unsupported-modal-backdrop {
	background: rgba(255, 255, 255, 0.6);
	position: absolute;
	top: 0;
	left: 0;
	bottom: 0;
	right: 0;
	align-items: center;
	justify-content: center;
	display: flex;
}
.unsupported-modal {
	background: var(--color-3-darkgray);
	border-radius: 4px;
	box-shadow: 2px 2px 5px 0 rgba(var(--color-0-black-rgb), 50%);
	padding: 0 16px 16px 16px;
	border: 1px solid var(--color-4-dimgray);
	max-width: 500px;

	& a {
		color: var(--color-accent-hover);
	}
}
.unsupported-modal-button {
	flex: 1;
	background: var(--color-1-nearblack);
	border: 0 none;
	padding: 12px;
	border-radius: 2px;

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);
	}

	&:active {
		background: var(--color-accent-hover);
		color: var(--color-f-white);
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { DialogState, createDialogState } from "@/state/dialog";
import { createDocumentsState, DocumentsState } from "@/state/documents";
import { createFullscreenState, FullscreenState } from "@/state/fullscreen";

import MainWindow from "@/components/window/MainWindow.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import { createEditorState, EditorState } from "@/state/wasm-loader";
import { createInputManager, InputManager } from "@/lifetime/input";
import { initErrorHandling } from "@/lifetime/errors";
import { AutoSaveState, createAutoSaveState } from "@/state/auto-save";

// Vue injects don't play well with TypeScript, and all injects will show up as `any`. As a workaround, we can define these types.
declare module "@vue/runtime-core" {
	interface ComponentCustomProperties {
		dialog: DialogState;
		documents: DocumentsState;
		fullscreen: FullscreenState;
		editor: EditorState;
		autoSave: AutoSaveState;
		// This must be set to optional because there is a time in the lifecycle of the component where inputManager is undefined.
		// That's because we initialize inputManager in `mounted()` rather than `data()` since the div hasn't been created yet.
		inputManger?: InputManager;
	}
}

export default defineComponent({
	provide() {
		return {
			editor: this.editor,
			dialog: this.dialog,
			documents: this.documents,
			fullscreen: this.fullscreen,
		};
	},
	data() {
		const editor = createEditorState();
		const dialog = createDialogState(editor);
		const autoSave = createAutoSaveState(editor);
		const documents = createDocumentsState(editor, dialog, autoSave);
		const fullscreen = createFullscreenState();
		initErrorHandling(editor, dialog);

		return {
			editor,
			dialog,
			autoSave,
			documents,
			fullscreen,
			showUnsupportedModal: !("BigInt64Array" in window),
			inputManager: undefined as undefined | InputManager,
		};
	},
	methods: {
		closeModal() {
			this.showUnsupportedModal = false;
		},
	},
	mounted() {
		this.inputManager = createInputManager(this.editor, this.$el.parentElement, this.dialog, this.documents, this.fullscreen);
	},
	beforeUnmount() {
		const { inputManager } = this;
		if (inputManager) inputManager.removeListeners();

		const { editor } = this;
		editor.instance.free();
	},
	components: { MainWindow, LayoutRow },
});
</script>
