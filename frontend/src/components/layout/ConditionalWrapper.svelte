<script lang="ts">
	export let condition: boolean;
	export let wrapperClass = "";
</script>

{#if condition}
	<div class={wrapperClass}>
		<slot />
	</div>
{:else}
	<slot />
{/if}

<style lang="scss" global></style>
