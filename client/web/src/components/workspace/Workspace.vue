<template>
	<LayoutRow class="workspace-grid-subdivision">
		<LayoutCol class="workspace-grid-subdivision" style="flex-grow: 1597">
			<Panel
				:panelType="'Document'"
				:tabCloseButtons="true"
				:tabMinWidths="true"
				:tabLabels="['Untitled Document*', 'Document 2', 'Document 3', 'Document 4', 'Document 5', 'Document 6']"
				:tabActiveIndex="0"
			/>
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
import { defineComponent } from "vue";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import Panel from "./Panel.vue";

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		Panel,
	},
});
</script>
