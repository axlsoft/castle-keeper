import { writable } from "svelte/store";
import { Store } from "@tauri-apps/plugin-store";

export let store: Store;

export const theme = writable("light");

async function loadTheme() {
  console.log("Loading theme");
  store = await Store.load(".settings.json");
  let savedTheme = await store.get<string>("theme");
  if (savedTheme) {
    theme.set(savedTheme);
    document.documentElement.setAttribute("data-theme", savedTheme);
  }
  else {
    detectSystemTheme();
  }

  theme.subscribe(async (value) => {
    document.documentElement.setAttribute("data-theme", value);
    if (await store?.get("theme") === value) return;
    await store.set("theme", value);
    await store.save();
  });

}

export async function detectSystemTheme() {
  const systemPrefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  theme.set(systemPrefersDark ? "dark" : "light");
}

loadTheme();
