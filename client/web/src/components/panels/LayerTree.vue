<template>
	<div></div>
</template>

<style lang="scss"></style>

<script lang="ts">
import { defineComponent } from "vue";
import { ResponseType, registerResponseHandler } from "../../response-handler";

export default defineComponent({
	components: {},
	props: {},
	methods: {},
	mounted() {
		registerResponseHandler(ResponseType["Document::ExpandFolder"], (responseData) => {
			console.log("ExpandFolder: ", responseData);
		});
		registerResponseHandler(ResponseType["Document::CollapseFolder"], (responseData) => {
			console.log("CollapseFolder: ", responseData);
		});
	},
	data() {
		return {};
	},
});
</script>
