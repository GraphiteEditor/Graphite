<template>
	<LayoutCol class="properties">
		<LayoutRow class="options-bar">
			<WidgetLayout :layout="propertiesOptionsLayout" />
		</LayoutRow>
		<LayoutRow class="sections" :scrollableY="true">
			<WidgetLayout :layout="propertiesSectionsLayout" />
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.properties {
	height: 100%;

	.widget-layout {
		flex: 1 1 100%;
		margin: 0 4px;
	}

	.options-bar {
		height: 32px;
		flex: 0 0 auto;
	}

	.sections {
		flex: 1 1 100%;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { defaultWidgetLayout, patchWidgetLayout, UpdatePropertyPanelOptionsLayout, UpdatePropertyPanelSectionsLayout } from "@/wasm-communication/messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import WidgetLayout from "@/components/widgets/WidgetLayout.vue";

export default defineComponent({
	inject: ["editor", "dialog"],
	data() {
		return {
			propertiesOptionsLayout: defaultWidgetLayout(),
			propertiesSectionsLayout: defaultWidgetLayout(),
		};
	},
	mounted() {
		this.editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelOptionsLayout, (updatePropertyPanelOptionsLayout) => {
			patchWidgetLayout(this.propertiesOptionsLayout, updatePropertyPanelOptionsLayout);
		});

		this.editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelSectionsLayout, (updatePropertyPanelSectionsLayout) => {
			patchWidgetLayout(this.propertiesSectionsLayout, updatePropertyPanelSectionsLayout);
		});
	},
	components: {
		LayoutCol,
		LayoutRow,
		WidgetLayout,
	},
});
</script>
