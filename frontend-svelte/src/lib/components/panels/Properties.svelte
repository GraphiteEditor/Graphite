<script lang="ts">
	import { onMount } from "svelte";

	import { defaultWidgetLayout, patchWidgetLayout, UpdatePropertyPanelOptionsLayout, UpdatePropertyPanelSectionsLayout } from "$lib/wasm-communication/messages";

	import LayoutCol from "$lib/components/layout/LayoutCol.svelte";
	import LayoutRow from "$lib/components/layout/LayoutRow.svelte";
	import WidgetLayout from "$lib/components/widgets/WidgetLayout.svelte";

	// inject: ["editor", "dialog"],

	let propertiesOptionsLayout = defaultWidgetLayout();
	let propertiesSectionsLayout = defaultWidgetLayout();

	onMount(() => {
		editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelOptionsLayout, (updatePropertyPanelOptionsLayout) => {
			patchWidgetLayout(propertiesOptionsLayout, updatePropertyPanelOptionsLayout);
		});

		editor.subscriptions.subscribeJsMessage(UpdatePropertyPanelSectionsLayout, (updatePropertyPanelSectionsLayout) => {
			patchWidgetLayout(propertiesSectionsLayout, updatePropertyPanelSectionsLayout);
		});
	});
</script>

<LayoutCol class="properties">
	<LayoutRow class="options-bar">
		<WidgetLayout layout={propertiesOptionsLayout} />
	</LayoutRow>
	<LayoutRow class="sections" scrollableY={true}>
		<WidgetLayout layout={propertiesSectionsLayout} />
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
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
