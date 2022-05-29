<template>
  <div>
    <h3>{{ title }}</h3>
    <figure ref="drawing"></figure>
  </div>
</template>

<script>

import { BezierDrawing } from './BezierDrawing.js'

export default {
  name: 'ExampleComponent',
  props: {
    title: String,
    points: Object,
  },
  mounted() {
    new BezierDrawing(this.$refs.drawing, this.points);
  }
}
</script>

<style scoped>
</style>
