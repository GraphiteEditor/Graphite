<template>
	<LayoutRow class="dockable-grid-subdivision">
		<LayoutCol class="dockable-grid-subdivision" style="flex-grow: 1597">
			<DockablePanel
				:panelType="'Document'"
				:tabCloseButtons="true"
				:tabMinWidths="true"
				:tabLabels="['X-35B Over Death Valley*', 'Document 2', 'Document 3', 'Document 4', 'Document 5']"
				:tabActiveIndex="0"
			/>
		</LayoutCol>
		<LayoutCol class="dockable-grid-resize-gutter"></LayoutCol>
		<LayoutCol class="dockable-grid-subdivision" style="flex-grow: 319">
			<LayoutRow class="dockable-grid-subdivision">
				<DockablePanel :panelType="'Properties'" :tabLabels="['Properties', 'Spreadsheet', 'Palettes']" :tabActiveIndex="0" />
			</LayoutRow>
			<LayoutRow class="dockable-grid-resize-gutter"></LayoutRow>
			<LayoutRow class="dockable-grid-subdivision">
				<DockablePanel :panelType="'LayerTree'" :tabLabels="['Layer Tree']" :tabActiveIndex="0" />
			</LayoutRow>
			<LayoutRow class="dockable-grid-resize-gutter"></LayoutRow>
			<LayoutRow class="dockable-grid-subdivision" style="flex-grow: 0; height: 0">
				<DockablePanel :panelType="'Minimap'" :tabLabels="['Minimap', 'Asset Manager']" :tabActiveIndex="0" />
			</LayoutRow>
		</LayoutCol>
	</LayoutRow>
</template>

<style lang="scss">
.dockable-grid-subdivision {
	min-height: 28px;
}

.dockable-grid-resize-gutter {
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
import DockablePanel from "./DockablePanel.vue";

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		DockablePanel,
	},
});
</script>
