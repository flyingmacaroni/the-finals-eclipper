<script setup lang="ts">
import useProcessVideo from "../composables/processVideo.ts";
import {onMounted, ref, watch} from "vue";
import ProcessingProgress from "../components/ProcessingProgress.vue";
import LogModal from "../components/LogModal.vue";
import Button from "primevue/button";
import {useRouter} from "vue-router";
import {Route} from "../constants/routes.ts";
import FramePreview from "../components/FramePreview.vue";

const {state, start} = useProcessVideo();
const logModalOpen = ref(false);
const router = useRouter();

onMounted(() => {
  if (!state.processing) {
    start();
  }
});

watch(() => state.processing, (processing) => {
  if (!processing) {
    router.replace(Route.EditClipsPage);
  }
});

</script>

<template>
  <div class="h-screen w-screen flex flex-col gap-3 p-3">
    <div>
      <Button @click="logModalOpen = true" severity="secondary" label="Logs" size="small" icon="pi pi-align-left"/>
    </div>
    <FramePreview class="grow"/>
    <ProcessingProgress :progress="state.progress" :speed="state.speed"/>
    <div
        v-if="!state.processing"
        class="text-green-500 absolute z-10 inset-0 flex items-center justify-center bg-black bg-opacity-75"
    >
      Processing video finished!
    </div>
    <LogModal v-model:open="logModalOpen"/>
  </div>
</template>
