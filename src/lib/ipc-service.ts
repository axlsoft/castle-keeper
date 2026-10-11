import { invoke } from "@tauri-apps/api/core";
import { toast } from "svelte-sonner";

/** Mirrors `AppError` in src-tauri/src/error.rs. */
export interface AppError {
    kind: string;
    message: string;
    hint?: string;
}

export function toAppError(error: unknown): AppError {
    if (error && typeof error === "object" && "kind" in error && "message" in error) {
        return error as AppError;
    }
    return { kind: "internal", message: String(error) };
}

export function toastError(error: unknown) {
    const { message, hint } = toAppError(error);
    toast.error(message, { description: hint });
}

export async function send_ipc<T>(
    command: string,
    payload?: Record<string, unknown> | undefined
): Promise<T> {
    try {
        return await invoke<T>(command, payload);
    } catch (error) {
        console.error(`Error invoking command "${command}":`, error);
        throw toAppError(error);
    }
}
