<template>
	<LayoutRow class="workspace" data-workspace>
		<LayoutRow class="workspace-grid-subdivision">
			<LayoutCol class="workspace-grid-subdivision">
				<Panel
					:panelType="'Document'"
					:tabCloseButtons="true"
					:tabMinWidths="true"
					:tabLabels="documents.state.documents.map((doc) => doc.displayName)"
					:clickAction="
						(tabIndex) => {
							const targetId = documents.state.documents[tabIndex].id;
							editor.instance.select_document(targetId);
						}
					"
					:closeAction="
						(tabIndex) => {
							const targetId = documents.state.documents[tabIndex].id;
							editor.instance.close_document_with_confirmation(targetId);
						}
					"
					:tabActiveIndex="documents.state.activeDocumentIndex"
					ref="documentsPanel"
				/>
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
import { defineComponent } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import DialogModal from "@/components/widgets/floating-menus/DialogModal.vue";
import Panel from "@/components/workspace/Panel.vue";

const MIN_PANEL_SIZE = 100;

export default defineComponent({
	inject: ["documents", "dialog", "editor"],
	components: {
		LayoutRow,
		LayoutCol,
		Panel,
		DialogModal,
	},
	computed: {
		activeDocumentIndex() {
			return this.documents.state.activeDocumentIndex;
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

				window.dispatchEvent(
					new CustomEvent("resize", {
						detail: {},
					})
				);
			}

			document.addEventListener("pointermove", updatePosition);

			function cleanup(event: PointerEvent): void {
				gutter.releasePointerCapture(event.pointerId);
				document.removeEventListener("pointermove", updatePosition);
				document.removeEventListener("pointerleave", cleanup);
				document.removeEventListener("pointerup", cleanup);
			}

			document.addEventListener("pointerleave", cleanup);
			document.addEventListener("pointerup", cleanup);
		},
	},
	watch: {
		activeDocumentIndex(newIndex: number) {
			this.$nextTick(() => {
				const documentsPanel = this.$refs.documentsPanel as typeof Panel;
				const newActiveTab = documentsPanel.$el.querySelectorAll("[data-tab-bar] [data-tab]")[newIndex];
				newActiveTab.scrollIntoView();
			});
		},
	},
});
</script>
