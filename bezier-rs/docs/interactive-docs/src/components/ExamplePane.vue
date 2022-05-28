<template>
  <div>
    <div v-for="example in examples" :key="example.id">
      <Example :title="example.title" />
    </div>
  </div>
</template>

<script>

import Example from './Example.vue'

export default {
  name: 'ExamplePane',
  components: {
    Example
  },
  data() {
    return {
      examples: [
        {
          id: 0,
          title: "Straight Line"
        },
        {
          id: 1,
          title: "Other Straight Line"
        }
      ]
    }
  }
}
</script>
