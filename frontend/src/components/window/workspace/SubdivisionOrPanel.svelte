<script lang="ts">
	import type { DivisionOrPanel } from "@graphite/wasm-communication/messages";

	import Division from "@graphite/components/window/workspace/Division.svelte";
	import Panel from "@graphite/components/window/workspace/Panel.svelte";

	export let value: DivisionOrPanel;
</script>

{#if "Division" in value}
	<Division divisionData={value.Division} />
{:else}
	<Panel
		panelType={value.Panel.tabs[value.Panel.activeIndex]}
		panelIdentifier={value.Panel.identifier}
		tabLabels={value.Panel.tabs.map((name) => ({ name }))}
		tabActiveIndex={Number(value.Panel.activeIndex)}
	/>
{/if}
