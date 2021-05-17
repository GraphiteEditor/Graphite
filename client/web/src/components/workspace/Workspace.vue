<template>
	<LayoutRow class="workspace-grid-subdivision">
		<LayoutCol class="workspace-grid-subdivision" style="flex-grow: 1597">
			<WorkspacePanel
				:panelType="'Document'"
				:tabCloseButtons="true"
				:tabMinWidths="true"
				:tabLabels="['X-35B Over Death Valley*', 'Document 2', 'Document 3', 'Document 4', 'Document 5']"
				:tabActiveIndex="0"
			/>
		</LayoutCol>
		<LayoutCol class="workspace-grid-resize-gutter"></LayoutCol>
		<LayoutCol class="workspace-grid-subdivision" style="flex-grow: 319">
			<LayoutRow class="workspace-grid-subdivision">
				<WorkspacePanel :panelType="'Properties'" :tabLabels="['Properties', 'Spreadsheet', 'Palettes']" :tabActiveIndex="0" />
			</LayoutRow>
			<LayoutRow class="workspace-grid-resize-gutter"></LayoutRow>
			<LayoutRow class="workspace-grid-subdivision">
				<WorkspacePanel :panelType="'LayerTree'" :tabLabels="['Layer Tree']" :tabActiveIndex="0" />
			</LayoutRow>
			<LayoutRow class="workspace-grid-resize-gutter"></LayoutRow>
			<LayoutRow class="workspace-grid-subdivision" style="flex-grow: 0; height: 0">
				<WorkspacePanel :panelType="'Minimap'" :tabLabels="['Minimap', 'Asset Manager']" :tabActiveIndex="0" />
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
</template>

<style lang="scss">
.workspace-grid-subdivision {
	min-height: 28px;
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
import { defineComponent } from "vue";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import WorkspacePanel from "./WorkspacePanel.vue";

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		WorkspacePanel,
	},
});
</script>
