/**
 * Resolves once no dialog is animating closed. Opening a modal while another
 * is mid-exit leaves bits-ui's body lock in place (page dimmed, pointer
 * events off), so callers that open a modal right after another closed
 * should await this first.
 */
export async function modalsSettled(maxWaitMs = 1000): Promise<void> {
    const started = performance.now();
    while (
        document.querySelector('[data-dialog-overlay][data-state="closed"]') &&
        performance.now() - started < maxWaitMs
    ) {
        await new Promise((resolve) => requestAnimationFrame(resolve));
    }
}
