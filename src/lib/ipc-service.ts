import { invoke } from "@tauri-apps/api/core";

export async function send_ipc<T>(
    command: string,
    payload?: Record<string, unknown> | undefined
): Promise<T> {
    try {
        const result = await invoke<T>(command, payload);

        return result;
    } catch (error) {
        console.error(`Error invoking command "${command}":`, error);
        throw error;
    }
}