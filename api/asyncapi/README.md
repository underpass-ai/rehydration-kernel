# AsyncAPI Contracts

The first event contract lives in `context-projection.v1alpha1.yaml`.

It documents:

- inbound node-centric projection events consumed by the kernel;
- the shared event envelope required across subjects;
- outbound notifications emitted after bundle generation.

This AsyncAPI contract is the generic kernel-owned async boundary.
Legacy compatibility subjects used during migration should not be treated as the
preferred integration contract for a new product.
