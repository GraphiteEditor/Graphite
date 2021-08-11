<template>
	<LayoutRow class="workspace-grid-subdivision">
		<LayoutCol class="workspace-grid-subdivision" style="flex-grow: 1597">
			<Panel :panelType="'Document'" :tabCloseButtons="true" :tabMinWidths="true" :tabLabels="documents.documents" :tabActiveIndex="documents.activeDocumentIndex" />
		</LayoutCol>
		<LayoutCol class="workspace-grid-resize-gutter"></LayoutCol>
		<LayoutCol class="workspace-grid-subdivision" style="flex-grow: 319">
			<LayoutRow class="workspace-grid-subdivision" style="flex-grow: 402">
				<Panel :panelType="'Properties'" :tabLabels="['Properties', 'Spreadsheet', 'Palettes']" :tabActiveIndex="0" />
			</LayoutRow>
			<LayoutRow class="workspace-grid-resize-gutter"></LayoutRow>
			<LayoutRow class="workspace-grid-subdivision" style="flex-grow: 590">
				<Panel :panelType="'LayerTree'" :tabLabels="['Layer Tree']" :tabActiveIndex="0" />
			</LayoutRow>
			<LayoutRow class="workspace-grid-resize-gutter"></LayoutRow>
			<LayoutRow class="workspace-grid-subdivision folded">
				<Panel :panelType="'Minimap'" :tabLabels="['Minimap', 'Asset Manager']" :tabActiveIndex="0" />
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
	<DialogModal v-if="dialog.visible" />
</template>

<style lang="scss">
.workspace-grid-subdivision {
	min-height: 28px;
	min-width: 0;
	flex: 1 1 0;

	&.folded {
		flex-grow: 0;
		height: 0;
	}
}

.workspace-grid-resize-gutter {
	flex: 0 0 auto;

	&.layout-row {
		height: 4px;
		cursor: ns-resize;
	}

	&.layout-col {
		width: 4px;
		cursor: ew-resize;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import Panel from "@/components/workspace/Panel.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import LayoutCol from "@/components/layout/LayoutCol.vue";
import DialogModal from "@/components/widgets/floating-menus/DialogModal.vue";

export default defineComponent({
	inject: ["documents", "dialog"],
	components: {
		LayoutRow,
		LayoutCol,
		Panel,
		DialogModal,
	},
	data() {
		return {};
	},
});
</script>
