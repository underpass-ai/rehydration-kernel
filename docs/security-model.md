# Security Model

## Trust Boundaries

The kernel operates within the following trust boundaries:

```
┌─────────────────────────────────┐
│  External callers (agents,      │
│  runtimes, operators)           │
│  ↕ gRPC (TLS optional)         │
├─────────────────────────────────┤
│  rehydration-kernel             │
│  ↕ internal adapters            │
├─────────────────────────────────┤
│  Neo4j    Valkey    NATS        │
│  (graph)  (detail/  (events)    │
│           snapshots)            │
└─────────────────────────────────┘
```

### Boundary 1: gRPC Transport → Kernel

- **TLS**: Supports disabled, server-only, and mutual TLS modes via
  `REHYDRATION_GRPC_TLS_MODE`.
- **Authentication**: None in v1beta1. No caller identity is extracted or
  validated. All callers are treated as trusted.
- **Authorization**: None in v1beta1. `ValidateScope` is a set-comparison
  utility; it does not enforce access control. `GetContext` does not include
  scope validation in its response.

### Boundary 2: Kernel → Neo4j

- **Transport**: Supports `neo4j://` (plaintext) and `bolt+s://` / `neo4j+s://`
  (TLS). The Helm chart supports mounting a CA certificate for private trust
  via `neo4jTls.existingSecret`.
- **Authentication**: URI-embedded credentials (`neo4j://user:pass@host`).
  Credentials should be provided via Kubernetes secrets, not inline values.
- **Authorization**: The kernel uses a single Neo4j connection identity.
  No per-caller credential delegation.

### Boundary 3: Kernel → Valkey

- **Transport**: Supports `redis://` (plaintext) and `rediss://` (TLS).
  Client certificates supported for mutual TLS via `valkeyTls.*` Helm values.
- **Authentication**: URI-embedded or TLS client identity.
- **Data at rest**: Valkey does not encrypt data at rest by default. Snapshots,
  node details, and event store entries are stored as JSON strings.

### Boundary 4: Kernel → NATS

- **Transport**: Supports plaintext and TLS via `natsTls.*` Helm values.
- **Authentication**: NATS connection credentials.
- **Authorization**: Subject-level permissions managed in NATS, not in the kernel.

## Threat Model (v1beta1)

| Threat | Mitigation | Status |
|--------|-----------|--------|
| Unauthenticated gRPC access | Mutual TLS available | Available but not enforced |
| Unauthorized context reads | None | Not implemented |
| Unauthorized context writes | Optimistic concurrency via revision check | Partial |
| Replay attacks on commands | Idempotency key deduplication | Implemented |
| Credential exposure in config | Kubernetes secrets, not inline URIs | Documented |
| Data exfiltration from Valkey | TLS transport, network isolation | Available |
| Man-in-the-middle on Neo4j | TLS with CA pinning | Available |
| Admin plane abuse | gRPC admin has no separate auth | Not mitigated |

## Recommendations for Production

1. Enable mutual TLS for gRPC transport.
2. Use Kubernetes secrets for all connection URIs.
3. Enable TLS for Neo4j, Valkey, and NATS connections.
4. Restrict NATS subject permissions to kernel-owned prefixes.
5. Network-isolate the kernel namespace from untrusted workloads.
6. Implement caller authentication before exposing the kernel externally.

## What the Kernel Does NOT Do

- Does not authenticate callers.
- Does not enforce authorization or access control.
- Does not encrypt data at rest.
- Does not manage secrets or rotate credentials.
- Does not validate the truthfulness of application-supplied explanations.
