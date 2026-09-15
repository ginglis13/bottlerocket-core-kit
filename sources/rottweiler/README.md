# rottweiler

Current version: 0.1.0

## Introduction

*rottweiler* is Bottlerocket's storage encryption helper. It provides a unified
interface for encrypting and managing encrypted storage resources including:

- Block devices (using LUKS or plain mode encryption)
- Directories (using fscrypt)
- TPM PCR measurements

### Commands

#### Key Management
- `generate-key <key-id>` - Generate an encryption key
- `delete-key <key-id>` - Delete an encryption key from the keystore

#### Block Device Operations
- `encrypt block-device <path> <key-id>` - Encrypt a block device using LUKS
- `encrypt-and-attach block-device <path> <key-id>` - Encrypt (plain mode) and attach a block device in one step
- `attach block-device <path> <key-id>` - Attach an encrypted block device
- `detach block-device <path>` - Detach an encrypted block device
- `resize block-device <path> <key-id>` - Resize an encrypted block device to match the underlying device size
- `check block-device <path> encrypted|unencrypted` - Check block device encryption state

#### Directory Operations
- `encrypt directory <path> <key-id>` - Encrypt a directory using fscrypt
- `lock directory <path>` - Lock an encrypted directory (remove key)
- `unlock directory <path> <key-id>` - Unlock an encrypted directory (add key)
- `check directory <path> encrypted|unencrypted` - Check directory encryption state

#### TPM Measurement Operations
- `measure settings` - Measure OS settings into PCR 8
- `measure user-data` - Measure EC2 IMDS user data into PCR 8
- `measure kernel-command-line` - Measure kernel command line into PCR 9
- `measure pcrphase <phase>` - Measure boot phase into PCR 11
  - Valid phases: `sysinit`, `preconfigured`, `configured`, `ready`, `shutdown`, `final`

### PCR 8

PCR 8 is extended exactly twice per boot, in this order:

1. `measure-user-data.service` runs `measure user-data` before settings are applied. The
   measured bytes are `ec2-imds-user-data:v1:` followed by the raw user data exactly as IMDS
   returns it, or the literal `ec2-imds-user-data:v1:absent` when IMDS has no user data.
2. `measure-settings.service` runs `measure settings` after `settings-applier.service`, before
   `bootstrap-commands.service`. The measured bytes are the OLPC canonical JSON of the settings
   tree (as printed by `apiclient get settings --canonicalize`), followed by a newline.

`settings.updates.seed` and `settings.network.hostname` are always excluded from the settings
measurement. The `settings.measurement` table adjusts what else is measured:

- `excluded-settings` lists additional settings nodes to exclude, for example
  `["host-containers", "aws.region"]`. Each entry excludes the node and everything beneath it.
  The bare `settings` node, empty strings, wildcards, and `settings.measurement` itself are
  rejected.
- `region-agnostic-image-sources = true` measures Amazon ECR image sources in
  `settings.host-containers.*.source` and `settings.bootstrap-containers.*.source` with the
  region segment of the registry host replaced by the literal placeholder `{region}`, e.g.
  `123456789012.dkr.ecr.{region}.amazonaws.com/agent@sha256:...`. The account ID, `-fips`
  marker, partition suffix, repository, tag, and digest are retained, and sources outside ECR
  are unchanged. The stored settings are not modified; only the measured bytes are.

Changes to `settings.measurement` take effect at the next boot. No TPM event log is written for
these extensions; a verifier recomputes the expected value from the same inputs.

### Aliases

For convenience, the following aliases are supported:
- `dir` can be used instead of `directory`
- `bdev` can be used instead of `block-device`

## Colophon

This text was generated using [cargo-readme](https://crates.io/crates/cargo-readme), and includes the rustdoc from `src/main.rs`.
