<script setup lang="ts">
import ChooseFile from "../components/ChooseFile.vue";
import useThreads from "../composables/threads.ts";
import useInput from "../composables/input.ts";
import Button from "primevue/button";
import {onMounted, ref, watch} from "vue";
import ImageExperimenter from "../components/ImageExperimenter.vue";
import UploadingVirus from "../components/UploadingVirus.vue";
import {useRouter} from "vue-router";
import {Route} from "../constants/routes.ts";

const threads = useThreads();
const input = useInput();
input.value = null;
const router = useRouter();

const imageExperimenterOpen = ref(false);

window.localStorage.removeItem('input');
watch(input, () => {
  if (input.value != null) {
    router.replace(Route.Processing);
  }
});

onMounted(() => {
  window.localStorage.removeItem('editHistory');
});

const isDevMode = import.meta.env.MODE === 'development';
</script>

<template>
  <div class="flex items-center justify-center w-[100svw] h-[100svh] overflow-hidden max-h-full p-3">
    <Button
        v-if="isDevMode"
        label="Image Experimenter" @click="imageExperimenterOpen = true" size="small" icon="pi pi-cog"
        severity="secondary" class="absolute top-4 left-4"
    />
    <UploadingVirus/>
    <ChooseFile v-model:threads="threads" @update:selected="input = $event"/>
    <ImageExperimenter v-if="isDevMode" v-model:open="imageExperimenterOpen"/>
  </div>
</template>
