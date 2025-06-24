import {ref} from "vue";
import {invoke} from "@tauri-apps/api/core";

const fileServerAddress = ref("");

invoke<string>("get_file_server_address").then((address) => {
    fileServerAddress.value = address + "/";
});

export default function useFileServerAddress() {
    if (fileServerAddress.value === "") {
        invoke<string>("get_file_server_address").then((address) => {
            fileServerAddress.value = address + "/";
        });
    }

    return fileServerAddress;
}
