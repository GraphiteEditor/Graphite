<template>
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
		<LayoutCol class="workspace-grid-resize-gutter" @mousedown="resizePanel($event)"></LayoutCol>
		<LayoutCol class="workspace-grid-subdivision" style="flex-grow: 0.17">
			<LayoutRow class="workspace-grid-subdivision" style="flex-grow: 402">
				<Panel :panelType="'Properties'" :tabLabels="['Properties']" :tabActiveIndex="0" />
			</LayoutRow>
			<LayoutRow class="workspace-grid-resize-gutter" @mousedown="resizePanel($event)"></LayoutRow>
			<LayoutRow class="workspace-grid-subdivision" style="flex-grow: 590">
				<Panel :panelType="'LayerTree'" :tabLabels="['Layer Tree']" :tabActiveIndex="0" />
			</LayoutRow>
			<!-- <LayoutRow class="workspace-grid-resize-gutter"></LayoutRow>
			<LayoutRow class="workspace-grid-subdivision folded">
				<Panel :panelType="'Minimap'" :tabLabels="['Minimap', 'Asset Manager']" :tabActiveIndex="0" />
			</LayoutRow> -->
		</LayoutCol>
	</LayoutRow>
	<DialogModal v-if="dialog.state.visible" />
</template>

<style lang="scss">
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
</style>

<script lang="ts">
import { defineComponent, unref } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import DialogModal from "@/components/widgets/floating-menus/DialogModal.vue";
import Panel from "@/components/workspace/Panel.vue";

export default defineComponent({
	inject: ["documents", "dialog", "editor", "inputManager"],
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
		resizePanel(event: MouseEvent) {
			const gutter = event.target as HTMLElement;
			const nextSibling = gutter.nextElementSibling as HTMLElement;
			const previousSibling = gutter.previousElementSibling as HTMLElement;
			const parent = gutter.parentElement as HTMLElement;

			// Are we resizing horizontally?
			const horizontal = parent.classList.contains("layout-row");

			// Get the current size in px of the panels being resized
			const nextSiblingSize = horizontal ? nextSibling.getBoundingClientRect().width : nextSibling.getBoundingClientRect().height;
			const previousSiblingSize = horizontal ? previousSibling.getBoundingClientRect().width : previousSibling.getBoundingClientRect().height;

			// Prevent cursor flicker as mouse temporarily leaves the gutter
			document.body.style.cursor = horizontal ? "ew-resize" : "ns-resize";

			const mouseStart = horizontal ? event.clientX : event.clientY;

			const inputManager = unref(this.inputManager);

			function updatePosition(event: MouseEvent) {
				const mouseCurrent = horizontal ? event.clientX : event.clientY;
				const mouseDelta = mouseStart - mouseCurrent;

				nextSibling.style.flexGrow = (nextSiblingSize + mouseDelta).toString();
				previousSibling.style.flexGrow = (previousSiblingSize - mouseDelta).toString();

				if (inputManager) {
					inputManager.onWindowResize(inputManager.container);
				}
			}

			document.addEventListener("mousemove", updatePosition);

			function cleanup() {
				document.body.style.cursor = "inherit";
				document.removeEventListener("mousemove", updatePosition);
				document.removeEventListener("mouseleave", cleanup);
				document.removeEventListener("mouseup", cleanup);
			}

			document.addEventListener("mouseleave", cleanup);
			document.addEventListener("mouseup", cleanup);
		},
	},
	watch: {
		activeDocumentIndex(newIndex: number) {
			this.$nextTick(() => {
				const documentsPanel = this.$refs.documentsPanel as typeof Panel;
				const newActiveTab = documentsPanel.$el.querySelectorAll(".tab-bar .tab-group .tab")[newIndex];
				newActiveTab.scrollIntoView();
			});
		},
	},
});
</script>
