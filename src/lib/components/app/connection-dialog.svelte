<script lang="ts">
    import * as Dialog from "$lib/components/ui/dialog";
    import { Button } from "$lib/components/ui/button";
    import Input from "$lib/components/ui/input/input.svelte";
    import type {
        ConnectionReport,
        Environment,
        Profile,
        ProfileInput,
        SaslMechanism,
    } from "@/bindings";
    import { commands } from "@/bindings";
    import { ENVIRONMENTS } from "@/environments";
    import { toAppError, type AppError } from "@/ipc-service";
    import { connections } from "@/stores/connections.svelte";

    type AuthMethod = "none" | SaslMechanism;

    interface Props {
        open: boolean;
        /** Profile to edit; null creates a new one. */
        profile?: Profile | null;
        onsaved?: (profile: Profile) => void;
    }

    let { open = $bindable(), profile = null, onsaved }: Props = $props();

    let name = $state("");
    let environment = $state<Environment>("local");
    let bootstrap = $state("");
    let auth = $state<AuthMethod>("none");
    let username = $state("");
    let tlsEnabled = $state(false);
    let caPath = $state("");
    let certPath = $state("");
    let keyPath = $state("");
    let verifyHostname = $state(true);
    let readOnly = $state(false);
    let adminTimeoutMs = $state(10000);
    let deliveryTimeoutMs = $state(15000);

    let testing = $state(false);
    let saving = $state(false);
    let report = $state<ConnectionReport | null>(null);
    let error = $state<AppError | null>(null);

    // Reset the form whenever the dialog opens.
    $effect(() => {
        if (!open) return;
        const p = profile;
        name = p?.name ?? "";
        environment = p?.environment ?? "local";
        bootstrap = p?.bootstrap ?? "";
        auth = p?.sasl?.mechanism ?? "none";
        username = p?.sasl?.username ?? "";
        tlsEnabled = p?.tls?.enabled ?? false;
        caPath = p?.tls?.caPath ?? "";
        certPath = p?.tls?.certPath ?? "";
        keyPath = p?.tls?.keyPath ?? "";
        verifyHostname = p?.tls?.verifyHostname ?? true;
        readOnly = p?.readOnly ?? false;
        adminTimeoutMs = p?.adminTimeoutMs ?? 10000;
        deliveryTimeoutMs = p?.deliveryTimeoutMs ?? 15000;
        report = null;
        error = null;
    });

    const isProd = $derived(environment === "prod");

    function input(): ProfileInput {
        return {
            name,
            environment,
            bootstrap,
            sasl: auth === "none" ? null : { mechanism: auth, username },
            tls: {
                enabled: tlsEnabled,
                caPath: caPath || null,
                certPath: certPath || null,
                keyPath: keyPath || null,
                verifyHostname,
            },
            readOnly: readOnly || isProd,
            adminTimeoutMs,
            deliveryTimeoutMs,
        };
    }

    function useLocal() {
        if (!name) name = "Local";
        environment = "local";
        bootstrap = "localhost:9092";
        auth = "none";
        tlsEnabled = false;
    }

    async function test() {
        testing = true;
        report = null;
        error = null;
        try {
            report = await commands.testConnection(input());
        } catch (e) {
            error = toAppError(e);
        } finally {
            testing = false;
        }
    }

    async function save(event: Event) {
        event.preventDefault();
        saving = true;
        error = null;
        try {
            const saved = profile
                ? await connections.update(profile.id, input())
                : await connections.create(input());
            open = false;
            onsaved?.(saved);
        } catch (e) {
            error = toAppError(e);
        } finally {
            saving = false;
        }
    }

    const fieldLabel = "text-muted-foreground text-xs font-medium";
</script>

<Dialog.Root bind:open>
    <Dialog.Content class="max-h-[90vh] max-w-xl overflow-y-auto">
        <Dialog.Header>
            <Dialog.Title>{profile ? "Edit connection" : "New connection"}</Dialog.Title>
            <Dialog.Description>
                Secrets are stored in the system keychain, never in the connection file.
            </Dialog.Description>
        </Dialog.Header>

        <form id="connection-form" onsubmit={save} class="grid gap-4">
            {#if !profile}
                <button
                    type="button"
                    class="border-border hover:bg-accent rounded-md border border-dashed px-3 py-2 text-left text-sm"
                    onclick={useLocal}
                >
                    <span class="font-medium">Local Kafka</span>
                    <span class="text-muted-foreground font-mono"> localhost:9092 · no auth</span>
                </button>
            {/if}

            <div class="grid grid-cols-2 gap-3">
                <label class="grid gap-1">
                    <span class={fieldLabel}>Name</span>
                    <Input bind:value={name} placeholder="orders-prod" required />
                </label>
                <div class="grid gap-1">
                    <span class={fieldLabel}>Environment</span>
                    <div class="flex gap-1" role="radiogroup" aria-label="Environment">
                        {#each ENVIRONMENTS as env (env.value)}
                            <button
                                type="button"
                                role="radio"
                                aria-checked={environment === env.value}
                                onclick={() => (environment = env.value)}
                                class="flex-1 rounded-md border px-2 py-1.5 text-xs font-medium transition-colors {environment ===
                                env.value
                                    ? `${env.band} border-transparent text-white`
                                    : 'hover:bg-accent'}"
                            >
                                {env.label}
                            </button>
                        {/each}
                    </div>
                </div>
            </div>

            <label class="grid gap-1">
                <span class={fieldLabel}>Bootstrap servers</span>
                <Input class="font-mono" bind:value={bootstrap} placeholder="broker1:9092,broker2:9092" required />
            </label>

            <div class="grid grid-cols-2 gap-3">
                <label class="grid gap-1">
                    <span class={fieldLabel}>Authentication</span>
                    <select bind:value={auth} class="border-input bg-background h-10 rounded-md border px-2 text-sm">
                        <option value="none">None</option>
                        <option value="plain">SASL/PLAIN</option>
                        <option value="scramSha256">SASL/SCRAM-SHA-256</option>
                        <option value="scramSha512">SASL/SCRAM-SHA-512</option>
                    </select>
                </label>
                {#if auth !== "none"}
                    <label class="grid gap-1">
                        <span class={fieldLabel}>Username</span>
                        <Input bind:value={username} autocomplete="off" required />
                    </label>
                {/if}
            </div>

            <div class="grid gap-2">
                <label class="flex items-center gap-2 text-sm">
                    <input type="checkbox" bind:checked={tlsEnabled} />
                    Use TLS
                </label>
                {#if tlsEnabled}
                    <div class="grid gap-2 border-l-2 pl-3">
                        <label class="grid gap-1">
                            <span class={fieldLabel}>CA certificate (PEM, optional — system trust store if empty)</span>
                            <Input class="font-mono" bind:value={caPath} placeholder="/path/to/ca.pem" />
                        </label>
                        <div class="grid grid-cols-2 gap-2">
                            <label class="grid gap-1">
                                <span class={fieldLabel}>Client certificate (mTLS)</span>
                                <Input class="font-mono" bind:value={certPath} placeholder="/path/to/client.pem" />
                            </label>
                            <label class="grid gap-1">
                                <span class={fieldLabel}>Client key (mTLS)</span>
                                <Input class="font-mono" bind:value={keyPath} placeholder="/path/to/client.key" />
                            </label>
                        </div>
                        <label class="flex items-center gap-2 text-sm">
                            <input type="checkbox" bind:checked={verifyHostname} />
                            Verify broker hostname
                        </label>
                    </div>
                {/if}
            </div>

            <label class="flex items-start gap-2 text-sm">
                <input type="checkbox" class="mt-0.5" checked={readOnly || isProd} disabled={isProd}
                    onchange={(e) => (readOnly = e.currentTarget.checked)} />
                <span>
                    Read-only
                    <span class="text-muted-foreground block text-xs">
                        {#if isProd}
                            Production connections are always read-only. Writes need a per-session unlock.
                        {:else}
                            Block every write to this cluster.
                        {/if}
                    </span>
                </span>
            </label>

            <details class="text-sm">
                <summary class="text-muted-foreground cursor-pointer text-xs">Timeouts</summary>
                <div class="mt-2 grid grid-cols-2 gap-3">
                    <label class="grid gap-1">
                        <span class={fieldLabel}>Admin requests (ms)</span>
                        <Input type="number" min="1000" step="1000" bind:value={adminTimeoutMs} />
                    </label>
                    <label class="grid gap-1">
                        <span class={fieldLabel}>Message delivery (ms)</span>
                        <Input type="number" min="1000" step="1000" bind:value={deliveryTimeoutMs} />
                    </label>
                </div>
            </details>

            {#if report}
                <div class="rounded-md border border-emerald-600/40 bg-emerald-600/5 p-3 text-sm" role="status">
                    <p class="font-medium text-emerald-700 dark:text-emerald-400">Connected</p>
                    <dl class="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs tabular-nums">
                        <dt class="text-muted-foreground">Brokers</dt>
                        <dd class="font-mono">
                            {report.brokers.map((b) => `${b.id} ${b.host}:${b.port}`).join(", ")}
                        </dd>
                        <dt class="text-muted-foreground">Controller</dt>
                        <dd class="font-mono">{report.controllerId ?? "unknown"}</dd>
                        <dt class="text-muted-foreground">Cluster id</dt>
                        <dd class="font-mono">{report.clusterId ?? "unknown"}</dd>
                        <dt class="text-muted-foreground">Topics</dt>
                        <dd>{report.topicCount}</dd>
                        <dt class="text-muted-foreground">Latency</dt>
                        <dd>{report.connectMs} ms connect · {report.roundTripMs} ms round trip</dd>
                    </dl>
                </div>
            {/if}
            {#if error}
                <div class="border-destructive/40 bg-destructive/5 rounded-md border p-3 text-sm" role="alert">
                    <p class="text-destructive font-medium">{error.message}</p>
                    {#if error.hint}<p class="text-muted-foreground mt-1 text-xs">{error.hint}</p>{/if}
                </div>
            {/if}
        </form>

        <Dialog.Footer class="gap-2">
            <Button variant="outline" onclick={test} disabled={testing || !bootstrap}>
                {testing ? "Testing…" : "Test connection"}
            </Button>
            <Button type="submit" form="connection-form" disabled={saving}>
                {profile ? "Save" : "Add connection"}
            </Button>
        </Dialog.Footer>
    </Dialog.Content>
</Dialog.Root>
