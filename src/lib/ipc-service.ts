import { toast } from "svelte-sonner";
import type { AppError } from "./bindings";

export type { AppError };

/** Normalizes anything thrown by a command into an AppError. */
export function toAppError(error: unknown): AppError {
    if (error && typeof error === "object" && "kind" in error && "message" in error) {
        return error as AppError;
    }
    return { kind: "internal", message: String(error) };
}

export function toastError(error: unknown) {
    const { message, hint } = toAppError(error);
    toast.error(message, { description: hint ?? undefined });
}
