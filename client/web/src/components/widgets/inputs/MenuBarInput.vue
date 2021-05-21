<template>
	<div class="menu-bar-input">
		<div class="entry"><GraphiteLogo width="16" height="16" /></div>
		<div class="entry"><span>File</span></div>
		<div class="entry"><span>Edit</span></div>
		<div class="entry"><span>Document</span></div>
		<div class="entry"><span>View</span></div>
		<div class="entry"><span>Help</span></div>
	</div>
</template>

<style lang="scss">
.menu-bar-input {
	display: flex;

	.entry {
		display: flex;
		align-items: center;
		white-space: nowrap;
		padding: 0 8px;

		svg {
			fill: #ddd;
		}

		&:hover {
			background: #666;

			svg {
				fill: #fff;
			}

			span {
				color: #fff;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import { ApplicationPlatform } from "../../window/MainWindow.vue";
import GraphiteLogo from "../../../../assets/svg/16x16-bounds-16x16-icon/graphite-logo.svg";

export default defineComponent({
	components: { GraphiteLogo },
	data() {
		return {
			ApplicationPlatform,
		};
	},
});
</script>
