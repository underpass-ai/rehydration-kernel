# Deployment Boundary

## Purpose

Clarify what deployment and packaging concerns belong in this repo versus in
sibling repos that consume the kernel.

## Kernel Ownership

This repo owns:

- kernel source code
- kernel contracts
- kernel integration examples
- kernel quality gates
- kernel-focused documentation
- standalone kernel container image packaging
- standalone kernel Helm chart packaging

This repo does not currently own:

- Docker Compose stacks for broader product systems
- Kubernetes deployment bundles for sibling runtimes or products
- Helm packaging for integrating runtimes
- product-specific release pipelines

## Sibling Runtime Packaging

The kernel is designed to be consumed by external runtimes and products.

When a sibling runtime repo publishes:

- container images to GitHub Container Registry
- Docker Compose execution paths
- Kubernetes deployment through Helm

that packaging should remain in the runtime-owning repo unless the artifact is
truly kernel-owned.

## Why This Split Exists

Keeping deployment assets in the owning repo avoids:

- mixing kernel scope with runtime scope
- coupling kernel release cadence to product packaging
- reintroducing product-specific assumptions into the kernel repository

## What A Public Reader Should Infer

From a public perspective:

- this repo is the source of truth for the kernel
- sibling repos may be the source of truth for runnable product or runtime
  distributions
- integration between them is documented here, but not necessarily packaged
  here

## Future Change Rule

Add deployment assets directly to this repo only if they package the kernel
itself as a standalone deliverable.

If the asset packages a runtime, a product adapter, or a broader system, keep
it in the owning sibling repo and link it from here instead.
