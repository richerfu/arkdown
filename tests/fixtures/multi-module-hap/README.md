# Dependency profile selection

The entry module depends on two different kinds of source packages:

- `library` is declared in the project's `build-profile.json5`. Its generated
  MergeProfile output is the dependency profile input.
- `source-package` is an installed OHPM dependency, absent from the project's
  module list. It ships `src/main/module.json5`, but no published
  `src/main/module.json`. Its source-only metadata and permissions must not enter
  the entry profile, and its missing `app.minAPIVersion` must not fail the build.

The current SDK's `merge-profile.js` defines this contract through
`MergeProfile.beforeAlwaysAction` → `mergeDependsLibs("module.json")` →
`transformJson2Opts` → `mergeAllHarModuleOpt`. Missing selected files are skipped
with a warning; every selected processed profile still passes
`checkHarApiStatus`, including profiles without metadata or permissions.

The real Hvigor matrix exercises this fixture through
`multi_module_hap_hvigor_parity_report`. Focused profile tests additionally cover
competing source/stale profiles, invalid JSON, missing or excessive minimum API
versions, and product-specific local MergeProfile output.
