<script lang="ts">
import { defineComponent } from "vue";

import DialogModal from "@/components/floating-menus/DialogModal.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import Panel from "@/components/window/workspace/Panel.vue";

const MIN_PANEL_SIZE = 100;
const PANEL_SIZES = {
	/**/ root: 100,
	/*   ├── */ content: 80,
	/*   │      ├── */ document: 60,
	/*   │      └── */ graph: 40,
	/*   └── */ details: 20,
	/*          ├── */ properties: 45,
	/*          └── */ layers: 55,
};

export default defineComponent({
	inject: ["workspace", "portfolio", "dialog", "editor"],
	data() {
		return {
			panelSizes: PANEL_SIZES,
		};
	},
	computed: {
		activeDocumentIndex() {
			return this.portfolio.state.activeDocumentIndex;
		},
		nodeGraphVisible() {
			return this.workspace.state.nodeGraphVisible;
		},
		documentTabLabels() {
			return this.portfolio.state.documents.map((doc) => {
				const name = doc.displayName;

				if (!this.editor.instance.inDevelopmentMode()) return { name };

				const tooltip = `Document ID ${doc.id}`;
				return { name, tooltip };
			});
		},
	},
	methods: {
		resizePanel(event: PointerEvent) {
			const gutter = (event.target || undefined) as HTMLDivElement | undefined;
			const nextSibling = (gutter?.nextElementSibling || undefined) as HTMLDivElement | undefined;
			const prevSibling = (gutter?.previousElementSibling || undefined) as HTMLDivElement | undefined;
			const parentElement = (gutter?.parentElement || undefined) as HTMLDivElement | undefined;

			const nextSiblingName = (nextSibling?.getAttribute("data-subdivision-name") || undefined) as keyof typeof PANEL_SIZES;
			const prevSiblingName = (prevSibling?.getAttribute("data-subdivision-name") || undefined) as keyof typeof PANEL_SIZES;

			if (!gutter || !nextSibling || !prevSibling || !parentElement || !nextSiblingName || !prevSiblingName) return;

			// Are we resizing horizontally?
			const isHorizontal = gutter.getAttribute("data-gutter-horizontal") !== null;

			// Get the current size in px of the panels being resized and the gutter
			const gutterSize = isHorizontal ? gutter.getBoundingClientRect().width : gutter.getBoundingClientRect().height;
			const nextSiblingSize = isHorizontal ? nextSibling.getBoundingClientRect().width : nextSibling.getBoundingClientRect().height;
			const prevSiblingSize = isHorizontal ? prevSibling.getBoundingClientRect().width : prevSibling.getBoundingClientRect().height;
			const parentElementSize = isHorizontal ? parentElement.getBoundingClientRect().width : parentElement.getBoundingClientRect().height;

			// Measure the resizing panels as a percentage of all sibling panels
			const totalResizingSpaceOccupied = gutterSize + nextSiblingSize + prevSiblingSize;
			const proportionBeingResized = totalResizingSpaceOccupied / parentElementSize;

			// Prevent cursor flicker as mouse temporarily leaves the gutter
			gutter.setPointerCapture(event.pointerId);

			const mouseStart = isHorizontal ? event.clientX : event.clientY;

			const updatePosition = (event: PointerEvent): void => {
				const mouseCurrent = isHorizontal ? event.clientX : event.clientY;
				let mouseDelta = mouseStart - mouseCurrent;

				mouseDelta = Math.max(nextSiblingSize + mouseDelta, MIN_PANEL_SIZE) - nextSiblingSize;
				mouseDelta = prevSiblingSize - Math.max(prevSiblingSize - mouseDelta, MIN_PANEL_SIZE);

				this.panelSizes[nextSiblingName] = ((nextSiblingSize + mouseDelta) / totalResizingSpaceOccupied) * proportionBeingResized * 100;
				this.panelSizes[prevSiblingName] = ((prevSiblingSize - mouseDelta) / totalResizingSpaceOccupied) * proportionBeingResized * 100;

				window.dispatchEvent(new CustomEvent("resize"));
			};

			const cleanup = (event: PointerEvent): void => {
				gutter.releasePointerCapture(event.pointerId);

				document.removeEventListener("pointermove", updatePosition);
				document.removeEventListener("pointerleave", cleanup);
				document.removeEventListener("pointerup", cleanup);
			};

			document.addEventListener("pointermove", updatePosition);
			document.addEventListener("pointerleave", cleanup);
			document.addEventListener("pointerup", cleanup);
		},
	},
	watch: {
		async activeDocumentIndex(newIndex: number) {
			(this.$refs.documentPanel as typeof Panel | undefined)?.scrollTabIntoView(newIndex);
		},
	},
	components: {
		DialogModal,
		LayoutCol,
		LayoutRow,
		Panel,
	},
});
</script>

<template>
	<LayoutRow class="workspace" data-workspace>
		<LayoutRow class="workspace-grid-subdivision" :style="{ 'flex-grow': panelSizes['root'] }" data-subdivision-name="root">
			<LayoutCol class="workspace-grid-subdivision" :style="{ 'flex-grow': panelSizes['content'] }" data-subdivision-name="content">
				<LayoutRow class="workspace-grid-subdivision" :style="{ 'flex-grow': panelSizes['document'] }" data-subdivision-name="document">
					<Panel
						:panelType="portfolio.state.documents.length > 0 ? 'Document' : undefined"
						:tabCloseButtons="true"
						:tabMinWidths="true"
						:tabLabels="documentTabLabels"
						:clickAction="(tabIndex: number) => editor.instance.selectDocument(portfolio.state.documents[tabIndex].id)"
						:closeAction="(tabIndex: number) => editor.instance.closeDocumentWithConfirmation(portfolio.state.documents[tabIndex].id)"
						:tabActiveIndex="portfolio.state.activeDocumentIndex"
						ref="documentPanel"
					/>
				</LayoutRow>
				<LayoutRow class="workspace-grid-resize-gutter" data-gutter-vertical @pointerdown="(e: PointerEvent) => resizePanel(e)" v-if="nodeGraphVisible"></LayoutRow>
				<LayoutRow class="workspace-grid-subdivision" v-if="nodeGraphVisible" :style="{ 'flex-grow': panelSizes['graph'] }" data-subdivision-name="graph">
					<Panel :panelType="'NodeGraph'" :tabLabels="[{ name: 'Node Graph' }]" :tabActiveIndex="0" />
				</LayoutRow>
			</LayoutCol>
			<LayoutCol class="workspace-grid-resize-gutter" data-gutter-horizontal @pointerdown="(e: PointerEvent) => resizePanel(e)"></LayoutCol>
			<LayoutCol class="workspace-grid-subdivision" :style="{ 'flex-grow': panelSizes['details'] }" data-subdivision-name="details">
				<LayoutRow class="workspace-grid-subdivision" :style="{ 'flex-grow': panelSizes['properties'] }" data-subdivision-name="properties">
					<Panel :panelType="'Properties'" :tabLabels="[{ name: 'Properties' }]" :tabActiveIndex="0" />
				</LayoutRow>
				<LayoutRow class="workspace-grid-resize-gutter" data-gutter-vertical @pointerdown="(e: PointerEvent) => resizePanel(e)"></LayoutRow>
				<LayoutRow class="workspace-grid-subdivision" :style="{ 'flex-grow': panelSizes['layers'] }" data-subdivision-name="layers">
					<Panel :panelType="'LayerTree'" :tabLabels="[{ name: 'Layer Tree' }]" :tabActiveIndex="0" />
				</LayoutRow>
			</LayoutCol>
		</LayoutRow>
		<DialogModal v-if="dialog.state.visible" />
	</LayoutRow>
</template>

<style lang="scss">
.workspace {
	position: relative;
	flex: 1 1 100%;

	.workspace-grid-subdivision {
		min-height: 28px;
		flex: 1 1 0;

		&.folded {
			flex-grow: 0;
			height: 0;
		}
	}

	.workspace-grid-resize-gutter {
		flex: 0 0 4px;

		&.layout-row {
			cursor: ns-resize;
		}

		&.layout-col {
			cursor: ew-resize;
		}
	}
}
</style>
