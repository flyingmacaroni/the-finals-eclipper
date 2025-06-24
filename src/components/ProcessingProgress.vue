<script setup lang="ts">
import {computed} from "vue";

const props = defineProps<{ progress: number; speed: number }>();

const progressRounded = computed(() => props.progress.toLocaleString(undefined, {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1
}));
const speedRounded = computed(() => props.speed.toLocaleString(undefined, {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
}))
</script>

<template>
  <div class="bg-[#C5C5C5] w-full p-1 flex flex-col">
    <div class="bg-[#000085] text-white p-1 font-[500]">
      Processing...
    </div>
    <div class="flex flex-col text-black text-sm p-3 gap-3">
      <span>Processing video at <code>{{ speedRounded }}</code>x speed</span>
      <div
          class="bg-white border-2 border-solid border-t-black border-l-black border-b-gray-200 border-r-gray-200 h-6 relative">
        <div class="absolute font-mono z-0 text-black text-center top-0 h-full border border-solid border-opacity-0"
             :style="`width: ${progress}%`">
          {{ progressRounded }}%
        </div>
        <div
            class="absolute overflow-hidden h-full z-10 border border-solid border-t-gray-200 border-l-gray-200 border-r-black border-b-black text-center text-white font-mono"
            :style="`background-color: color-mix(in srgb, #F2020D ${progress}%, #5200AD); width: ${progress}%`">
          {{ progressRounded }}%
        </div>
      </div>
    </div>
  </div>
</template>
