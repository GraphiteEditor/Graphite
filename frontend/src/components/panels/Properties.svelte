<script lang="ts">
	import { getContext, onMount, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import WidgetLayout from "/src/components/widgets/WidgetLayout.svelte";
	import type { SubscriptionsRouter } from "/src/subscriptions-router";
	import { patchLayout } from "/src/utility-functions/widgets";
	import type { Layout } from "/wrapper/pkg/graphite_wasm_wrapper";

	const subscriptions = getContext<SubscriptionsRouter>("subscriptions");

	let propertiesPanelLayout: Layout = [];

	onMount(() => {
		subscriptions.subscribeLayoutUpdate("PropertiesPanel", (data) => {
			patchLayout(propertiesPanelLayout, data);
			propertiesPanelLayout = propertiesPanelLayout;
		});
	});

	onDestroy(() => {
		subscriptions.unsubscribeLayoutUpdate("PropertiesPanel");
	});
</script>

<LayoutCol class="properties">
	<LayoutCol class="sections" scrollableY={true}>
		<WidgetLayout layout={propertiesPanelLayout} layoutTarget="PropertiesPanel" />
	</LayoutCol>
</LayoutCol>

<style lang="scss" global>
	.properties {
		height: 100%;
		flex: 1 1 100%;

		.sections {
			flex: 1 1 100%;

			// Used as a placeholder for empty assist widgets
			.separator.section.horizontal {
				margin: 0;
				margin-left: 24px;

				div {
					width: 0;
				}
			}
		}

		.text-button {
			flex-basis: 0;
		}
	}
</style>
