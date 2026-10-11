/** True on macOS, where shortcuts use ⌘ instead of Ctrl. */
export const isMac: boolean =
    typeof navigator !== "undefined" &&
    /mac/i.test(
        (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
            navigator.platform ??
            "",
    );

/** Label for the primary modifier key: "⌘" on macOS, "Ctrl" elsewhere. */
export const modKey = isMac ? "⌘" : "Ctrl";

/** Formats a shortcut like `shortcut("K")` → "⌘K" or "Ctrl+K". */
export function shortcut(key: string): string {
    return isMac ? `⌘${key}` : `Ctrl+${key}`;
}

/** Whether a keyboard event used the platform's primary modifier. */
export function hasModifier(event: KeyboardEvent): boolean {
    return isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}
