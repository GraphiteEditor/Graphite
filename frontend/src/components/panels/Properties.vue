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

		.widget-row > .icon-label:first-of-type {
			border-radius: 2px;
			background: var(--color-node-background);
			fill: var(--color-node-icon);
		}
	}

	.sections {
		flex: 1 1 100%;

		.widget-section + .widget-section {
			margin-top: 1px;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import { defaultWidgetLayout, UpdatePropertyPanelOptionsLayout, UpdatePropertyPanelSectionsLayout } from "@/wasm-communication/messages";

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
			this.propertiesOptionsLayout = updatePropertyPanelOptionsLayout;
		});

		this.editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelSectionsLayout, (updatePropertyPanelSectionsLayout) => {
			this.propertiesSectionsLayout = updatePropertyPanelSectionsLayout;
		});
	},
	components: {
		WidgetLayout,
		LayoutRow,
		LayoutCol,
	},
});
</script>
