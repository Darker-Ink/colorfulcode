<template>
  <div class="todo" :class="{ done: item.done }">
    <input v-model="draft" @keyup.enter="add" placeholder="What next?" />
    <ul>
      <li v-for="item in items" :key="item.id" @click="toggle(item)">
        {{ item.text.toUpperCase() }}
      </li>
    </ul>
    <TodoFooter :count="remaining" />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";

interface Item { id: number; text: string; done: boolean }
const items = ref<Item[]>([]);
const draft = ref("");
const remaining = computed(() => items.value.filter((i) => !i.done).length);
function add() {
  items.value.push({ id: Date.now(), text: draft.value, done: false });
  draft.value = "";
}
</script>

<style scoped lang="scss">
$gap: 8px;
.todo { display: grid; gap: $gap; }
</style>
