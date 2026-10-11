import type { Environment } from "./bindings";

export interface EnvironmentStyle {
    value: Environment;
    label: string;
    /** Solid background for the env band/strip. */
    band: string;
    /** Small badge (text + background). */
    badge: string;
}

export const ENVIRONMENTS: EnvironmentStyle[] = [
    { value: "local", label: "Local", band: "bg-zinc-500", badge: "bg-zinc-500/15 text-zinc-600 dark:text-zinc-300" },
    { value: "dev", label: "Dev", band: "bg-blue-600", badge: "bg-blue-600/15 text-blue-700 dark:text-blue-300" },
    { value: "staging", label: "Staging", band: "bg-amber-500", badge: "bg-amber-500/15 text-amber-700 dark:text-amber-300" },
    { value: "prod", label: "Prod", band: "bg-red-600", badge: "bg-red-600/15 text-red-700 dark:text-red-300" },
];

export function environmentStyle(env: Environment): EnvironmentStyle {
    return ENVIRONMENTS.find((e) => e.value === env) ?? ENVIRONMENTS[0];
}
