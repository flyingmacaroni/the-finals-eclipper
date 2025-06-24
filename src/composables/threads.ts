import {ref} from "vue";
import {invoke} from "@tauri-apps/api/core";

const threads = ref(1);

invoke<number>('max_thread_count').then((max_threads) => threads.value = max_threads);

export default function useThreads() {
    return threads
}
