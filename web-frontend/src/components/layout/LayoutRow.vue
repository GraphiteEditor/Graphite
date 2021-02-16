<template>
	<div :class="['layout-row']">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.layout-row {
	display: flex;
	flex-direction: row;
	flex-grow: 1;
}
</style>

<script lang="ts">
import { Options, Vue } from "vue-class-component";

@Options({
	components: {},
	props: {},
})
export default class LayoutRow extends Vue {}
</script>
