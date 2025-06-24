import {ref} from "vue";
import {listen} from "@tauri-apps/api/event";

const previewFrame = ref<null | number>(null);

listen<number>('preview_frame', (event) => {
    previewFrame.value = event.payload;
}).then();

export default function usePreviewFrame() {
    return previewFrame;
}
