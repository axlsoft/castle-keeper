/** Page-specific entries a page can contribute to the command palette. */
export interface PaletteItem {
    id: string;
    label: string;
    detail?: string;
    keywords?: string[];
    run: () => unknown;
}

class PaletteCommands {
    heading = $state("");
    items = $state.raw<PaletteItem[]>([]);

    /** Replaces the page section; returns a cleanup that clears it. */
    set(heading: string, items: PaletteItem[]) {
        this.heading = heading;
        this.items = items;
        return () => {
            if (this.items === items) {
                this.items = [];
                this.heading = "";
            }
        };
    }
}

export const paletteCommands = new PaletteCommands();
