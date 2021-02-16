<template>
	<div :class="['layout-col']">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.layout-col {
	display: flex;
	flex-direction: column;
	flex-grow: 1;
}
</style>

<script lang="ts">
import { Options, Vue } from "vue-class-component";

@Options({
	components: {},
	props: {},
})
export default class LayoutCol extends Vue {}
</script>
