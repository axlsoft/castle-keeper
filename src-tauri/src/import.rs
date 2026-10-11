//! Import connection settings from a pasted `client.properties` block, in
//! either librdkafka or Java client form.

use serde::Serialize;
use specta::Type;

use crate::profiles::{SaslConfig, SaslMechanism, TlsConfig};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedConnection {
    pub bootstrap: Option<String>,
    pub sasl: Option<SaslConfig>,
    /// Returned so the form can be filled in; saved only to the keychain.
    pub sasl_password: Option<String>,
    pub tls: TlsConfig,
    pub tls_key_password: Option<String>,
    pub admin_timeout_ms: Option<u32>,
    /// Keys we recognised and applied.
    pub applied: Vec<String>,
    /// Keys we don't map (left for the user to review).
    pub ignored: Vec<String>,
    /// Settings that can't be represented and need attention.
    pub warnings: Vec<String>,
}

/// Parses Java-style properties: `key=value` or `key: value`, `#`/`!`
/// comments, trailing-backslash continuations.
fn properties(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if pending.is_empty() && (line.is_empty() || line.starts_with('#') || line.starts_with('!'))
        {
            continue;
        }
        if let Some(stripped) = line.strip_suffix('\\') {
            pending.push_str(stripped);
            continue;
        }
        pending.push_str(line);
        let entry = std::mem::take(&mut pending);
        let split = entry.find(['=', ':']);
        if let Some(index) = split {
            let key = entry[..index].trim().to_string();
            let value = entry[index + 1..].trim().to_string();
            if !key.is_empty() {
                out.push((key, value));
            }
        }
    }
    out
}

/// Pulls `name="value"` out of a JAAS config line.
fn jaas_option(config: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let start = config.find(&needle)? + needle.len();
    let rest = config[start..].trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        quoted.find('"').map(|end| quoted[..end].to_string())
    } else {
        Some(
            rest.split(|c: char| c.is_whitespace() || c == ';')
                .next()
                .unwrap_or_default()
                .to_string(),
        )
    }
}

pub fn parse_client_properties(text: &str) -> ImportedConnection {
    let mut result = ImportedConnection::default();
    let mut protocol: Option<String> = None;
    let mut mechanism: Option<String> = None;
    let mut username: Option<String> = None;

    for (key, value) in properties(text) {
        let mut applied = true;
        match key.as_str() {
            "bootstrap.servers" | "metadata.broker.list" => result.bootstrap = Some(value),
            "security.protocol" => protocol = Some(value.to_ascii_uppercase()),
            "sasl.mechanism" | "sasl.mechanisms" => mechanism = Some(value.to_ascii_uppercase()),
            "sasl.username" => username = Some(value),
            "sasl.password" => result.sasl_password = Some(value),
            "sasl.jaas.config" => {
                username = jaas_option(&value, "username").or(username);
                result.sasl_password =
                    jaas_option(&value, "password").or(result.sasl_password.take());
                if mechanism.is_none() {
                    if value.contains("ScramLoginModule") {
                        result.warnings.push(
                            "JAAS uses SCRAM; check the SCRAM variant (SHA-256 or SHA-512).".into(),
                        );
                        mechanism = Some("SCRAM-SHA-512".into());
                    } else if value.contains("PlainLoginModule") {
                        mechanism = Some("PLAIN".into());
                    }
                }
            }
            "ssl.ca.location" => result.tls.ca_path = Some(value),
            "ssl.certificate.location" => result.tls.cert_path = Some(value),
            "ssl.key.location" => result.tls.key_path = Some(value),
            "ssl.key.password" => result.tls_key_password = Some(value),
            "ssl.endpoint.identification.algorithm" => {
                result.tls.verify_hostname =
                    !(value.is_empty() || value.eq_ignore_ascii_case("none"));
            }
            "request.timeout.ms" | "default.api.timeout.ms" => {
                if let Ok(ms) = value.parse() {
                    result.admin_timeout_ms = Some(ms);
                }
            }
            "ssl.truststore.location" | "ssl.keystore.location" => {
                applied = false;
                result.warnings.push(format!(
                    "{key}: Java keystores aren't supported. Export the certificates as PEM files and \
                     set them under TLS."
                ));
            }
            _ => applied = false,
        }
        if applied {
            result.applied.push(key);
        } else if !key.starts_with("ssl.truststore") && !key.starts_with("ssl.keystore") {
            result.ignored.push(key);
        }
    }

    if let Some(protocol) = &protocol {
        result.tls.enabled = protocol.ends_with("SSL");
        if !protocol.starts_with("SASL") {
            mechanism = None;
        }
    }
    result.sasl = match mechanism.as_deref() {
        Some("PLAIN") => Some(SaslMechanism::Plain),
        Some("SCRAM-SHA-256") => Some(SaslMechanism::ScramSha256),
        Some("SCRAM-SHA-512") => Some(SaslMechanism::ScramSha512),
        Some(other) => {
            result
                .warnings
                .push(format!("SASL mechanism {other} isn't supported yet."));
            None
        }
        None => None,
    }
    .map(|mechanism| SaslConfig {
        mechanism,
        username: username.unwrap_or_default(),
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_confluent_cloud_style() {
        let imported = parse_client_properties(
            r#"
# Required connection configs for Kafka producer, consumer, and admin
bootstrap.servers=pkc-123.us-east-1.aws.confluent.cloud:9092
security.protocol=SASL_SSL
sasl.mechanism=PLAIN
sasl.jaas.config=org.apache.kafka.common.security.plain.PlainLoginModule required username="KEY" password="SECRET";
client.dns.lookup=use_all_dns_ips
session.timeout.ms=45000
"#,
        );
        assert_eq!(
            imported.bootstrap.as_deref(),
            Some("pkc-123.us-east-1.aws.confluent.cloud:9092")
        );
        assert!(imported.tls.enabled);
        assert!(imported.tls.verify_hostname);
        assert_eq!(
            imported.sasl,
            Some(SaslConfig {
                mechanism: SaslMechanism::Plain,
                username: "KEY".into()
            })
        );
        assert_eq!(imported.sasl_password.as_deref(), Some("SECRET"));
        assert_eq!(
            imported.ignored,
            vec!["client.dns.lookup", "session.timeout.ms"]
        );
        assert!(imported.warnings.is_empty());
    }

    #[test]
    fn librdkafka_style_with_mtls() {
        let imported = parse_client_properties(
            "metadata.broker.list: b1:9093,b2:9093\n\
             security.protocol = ssl\n\
             ssl.ca.location=/etc/ca.pem\n\
             ssl.certificate.location=/etc/client.pem\n\
             ssl.key.location=/etc/client.key\n\
             ssl.key.password=kp\n\
             ssl.endpoint.identification.algorithm=none\n",
        );
        assert_eq!(imported.bootstrap.as_deref(), Some("b1:9093,b2:9093"));
        assert!(imported.tls.enabled);
        assert!(!imported.tls.verify_hostname);
        assert_eq!(imported.tls.key_path.as_deref(), Some("/etc/client.key"));
        assert_eq!(imported.tls_key_password.as_deref(), Some("kp"));
        assert_eq!(imported.sasl, None);
    }

    #[test]
    fn scram_with_separate_credentials_and_continuation() {
        let imported = parse_client_properties(
            "bootstrap.servers=localhost:9094,\\\n    localhost:9095\n\
             security.protocol=SASL_PLAINTEXT\n\
             sasl.mechanisms=SCRAM-SHA-256\n\
             sasl.username=alice\n\
             sasl.password=pw\n",
        );
        assert_eq!(
            imported.bootstrap.as_deref(),
            Some("localhost:9094,localhost:9095")
        );
        assert!(!imported.tls.enabled);
        assert_eq!(imported.sasl.unwrap().mechanism, SaslMechanism::ScramSha256);
        assert_eq!(imported.sasl_password.as_deref(), Some("pw"));
    }

    #[test]
    fn unsupported_settings_become_warnings() {
        let imported = parse_client_properties(
            "bootstrap.servers=x:1\nsecurity.protocol=SASL_SSL\nsasl.mechanism=GSSAPI\n\
             ssl.truststore.location=/etc/truststore.jks\nssl.truststore.password=changeit\n",
        );
        assert_eq!(imported.sasl, None);
        assert_eq!(imported.warnings.len(), 2, "{:?}", imported.warnings);
        assert!(imported.ignored.is_empty(), "{:?}", imported.ignored);
    }
}
