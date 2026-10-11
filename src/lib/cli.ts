import type { BrowseRequest, Profile } from "./bindings";

/**
 * Shell commands equivalent to what a view shows, for the Apache Kafka CLI
 * tools and kcat. Secrets are never included: passwords are referenced as
 * environment variables the user sets themselves.
 */

export const PASSWORD_ENV = "KAFKA_SASL_PASSWORD";
export const KEY_PASSWORD_ENV = "KAFKA_SSL_KEY_PASSWORD";

export interface CliCommand {
    label: string;
    command: string;
}

/** POSIX single-quoting; leaves simple tokens bare for readability. */
export function quote(value: string): string {
    return /^[A-Za-z0-9_./:@,=+-]+$/.test(value) ? value : `'${value.replace(/'/g, `'"'"'`)}'`;
}

function securityProtocol(profile: Profile): string {
    const sasl = !!profile.sasl;
    const tls = !!profile.tls?.enabled;
    return sasl ? (tls ? "SASL_SSL" : "SASL_PLAINTEXT") : tls ? "SSL" : "PLAINTEXT";
}

const mechanism = { plain: "PLAIN", scramSha256: "SCRAM-SHA-256", scramSha512: "SCRAM-SHA-512" } as const;

/** librdkafka properties for kcat (-X key=value). */
function librdkafkaProperties(profile: Profile): [string, string][] {
    const props: [string, string][] = [];
    const protocol = securityProtocol(profile);
    if (protocol !== "PLAINTEXT") props.push(["security.protocol", protocol]);
    if (profile.sasl) {
        props.push(["sasl.mechanisms", mechanism[profile.sasl.mechanism]]);
        props.push(["sasl.username", profile.sasl.username]);
        props.push(["sasl.password", `$${PASSWORD_ENV}`]);
    }
    if (profile.tls?.enabled) {
        if (profile.tls.caPath) props.push(["ssl.ca.location", profile.tls.caPath]);
        if (profile.tls.certPath) props.push(["ssl.certificate.location", profile.tls.certPath]);
        if (profile.tls.keyPath) {
            props.push(["ssl.key.location", profile.tls.keyPath]);
            props.push(["ssl.key.password", `$${KEY_PASSWORD_ENV}`]);
        }
        if (profile.tls.verifyHostname === false) props.push(["ssl.endpoint.identification.algorithm", "none"]);
    }
    return props;
}

/** Quote a -X value, keeping $VAR references expandable. */
function kcatValue(key: string, value: string): string {
    return value.startsWith("$") ? `"${key}=${value}"` : quote(`${key}=${value}`);
}

function kcatBase(profile: Profile): string {
    const props = librdkafkaProperties(profile).map(([k, v]) => `-X ${kcatValue(k, v)}`);
    return ["kcat", "-b", quote(profile.bootstrap), ...props].join(" ");
}

/**
 * Java client properties for the Apache Kafka CLI tools, written to a
 * temporary file with a heredoc (null for plaintext connections).
 */
function javaClientProperties(profile: Profile): string | null {
    const protocol = securityProtocol(profile);
    if (protocol === "PLAINTEXT") return null;
    const lines = [`security.protocol=${protocol}`];
    if (profile.sasl) {
        const login =
            profile.sasl.mechanism === "plain"
                ? "org.apache.kafka.common.security.plain.PlainLoginModule"
                : "org.apache.kafka.common.security.scram.ScramLoginModule";
        lines.push(`sasl.mechanism=${mechanism[profile.sasl.mechanism]}`);
        lines.push(
            `sasl.jaas.config=${login} required username="${profile.sasl.username}" password="\${${PASSWORD_ENV}}";`,
        );
    }
    if (profile.tls?.enabled) {
        if (profile.tls.caPath) {
            lines.push("ssl.truststore.type=PEM", `ssl.truststore.location=${profile.tls.caPath}`);
        }
        if (profile.tls.certPath && profile.tls.keyPath) {
            lines.push(
                "# The Java client needs the certificate and key combined in one PEM file:",
                `ssl.keystore.type=PEM`,
                `ssl.keystore.location=${profile.tls.certPath}`,
            );
        }
        if (profile.tls.verifyHostname === false) lines.push("ssl.endpoint.identification.algorithm=");
    }
    return lines.join("\n");
}

/** Wraps a Kafka CLI invocation with its connection flags. */
function kafkaTool(profile: Profile, tool: string, args: string[]): string {
    const config = javaClientProperties(profile);
    const invocation = [tool, "--bootstrap-server", quote(profile.bootstrap), ...args];
    if (!config) return invocation.join(" ");
    // Unquoted heredoc so ${KAFKA_SASL_PASSWORD} expands from the environment.
    return [
        "cat > /tmp/castle-keeper-client.properties <<EOF",
        config,
        "EOF",
        [...invocation, "--command-config", "/tmp/castle-keeper-client.properties"].join(" "),
    ].join("\n");
}

export function topicCommands(profile: Profile, topic: string): CliCommand[] {
    return [
        { label: "Describe topic (kafka-topics.sh)", command: kafkaTool(profile, "kafka-topics.sh", ["--describe", "--topic", quote(topic)]) },
        {
            label: "Topic configs (kafka-configs.sh)",
            command: kafkaTool(profile, "kafka-configs.sh", ["--describe", "--entity-type", "topics", "--entity-name", quote(topic)]),
        },
        { label: "Topic metadata (kcat)", command: `${kcatBase(profile)} -L -t ${quote(topic)}` },
    ];
}

export function topicListCommands(profile: Profile): CliCommand[] {
    return [
        { label: "List topics (kafka-topics.sh)", command: kafkaTool(profile, "kafka-topics.sh", ["--list"]) },
        { label: "Cluster metadata (kcat)", command: `${kcatBase(profile)} -L` },
    ];
}

export function groupCommands(profile: Profile, groupId: string): CliCommand[] {
    return [
        {
            label: "Describe group (kafka-consumer-groups.sh)",
            command: kafkaTool(profile, "kafka-consumer-groups.sh", ["--describe", "--group", quote(groupId)]),
        },
        {
            label: "Group members (kafka-consumer-groups.sh)",
            command: kafkaTool(profile, "kafka-consumer-groups.sh", ["--describe", "--members", "--verbose", "--group", quote(groupId)]),
        },
    ];
}

export function groupListCommands(profile: Profile): CliCommand[] {
    return [
        {
            label: "List groups (kafka-consumer-groups.sh)",
            command: kafkaTool(profile, "kafka-consumer-groups.sh", ["--list", "--state"]),
        },
    ];
}

/** kcat consume command matching a browse request (never commits). */
export function browseCommands(profile: Profile, request: BrowseRequest): CliCommand[] {
    const start =
        request.start.kind === "earliest" ? "beginning"
        : request.start.kind === "latest" ? `-${request.start.count}`
        : request.start.kind === "offset" ? String(request.start.offset)
        : `s@${request.start.timestamp}`;
    const format = quote("%t [%p] @%o %T key=%k\\n%s\\n\\n");
    const base = `${kcatBase(profile)} -C -t ${quote(request.topic)} -o ${start} -c ${request.limit} -e -f ${format}`;
    const partitions = request.partitions ?? [];
    const command = partitions.length
        ? partitions.map((p) => `${base} -p ${p}`).join("\n")
        : base;
    return [{ label: "Read the same messages (kcat)", command }];
}
