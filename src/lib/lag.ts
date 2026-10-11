/** Text color for a lag value: color is reserved for lag that matters. */
export function lagClass(lag: number | null | undefined): string {
    if (lag === null || lag === undefined) return "text-muted-foreground";
    if (lag === 0) return "text-muted-foreground";
    if (lag < 1_000) return "";
    if (lag < 100_000) return "text-amber-700 dark:text-amber-400";
    return "text-red-700 dark:text-red-400 font-medium";
}

export const ACTIVE_STATES = new Set(["Stable", "PreparingRebalance", "CompletingRebalance"]);

export function formatLag(lag: number | null | undefined): string {
    return lag === null || lag === undefined ? "—" : lag.toLocaleString();
}
