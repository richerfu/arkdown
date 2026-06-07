---
name: arkdown-build-session
description: Mandatory arkdown build-session and context-propagation rules. Use when Codex edits, reviews, plans, or implements arkdown BuildSession, ModuleCompileContext, startup discovery, profile store, SDK snapshot, dependency graph snapshot, source store, artifact layout, resolver indexes, plugin constructors, build runner, module pipeline, watch invalidation, cache fingerprints, or any change intended to avoid repeated manifest/profile/SDK/package reads.
---

# Arkdown Build Session

## Core Contract

Construct all stable project, module, dependency, SDK, profile, resource, path,
source inventory, package acquisition, and fingerprint data once at startup.
Pass typed context through the compile pipeline. Later stages consume snapshots
and plans; they do not rediscover the project.

## Required Owners

- `BuildSessionBuilder` owns startup discovery.
- `BuildSession` owns immutable project-wide snapshots.
- `ProjectSnapshot` owns project root, selected config, SDK selection, build
  profile, products, signing inputs, modules, and shared project inputs.
- `ModuleSnapshot` owns module root, target, module JSON, package metadata,
  source roots, resource roots, profile references, and module-specific inputs.
- `DependencyGraphSnapshot` owns all package/dependency nodes and classifications.
- `PackageAcquisitionPlanner` owns package search/acquisition tasks and calls
  `arkdown-ohpm`.
- `ProfileStore` owns discovered profile files and parsed profile inventory.
- `SdkSnapshot` owns SDK component/API metadata.
- `PathResolver` owns path normalization and classification.
- `SourceStore` owns source content reads and source fingerprints.
- `ModuleCompileContext` carries `Arc<BuildSession>`, the current module, target
  config, dependency view, artifact layout, and fingerprint view.

## IO Rules

- `module.json5`, `oh-package.json5`, profile JSON, SDK metadata, dependency
  roots, and resource inventories are read once during session construction.
- OHPM lockfile/cache lookup and registry fetch are startup package acquisition
  inputs. They are not profile-processing inputs.
- Plugin hooks must not read project/package/profile/SDK files.
- Resolver hot paths must not walk upward to find package roots.
- Profile processors consume `ProfileStore` and `ModuleCompileContext`.
- Resource, pack, sign, cache, obfuscate, ts2abc, and abc stages consume typed
  plans derived from context.

## Context Propagation

- Plugin constructors accept `Arc<ModuleCompileContext>` or a narrower plan.
- `arkdown-cli` builds the session, then builds a `BuildPlan`.
- `BuildPlanBuilder` converts `ModuleCompileContext` into domain plans.
- `BuildRunner` and `ModulePipeline` own orchestration.
- Domain crates do not depend on `arkdown-session`; their plan structs contain
  the data they need.
- `arkdown` receives a `BundlePlan` and rolldown plugin trait objects.

## Dependency Graph

One production dependency graph exists. It backs:

- resolver package index
- loader `modulePathMap`
- HAR/HSP maps
- bytecode HAR metadata
- startup/share profile dependency views
- integrated HSP collection
- native SO ownership
- runtime-only ByteCode HAR tracking
- remote/cache package acquisition diagnostics
- watch invalidation

Duplicate graph builders are migration blockers.

## OHPM Acquisition

- `arkdown-ohpm` owns lockfile parsing, global cache layout, offline and
  prefer-offline lookup, registry metadata fetch, tarball integrity verification,
  safe `.tgz` extraction, and cache installation.
- `arkdown-session` owns acquisition orchestration and dependency graph
  classification.
- `arkdown-profile` does not own or re-export OHPM registry/cache/lockfile
  types.

## Watch And Cache

- Watch mode uses session input inventory instead of recursively calling the
  full CLI with ad hoc state.
- Cache keys use `InputFingerprintSet`.
- Fingerprints include source files, profile files, package manifests, module
  manifests, SDK metadata, resource inputs, transform options, and dependency
  graph identity.
- Placeholder hashes are forbidden.

## Verification

- Session fixture tests prove every startup input is represented in the session.
- Read-count tests prove manifests, package files, profile files, and SDK
  metadata are read once per session build.
- Plugin tests use session fixtures.
- No plugin reads `oh-package.json5` or `module.json5` during hooks.
