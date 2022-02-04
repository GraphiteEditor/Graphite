<template>
	<WidgetLayout class="properties-panel" :layout="propertiesLayout"></WidgetLayout>
</template>

<style lang="scss"></style>

<script lang="ts">
import { defineComponent } from "vue";

import { defaultWidgetLayout, UpdatePropertyPanelLayout } from "@/dispatcher/js-messages";

import WidgetLayout from "@/components/widgets/WidgetLayout.vue";

export default defineComponent({
	inject: ["editor", "dialog"],
	data() {
		return {
			propertiesLayout: defaultWidgetLayout(),
		};
	},
	mounted() {
		this.editor.dispatcher.subscribeJsMessage(UpdatePropertyPanelLayout, (updatePropertyPanelLayout) => {
			this.propertiesLayout = updatePropertyPanelLayout;
		});
	},
	components: { WidgetLayout },
});
</script>
