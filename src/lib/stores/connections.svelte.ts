import { commands, type Profile, type ProfileInput } from "@/bindings";

/** Connection profiles and the active selection, shared across the app. */
class ConnectionsStore {
    profiles = $state<Profile[]>([]);
    activeId = $state<string | null>(null);
    loaded = $state(false);

    active = $derived(this.profiles.find((p) => p.id === this.activeId) ?? null);

    async refresh() {
        const list = await commands.listProfiles();
        this.profiles = list.profiles;
        this.activeId = list.activeId ?? null;
        this.loaded = true;
    }

    async select(id: string) {
        await commands.selectProfile(id);
        await this.refresh();
    }

    async create(input: ProfileInput) {
        const profile = await commands.createProfile(input);
        await this.refresh();
        return profile;
    }

    async update(id: string, input: ProfileInput) {
        const profile = await commands.updateProfile(id, input);
        await this.refresh();
        return profile;
    }

    async remove(id: string) {
        await commands.deleteProfile(id);
        await this.refresh();
    }
}

export const connections = new ConnectionsStore();
