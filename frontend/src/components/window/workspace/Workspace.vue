<template>
	<LayoutRow class="workspace" data-workspace>
		<LayoutRow class="workspace-grid-subdivision">
			<LayoutCol class="workspace-grid-subdivision">
				<LayoutRow class="workspace-grid-subdivision">
					<Panel
						:panelType="portfolio.state.documents.length > 0 ? 'Document' : undefined"
						:tabCloseButtons="true"
						:tabMinWidths="true"
						:tabLabels="portfolio.state.documents.map((doc) => doc.displayName)"
						:clickAction="(tabIndex) => editor.instance.selectDocument(portfolio.state.documents[tabIndex].id)"
						:closeAction="(tabIndex) => editor.instance.closeDocumentWithConfirmation(portfolio.state.documents[tabIndex].id)"
						:tabActiveIndex="portfolio.state.activeDocumentIndex"
						ref="documentsPanel"
					/>
				</LayoutRow>
				<LayoutRow class="workspace-grid-resize-gutter" @pointerdown="(e) => resizePanel(e)" v-if="nodeGraphVisible"></LayoutRow>
				<LayoutRow class="workspace-grid-subdivision" v-if="nodeGraphVisible">
					<Panel :panelType="'NodeGraph'" :tabLabels="['Node Graph']" :tabActiveIndex="0" />
				</LayoutRow>
			</LayoutCol>
			<LayoutCol class="workspace-grid-resize-gutter" @pointerdown="(e) => resizePanel(e)"></LayoutCol>
			<LayoutCol class="workspace-grid-subdivision" style="flex-grow: 0.17">
				<LayoutRow class="workspace-grid-subdivision" style="flex-grow: 402">
					<Panel :panelType="'Properties'" :tabLabels="['Properties']" :tabActiveIndex="0" />
				</LayoutRow>
				<LayoutRow class="workspace-grid-resize-gutter" @pointerdown="(e) => resizePanel(e)"></LayoutRow>
				<LayoutRow class="workspace-grid-subdivision" style="flex-grow: 590">
					<Panel :panelType="'LayerTree'" :tabLabels="['Layer Tree']" :tabActiveIndex="0" />
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

<script lang="ts">
import { defineComponent, nextTick } from "vue";

import DialogModal from "@/components/floating-menus/DialogModal.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import Panel from "@/components/window/workspace/Panel.vue";

const MIN_PANEL_SIZE = 100;

export default defineComponent({
	inject: ["workspace", "portfolio", "dialog", "editor"],
	components: {
		LayoutRow,
		LayoutCol,
		Panel,
		DialogModal,
	},
	computed: {
		activeDocumentIndex() {
			return this.portfolio.state.activeDocumentIndex;
		},
		nodeGraphVisible() {
			return this.workspace.state.nodeGraphVisible;
		},
	},
	methods: {
		resizePanel(event: PointerEvent) {
			const gutter = event.target as HTMLElement;
			const nextSibling = gutter.nextElementSibling as HTMLElement;
			const previousSibling = gutter.previousElementSibling as HTMLElement;

			// Are we resizing horizontally?
			const horizontal = gutter.classList.contains("layout-col");

			// Get the current size in px of the panels being resized
			const nextSiblingSize = horizontal ? nextSibling.getBoundingClientRect().width : nextSibling.getBoundingClientRect().height;
			const previousSiblingSize = horizontal ? previousSibling.getBoundingClientRect().width : previousSibling.getBoundingClientRect().height;

			// Prevent cursor flicker as mouse temporarily leaves the gutter
			gutter.setPointerCapture(event.pointerId);

			const mouseStart = horizontal ? event.clientX : event.clientY;

			function updatePosition(event: PointerEvent): void {
				const mouseCurrent = horizontal ? event.clientX : event.clientY;
				let mouseDelta = mouseStart - mouseCurrent;

				mouseDelta = Math.max(nextSiblingSize + mouseDelta, MIN_PANEL_SIZE) - nextSiblingSize;
				mouseDelta = previousSiblingSize - Math.max(previousSiblingSize - mouseDelta, MIN_PANEL_SIZE);

				nextSibling.style.flexGrow = (nextSiblingSize + mouseDelta).toString();
				previousSibling.style.flexGrow = (previousSiblingSize - mouseDelta).toString();

				window.dispatchEvent(new CustomEvent("resize"));
			}

			function cleanup(event: PointerEvent): void {
				gutter.releasePointerCapture(event.pointerId);

				document.removeEventListener("pointermove", updatePosition);
				document.removeEventListener("pointerleave", cleanup);
				document.removeEventListener("pointerup", cleanup);
			}

			document.addEventListener("pointermove", updatePosition);
			document.addEventListener("pointerleave", cleanup);
			document.addEventListener("pointerup", cleanup);
		},
	},
	watch: {
		async activeDocumentIndex(newIndex: number) {
			await nextTick();

			const documentsPanel = this.$refs.documentsPanel as typeof Panel;
			const newActiveTab = documentsPanel.$el.querySelectorAll("[data-tab-bar] [data-tab]")[newIndex];
			newActiveTab.scrollIntoView();
		},
	},
});
</script>
