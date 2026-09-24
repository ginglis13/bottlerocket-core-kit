//! Transformations applied to OS settings before they are measured into PCR 8.
//!
//! `settings.measurement.image-sources` selects how container image sources are measured:
//!
//! - `region-agnostic`: the region segment of every Amazon ECR private registry host found in a
//!   container image source is replaced with the literal placeholder `{region}`, so that the same
//!   image replicated across regions within one account measures to the same value. Everything
//!   else in the source (account ID, `-fips` marker, partition suffix, repository, tag, and digest)
//!   is preserved.
//! - `location-agnostic`: the Amazon ECR private registry host is removed entirely, leaving the
//!   repository, tag, and digest, so that the same image replicated across regions and accounts
//!   measures to the same value.
//!
//! Non-ECR sources are left untouched in every mode.
//!
//! The rewritten settings are then serialized with the same OLPC canonical JSON formatter that
//! `apiclient get --canonicalize` uses, followed by a newline, so the framing of the measured
//! bytes is identical to the un-transformed path.

use crate::system::ImageSourceMeasurement;
use regex::Regex;
use serde::Serialize;
use snafu::prelude::*;
use std::sync::LazyLock;
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, snafu::Whatever>;

/// Placeholder substituted for the region segment of an ECR private registry host.
pub const REGION_PLACEHOLDER: &str = "{region}";

/// The settings tables whose members carry a container image `source`.
const IMAGE_SOURCE_TABLES: &[&str] = &["host-containers", "bootstrap-containers"];

/// Amazon ECR private registry hosts, e.g. `777777777777.dkr.ecr.us-west-2.amazonaws.com`.
///
/// This mirrors `ecrPrivateHostPattern` in `sources/host-ctr/cmd/host-ctr/ecr.go` so that
/// rottweiler and host-ctr agree on what counts as an ECR host. Capture groups: 1 = account ID,
/// 2 = optional `-fips`, 3 = region.
static ECR_PRIVATE_HOST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^([0-9]{12})",                            // [1] account ID (12 digits)
        r"\.dkr[.-]ecr",                            // .dkr.ecr or .dkr-ecr
        r"(-fips)?",                                // [2] optional -fips
        r"\.([a-zA-Z0-9][a-zA-Z0-9_-]*)",           // [3] region
        r"\.(?:",                                   // domain suffix (non-capturing)
        r"amazonaws\.(?:com(?:\.cn)?|eu)|",         // amazonaws.com, .com.cn, .eu
        r"on\.(?:aws|amazonwebservices\.com\.cn)|", // on.aws, on.amazonwebservices.com.cn
        r"sc2s\.sgov\.gov|",                        // Isolated regions
        r"c2s\.ic\.gov|",
        r"cloud\.adc-e\.uk|",
        r"csp\.hci\.ic\.gov",
        r")$",
    ))
    .expect("ECR host pattern is valid")
});

/// Split a container image source into its Amazon ECR private registry host and the remainder.
///
/// Returns `None` when the source does not name an ECR private registry host (including sources
/// with no registry component at all).
fn split_ecr_source(source: &str) -> Option<(&str, regex::Captures<'_>, &str)> {
    let (host, path) = source.split_once('/')?;
    let captures = ECR_PRIVATE_HOST.captures(host)?;
    Some((host, captures, path))
}

/// Produce the region-agnostic form of a container image source, or `None` if it should be
/// measured unchanged.
pub fn region_agnostic_source(source: &str) -> Option<String> {
    let (host, captures, path) = split_ecr_source(source)?;
    let region = captures.get(3)?;
    let mut rewritten = String::with_capacity(source.len());
    rewritten.push_str(&host[..region.start()]);
    rewritten.push_str(REGION_PLACEHOLDER);
    rewritten.push_str(&host[region.end()..]);
    rewritten.push('/');
    rewritten.push_str(path);
    Some(rewritten)
}

/// Produce the location-agnostic form of a container image source (repository, tag, and digest
/// only), or `None` if it should be measured unchanged.
pub fn location_agnostic_source(source: &str) -> Option<String> {
    let (_, _, path) = split_ecr_source(source)?;
    Some(path.to_string())
}

/// Produce the form of a container image source to measure under `mode`, or `None` if it should
/// be measured unchanged.
pub fn measured_source(source: &str, mode: ImageSourceMeasurement) -> Option<String> {
    match mode {
        ImageSourceMeasurement::Full => None,
        ImageSourceMeasurement::RegionAgnostic => region_agnostic_source(source),
        ImageSourceMeasurement::LocationAgnostic => location_agnostic_source(source),
    }
}

/// Rewrite every container image `source` under `settings.host-containers` and
/// `settings.bootstrap-containers` to the form selected by `mode`, in place.
///
/// Returns the number of sources that were rewritten.
pub fn rewrite_image_sources(
    settings: &mut serde_json::Value,
    mode: ImageSourceMeasurement,
) -> usize {
    let mut rewritten = 0;
    for table in IMAGE_SOURCE_TABLES {
        let Some(containers) = settings
            .pointer_mut(&format!("/settings/{table}"))
            .and_then(serde_json::Value::as_object_mut)
        else {
            continue;
        };
        for container in containers.values_mut() {
            let Some(source) = container.get_mut("source") else {
                continue;
            };
            let Some(replacement) = source.as_str().and_then(|s| measured_source(s, mode)) else {
                continue;
            };
            *source = serde_json::Value::String(replacement);
            rewritten += 1;
        }
    }
    rewritten
}

/// Serialize settings as OLPC canonical JSON followed by a newline.
///
/// This reproduces the bytes `apiclient get --canonicalize` prints (its output is written with
/// `println!`), so measurements taken through this path are framed identically to those taken
/// directly from apiclient.
pub fn canonicalize(settings: &serde_json::Value) -> Result<Zeroizing<Vec<u8>>> {
    let mut buf = Zeroizing::new(Vec::new());
    let mut ser =
        serde_json::Serializer::with_formatter(&mut *buf, olpc_cjson::CanonicalFormatter::new());
    settings
        .serialize(&mut ser)
        .with_whatever_context(|_| "failed to canonicalize settings")?;
    buf.push(b'\n');
    Ok(buf)
}

/// Build the PCR 8 settings measurement input from apiclient's (non-canonical) JSON output,
/// with image sources rewritten to the form selected by `mode`.
pub fn transformed_measurement(
    raw_json: &[u8],
    mode: ImageSourceMeasurement,
) -> Result<Zeroizing<Vec<u8>>> {
    let mut settings: serde_json::Value = serde_json::from_slice(raw_json)
        .with_whatever_context(|_| "failed to parse settings as JSON")?;
    let rewritten = rewrite_image_sources(&mut settings, mode);
    log::info!("rewrote {rewritten} container image source(s) for {mode:?} measurement");
    canonicalize(&settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ImageSourceMeasurement::{Full, LocationAgnostic, RegionAgnostic};
    use serde_json::json;

    // === region_agnostic_source ===

    #[test]
    fn rewrites_ecr_private_hosts_across_partitions() {
        let cases = [
            (
                "123456789012.dkr.ecr.us-west-2.amazonaws.com/repo:tag",
                "123456789012.dkr.ecr.{region}.amazonaws.com/repo:tag",
            ),
            (
                "123456789012.dkr.ecr-fips.us-gov-west-1.amazonaws.com/repo:tag",
                "123456789012.dkr.ecr-fips.{region}.amazonaws.com/repo:tag",
            ),
            (
                "123456789012.dkr-ecr.us-east-1.amazonaws.com/repo:tag",
                "123456789012.dkr-ecr.{region}.amazonaws.com/repo:tag",
            ),
            (
                "123456789012.dkr.ecr.cn-north-1.amazonaws.com.cn/repo:tag",
                "123456789012.dkr.ecr.{region}.amazonaws.com.cn/repo:tag",
            ),
            (
                "123456789012.dkr.ecr.eusc-de-east-1.amazonaws.eu/repo:tag",
                "123456789012.dkr.ecr.{region}.amazonaws.eu/repo:tag",
            ),
            (
                "123456789012.dkr.ecr.us-iso-east-1.c2s.ic.gov/repo:tag",
                "123456789012.dkr.ecr.{region}.c2s.ic.gov/repo:tag",
            ),
            (
                "123456789012.dkr.ecr.us-isob-east-1.sc2s.sgov.gov/repo:tag",
                "123456789012.dkr.ecr.{region}.sc2s.sgov.gov/repo:tag",
            ),
            (
                "123456789012.dkr.ecr.eu-isoe-west-1.cloud.adc-e.uk/repo:tag",
                "123456789012.dkr.ecr.{region}.cloud.adc-e.uk/repo:tag",
            ),
            (
                "123456789012.dkr.ecr.us-isof-south-1.csp.hci.ic.gov/repo:tag",
                "123456789012.dkr.ecr.{region}.csp.hci.ic.gov/repo:tag",
            ),
            (
                "123456789012.dkr.ecr.us-west-2.on.aws/repo:tag",
                "123456789012.dkr.ecr.{region}.on.aws/repo:tag",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                region_agnostic_source(input).as_deref(),
                Some(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn keeps_repository_tag_and_digest() {
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cases = [
            (
                "328549459982.dkr.ecr.us-west-2.amazonaws.com/bottlerocket-admin:v0.21.5"
                    .to_string(),
                "328549459982.dkr.ecr.{region}.amazonaws.com/bottlerocket-admin:v0.21.5"
                    .to_string(),
            ),
            (
                format!("328549459982.dkr.ecr.us-west-2.amazonaws.com/team/agent@{digest}"),
                format!("328549459982.dkr.ecr.{{region}}.amazonaws.com/team/agent@{digest}"),
            ),
            (
                format!("328549459982.dkr.ecr.us-west-2.amazonaws.com/team/agent:v1@{digest}"),
                format!("328549459982.dkr.ecr.{{region}}.amazonaws.com/team/agent:v1@{digest}"),
            ),
            (
                "328549459982.dkr.ecr.us-west-2.amazonaws.com/agent".to_string(),
                "328549459982.dkr.ecr.{region}.amazonaws.com/agent".to_string(),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                region_agnostic_source(&input).as_deref(),
                Some(expected.as_str()),
                "{input}"
            );
        }
    }

    #[test]
    fn location_agnostic_drops_ecr_host_only() {
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cases = [
            (
                "328549459982.dkr.ecr.us-west-2.amazonaws.com/bottlerocket-admin:v0.21.5"
                    .to_string(),
                "bottlerocket-admin:v0.21.5".to_string(),
            ),
            (
                format!(
                    "111122223333.dkr.ecr-fips.us-gov-west-1.amazonaws.com/team/agent@{digest}"
                ),
                format!("team/agent@{digest}"),
            ),
            (
                format!("111122223333.dkr.ecr.cn-north-1.amazonaws.com.cn/team/agent:v1@{digest}"),
                format!("team/agent:v1@{digest}"),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                location_agnostic_source(&input).as_deref(),
                Some(expected.as_str()),
                "{input}"
            );
        }
        // different accounts and regions collapse to the same measured value
        assert_eq!(
            location_agnostic_source(
                "111111111111.dkr.ecr.us-east-1.amazonaws.com/agent@sha256:aa"
            ),
            location_agnostic_source(
                "222222222222.dkr.ecr.eu-west-1.amazonaws.com/agent@sha256:aa"
            ),
        );
    }

    #[test]
    fn full_mode_never_rewrites() {
        let src = "328549459982.dkr.ecr.us-west-2.amazonaws.com/repo:tag";
        assert_eq!(measured_source(src, Full), None);
        assert!(measured_source(src, RegionAgnostic).is_some());
        assert!(measured_source(src, LocationAgnostic).is_some());
    }

    #[test]
    fn leaves_non_ecr_sources_untouched() {
        let cases = [
            "public.ecr.aws/bottlerocket/bottlerocket-admin:v0.21.5",
            "docker.io/library/nginx:latest",
            "quay.io/org/image@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "nginx:latest",
            "localhost:5000/repo:tag",
            "123456789012.dkr.ecr.us-west-2.amazonaws.com:443/repo:tag",
            "12345678901.dkr.ecr.us-west-2.amazonaws.com/repo:tag",
            "123456789012.dkr.ecr.us-west-2.example.com/repo:tag",
            "123456789012.dkr.ecr.us-west-2.amazonaws.com",
            "",
        ];
        for input in cases {
            assert_eq!(region_agnostic_source(input), None, "{input}");
            assert_eq!(location_agnostic_source(input), None, "{input}");
        }
    }

    // === rewrite_image_sources ===

    fn sample_settings() -> serde_json::Value {
        json!({
            "settings": {
                "aws": { "region": "us-west-2" },
                "host-containers": {
                    "admin": {
                        "enabled": true,
                        "source": "328549459982.dkr.ecr.us-west-2.amazonaws.com/bottlerocket-admin:v0.21.5",
                        "superpowered": true
                    },
                    "control": {
                        "enabled": false,
                        "source": "public.ecr.aws/bottlerocket/bottlerocket-control:v0.21.5"
                    },
                    "custom": {
                        "enabled": true
                    }
                },
                "bootstrap-containers": {
                    "agent": {
                        "essential": true,
                        "mode": "always",
                        "source": "111122223333.dkr.ecr.us-west-2.amazonaws.com/agent@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    }
                },
                "motd": "hello"
            }
        })
    }

    #[test]
    fn rewrites_sources_in_both_tables_only() {
        let mut settings = sample_settings();
        assert_eq!(rewrite_image_sources(&mut settings, RegionAgnostic), 2);
        assert_eq!(
            settings,
            json!({
                "settings": {
                    "aws": { "region": "us-west-2" },
                    "host-containers": {
                        "admin": {
                            "enabled": true,
                            "source": "328549459982.dkr.ecr.{region}.amazonaws.com/bottlerocket-admin:v0.21.5",
                            "superpowered": true
                        },
                        "control": {
                            "enabled": false,
                            "source": "public.ecr.aws/bottlerocket/bottlerocket-control:v0.21.5"
                        },
                        "custom": {
                            "enabled": true
                        }
                    },
                    "bootstrap-containers": {
                        "agent": {
                            "essential": true,
                            "mode": "always",
                            "source": "111122223333.dkr.ecr.{region}.amazonaws.com/agent@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        }
                    },
                    "motd": "hello"
                }
            })
        );
    }

    #[test]
    fn location_agnostic_rewrites_sources_in_both_tables() {
        let mut settings = sample_settings();
        assert_eq!(rewrite_image_sources(&mut settings, LocationAgnostic), 2);
        assert_eq!(
            settings
                .pointer("/settings/host-containers/admin/source")
                .unwrap(),
            "bottlerocket-admin:v0.21.5"
        );
        assert_eq!(
            settings
                .pointer("/settings/bootstrap-containers/agent/source")
                .unwrap(),
            "agent@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            settings
                .pointer("/settings/host-containers/control/source")
                .unwrap(),
            "public.ecr.aws/bottlerocket/bottlerocket-control:v0.21.5"
        );
    }

    #[test]
    fn full_mode_rewrites_nothing() {
        let original = sample_settings();
        let mut settings = original.clone();
        assert_eq!(rewrite_image_sources(&mut settings, Full), 0);
        assert_eq!(settings, original);
    }

    #[test]
    fn rewrite_is_a_no_op_without_container_tables() {
        let original = json!({ "settings": { "aws": { "region": "us-west-2" } } });
        let mut settings = original.clone();
        assert_eq!(rewrite_image_sources(&mut settings, RegionAgnostic), 0);
        assert_eq!(settings, original);

        let original = json!({});
        let mut settings = original.clone();
        assert_eq!(rewrite_image_sources(&mut settings, RegionAgnostic), 0);
        assert_eq!(settings, original);
    }

    #[test]
    fn rewrite_ignores_non_string_and_non_object_shapes() {
        let original = json!({
            "settings": {
                "host-containers": {
                    "weird": { "source": 42 },
                    "list": ["328549459982.dkr.ecr.us-west-2.amazonaws.com/x:y"],
                    "flat": "328549459982.dkr.ecr.us-west-2.amazonaws.com/x:y"
                },
                "bootstrap-containers": "not-a-table"
            }
        });
        let mut settings = original.clone();
        assert_eq!(rewrite_image_sources(&mut settings, RegionAgnostic), 0);
        assert_eq!(settings, original);
    }

    // === canonicalize / transformed_measurement ===

    #[test]
    fn canonical_output_is_sorted_compact_and_newline_terminated() {
        let value = json!({
            "settings": {
                "motd": "hi",
                "aws": { "region": "us-west-2" },
                "kernel": { "lockdown": "integrity", "sysctl": { "vm.swappiness": "10" } },
                "ntp": { "time-servers": ["a", "b"] },
                "updates": { "ignore-waves": false, "seed": 1234 }
            }
        });
        let expected = concat!(
            r#"{"settings":{"aws":{"region":"us-west-2"},"kernel":{"lockdown":"integrity","sysctl":{"vm.swappiness":"10"}},"#,
            r#""motd":"hi","ntp":{"time-servers":["a","b"]},"updates":{"ignore-waves":false,"seed":1234}}}"#,
            "\n"
        );
        assert_eq!(
            canonicalize(&value).unwrap().as_slice(),
            expected.as_bytes()
        );
    }

    #[test]
    fn measurement_from_pretty_json_matches_canonical_of_rewritten_value() {
        // apiclient's non-canonical output is pretty-printed standard JSON
        let pretty = serde_json::to_string_pretty(&sample_settings()).unwrap();
        let measured = transformed_measurement(pretty.as_bytes(), RegionAgnostic).unwrap();

        let mut expected_value = sample_settings();
        rewrite_image_sources(&mut expected_value, RegionAgnostic);
        let expected = canonicalize(&expected_value).unwrap();

        assert_eq!(measured.as_slice(), expected.as_slice());
        assert!(measured.ends_with(b"\n"));
        let text = std::str::from_utf8(&measured).unwrap();
        assert!(
            text.contains("328549459982.dkr.ecr.{region}.amazonaws.com/bottlerocket-admin:v0.21.5")
        );
        assert!(text.contains("111122223333.dkr.ecr.{region}.amazonaws.com/agent@sha256:"));
        assert!(text.contains(r#""region":"us-west-2""#));
    }

    #[test]
    fn measurement_survives_control_characters_and_unicode_in_values() {
        // Pretty JSON escapes control characters; canonical JSON emits them raw. Parsing the
        // pretty form and canonicalizing must produce the same bytes as canonicalizing the
        // original value directly.
        let value =
            json!({ "settings": { "motd": "line1\nline2\ttab \u{00e9} \u{0065}\u{0301}" } });
        let pretty = serde_json::to_string_pretty(&value).unwrap();
        assert_eq!(
            transformed_measurement(pretty.as_bytes(), RegionAgnostic)
                .unwrap()
                .as_slice(),
            canonicalize(&value).unwrap().as_slice()
        );
    }

    #[test]
    fn measurement_rejects_invalid_json() {
        assert!(transformed_measurement(b"not json", RegionAgnostic).is_err());
    }

    #[test]
    fn canonicalize_rejects_floats() {
        assert!(canonicalize(&json!({ "settings": { "x": 1.5 } })).is_err());
    }
}
