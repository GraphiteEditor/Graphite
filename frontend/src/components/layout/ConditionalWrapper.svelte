<script lang="ts">
	import type { Snippet } from "svelte";

	type Props = {
		condition: boolean;
		wrapperClass?: string;
		children?: Snippet;
	};

	let { condition, wrapperClass = "", children }: Props = $props();
</script>

{#if condition}
	<div class={wrapperClass}>
		{@render children?.()}
	</div>
{:else}
	{@render children?.()}
{/if}

<style lang="scss" global></style>
