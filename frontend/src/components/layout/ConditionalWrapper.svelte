<script lang="ts">
	export let condition: boolean;
	export let wrapperClass = "";
	export let wrapperStyle: string | undefined = undefined;
</script>

{#if condition}
	<div class={wrapperClass} style={wrapperStyle}>
		<slot />
	</div>
{:else}
	<slot />
{/if}

<style lang="scss"></style>
