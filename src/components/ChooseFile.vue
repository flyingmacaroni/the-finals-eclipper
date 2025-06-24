<script setup lang="ts">
import {open} from '@tauri-apps/plugin-dialog';
import Button from 'primevue/button';
import InputNumber from "primevue/inputnumber";
import {onMounted, ref} from "vue";
import useProcessVideo from "../composables/processVideo.ts";
import Checkbox from "primevue/checkbox";
import {invoke} from "@tauri-apps/api/core";

defineProps<{
  threads: number,
}>();

const emit = defineEmits<{
  (e: 'update:selected', path: string | null): void;
  (e: 'update:threads', threads: number): void;
}>();

const max_threads = ref(1);
const {state} = useProcessVideo();

onMounted(() => {
  invoke<number>('max_thread_count').then((v) => max_threads.value = v * 2);
});

async function selectRecording() {
// Open a selection dialog for image files
  const selected = await open({
    multiple: false,
    filters: [{
      name: 'Video',
      extensions: ['mp4', 'mkv', 'webm']
    }]
  });

  emit('update:selected', selected as string | null);
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex gap-4 items-center">
      <div class="flex items-center">
        <Checkbox v-model="state.includeAssists" inputId="includeAssists" name="includeAssists" binary/>
        <label for="includeAssists" class="ml-2"> Include Assists </label>
      </div>
      <div class="flex items-center">
        <Checkbox v-model="state.includeSpectating" inputId="includeSpectating" name="includeSpectating" binary/>
        <label for="includeSpectating" class="ml-2"> Include Spectating </label>
      </div>
    </div>
    <div>
      <div class="flex items-center gap-2">
        <label for="elimClipDuration" class="ml-2"> Elimination Clip Duration </label>
        <InputNumber v-model="state.elimClipDuration" inputId="elimClipDuration" name="elimClipDuration"/>
      </div>
    </div>
    <label for="threads">How much do you want to torture your PC? Lower number = less torture.</label>
    <InputNumber :model-value="threads" @update:model-value="emit('update:threads', $event)" inputId="threads"
                 :max="max_threads" :min="1" show-buttons buttonLayout="horizontal">
      <template #incrementbuttonicon>
        <span class="pi pi-plus"/>
      </template>
      <template #decrementbuttonicon>
        <span class="pi pi-minus"/>
      </template>
    </InputNumber>
    <div>
      <div class="flex items-center">
        <Checkbox v-model="state.hwAccel" inputId="hwAccel" name="hwAccel" binary/>
        <label for="hwAccel" class="ml-2"> Hardware Video Acceleration </label>
      </div>
      <small class="text-gray-400">May not speed up the process in all cases but it can reduce CPU load.</small>
    </div>
    <div class="flex flex-col gap-1">
      <Button @click="selectRecording" label="Button" raised/>
      <small class="text-gray-400 max-w-lg">
        Push this button to select a video recording of THE FINALS. Processing will
        begin immediately after selecting a video file.
      </small>
    </div>
  </div>
</template>
