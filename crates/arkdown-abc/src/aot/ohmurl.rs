//! OHM URL manipulation utilities — ported from upstream `commonUtil.cpp/h`.
//!
//! These functions rewrite OHM URL strings in static and dynamic imports of
//! precompiled ABC programs.  They are used by `AbcInputProgramCompiler` when
//! `compileContextInfo.needModifyRecord`, `updateVersionInfo`, or `bundleName`
//! changes are requested.
//!
//! ## OHM URL format (non-SO, normalized):
//! ```text
//! @normalized:{N|Y}&[module_name]&[bundle_name]&{<pkgName>|<@scope/pkg>}/{path}&[version]
//! ```
//!
//! Position indices (0-based split by `&`):
//! - 0: `@normalized:N` or `@normalized:Y`
//! - 1: module name
//! - 2: bundle name
//! - 3: normalized import path (pkgName/path)
//! - 4: version
//!
//! ## Upstream reference
//! - `es2panda/util/commonUtil.h` — `VisitStaticImports`, `VisitDyanmicImports`
//! - `es2panda/util/commonUtil.cpp` — `UpdatePackageVersionIfNeeded`,
//!   `UpdateBundleNameIfNeeded`, `UpdatePackageNameIfNeeded`,
//!   `GetPkgNameFromNormalizedOhmurl`, `GetRecordNameFromNormalizedOhmurl`

use std::collections::BTreeSet;

use crate::aot::options::PkgInfo;

const NORMALIZED_PREFIX: &str = "@normalized:";
const NORMALIZED_NOT_SO: &str = "@normalized:N";
const SEPARATOR: char = '&';
const SLASH: char = '/';
const AT: char = '@';

/// Position indices in a split OHM URL (`split('&')` result).
const _MODULE_NAME_POS: usize = 1;
const BUNDLE_NAME_POS: usize = 2;
const NORMALIZED_IMPORT_POS: usize = 3;
const VERSION_POS: usize = 4;

// ── URL component extraction ──────────────────────────────────────────────────

/// Extract the package name from a normalized import path.
///
/// Upstream `GetPkgNameFromNormalizedImport` in `commonUtil.cpp`.
///
/// Examples:
/// - `"entry/src/main/ets/pages/Index"` → `"entry"`
/// - `"@scope/pkg/src/Index"` → `"@scope/pkg"`
pub fn pkg_name_from_normalized_import(normalized_import: &str) -> Option<&str> {
    let mut pos = normalized_import.find(SLASH)?;
    if normalized_import.starts_with(AT) {
        // Scoped package: skip first segment, find second slash.
        pos = normalized_import[pos + 1..].find(SLASH)? + pos + 1;
    }
    Some(&normalized_import[..pos])
}

/// Extract the package name from a normalized OHM URL.
///
/// Upstream `GetPkgNameFromNormalizedOhmurl` in `commonUtil.cpp`.
pub fn pkg_name_from_normalized_ohmurl(ohmurl: &str) -> Option<&str> {
    let items: Vec<&str> = ohmurl.split(SEPARATOR).collect();
    let normalized_import = items.get(NORMALIZED_IMPORT_POS)?;
    pkg_name_from_normalized_import(normalized_import)
}

/// Extract the record name from a normalized OHM URL.
///
/// Record name format: `"<bundleName>&<normalizedImport>&<version>"`.
///
/// Upstream `GetRecordNameFromNormalizedOhmurl` in `commonUtil.cpp`.
pub fn record_name_from_normalized_ohmurl(ohmurl: &str) -> Option<String> {
    let items: Vec<&str> = ohmurl.split(SEPARATOR).collect();
    if items.len() <= VERSION_POS {
        return None;
    }
    Some(format!(
        "{}{SEPARATOR}{}{SEPARATOR}{}",
        items[BUNDLE_NAME_POS],
        items[NORMALIZED_IMPORT_POS],
        items[VERSION_POS],
    ))
}

// ── URL transformation functions ─────────────────────────────────────────────

/// Update the version in a normalized OHM URL if the package is in `pkg_context_info`.
///
/// Upstream `UpdatePackageVersionIfNeeded` in `commonUtil.cpp`:
/// 1. Only processes `@normalized:N` URLs (non-SO).
/// 2. Extracts the package name from the normalized import position.
/// 3. Looks up the package in `pkg_context_info`.
/// 4. Replaces the version (everything after the last `&`) with the new version.
pub fn update_package_version_if_needed<'a>(
    ohmurl: &'a str,
    pkg_context_info: &std::collections::BTreeMap<Box<str>, PkgInfo>,
) -> std::borrow::Cow<'a, str> {
    if !ohmurl.starts_with(NORMALIZED_NOT_SO) {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    let Some(pkg_name) = pkg_name_from_normalized_ohmurl(ohmurl) else {
        return std::borrow::Cow::Borrowed(ohmurl);
    };
    let Some(pkg_info) = pkg_context_info.get(pkg_name) else {
        return std::borrow::Cow::Borrowed(ohmurl);
    };
    // Replace version: everything after the last `&`.
    let Some(version_start) = ohmurl.rfind(SEPARATOR) else {
        return std::borrow::Cow::Borrowed(ohmurl);
    };
    let mut updated = ohmurl[..=version_start].to_string();
    updated.push_str(&pkg_info.version);
    std::borrow::Cow::Owned(updated)
}

/// Update the bundle name in a normalized OHM URL.
///
/// Upstream `UpdateBundleNameIfNeeded` in `commonUtil.cpp`:
/// 1. Only processes `@normalized:` URLs.
/// 2. Only updates when `bundle_name` (at pos 2) is currently EMPTY.
/// 3. Skips external HSP packages (`externalPkgNames`).
pub fn update_bundle_name_if_needed<'a>(
    ohmurl: &'a str,
    bundle_name: &str,
    external_pkg_names: &BTreeSet<Box<str>>,
) -> std::borrow::Cow<'a, str> {
    if !ohmurl.starts_with(NORMALIZED_PREFIX) {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    let mut items: Vec<&str> = ohmurl.split(SEPARATOR).collect();
    if items.len() <= VERSION_POS {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    // Only update when bundle_name position is currently empty.
    if !items[BUNDLE_NAME_POS].is_empty() {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    // Skip external (HSP) packages.
    if is_external_pkg_name(ohmurl, external_pkg_names) {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    items[BUNDLE_NAME_POS] = bundle_name;
    std::borrow::Cow::Owned(items.join(&SEPARATOR.to_string()))
}

/// Update the package name in a normalized OHM URL.
///
/// Upstream `UpdatePackageNameIfNeeded` in `commonUtil.cpp`:
/// Used when `modifiedPkgName = "srcPkg:dstPkg"` (colon-separated pair).
/// Replaces the matching package name in the normalized import path.
pub fn update_package_name_if_needed<'a>(
    ohmurl: &'a str,
    modified_pkg_name: &str,
) -> std::borrow::Cow<'a, str> {
    if !ohmurl.starts_with(NORMALIZED_NOT_SO) {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    let Some(pkg_name) = pkg_name_from_normalized_ohmurl(ohmurl) else {
        return std::borrow::Cow::Borrowed(ohmurl);
    };
    // modified_pkg_name format: "srcPkg:dstPkg"
    let Some((src_pkg, dst_pkg)) = modified_pkg_name.split_once(':') else {
        return std::borrow::Cow::Borrowed(ohmurl);
    };
    if pkg_name != src_pkg {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    // Find the position of the pkg name in the normalized import part and replace.
    let items: Vec<&str> = ohmurl.split(SEPARATOR).collect();
    if items.len() <= NORMALIZED_IMPORT_POS {
        return std::borrow::Cow::Borrowed(ohmurl);
    }
    let normalized_import = items[NORMALIZED_IMPORT_POS];
    let Some(pos) = normalized_import.find(pkg_name) else {
        return std::borrow::Cow::Borrowed(ohmurl);
    };
    let mut new_import = normalized_import[..pos].to_string();
    new_import.push_str(dst_pkg);
    new_import.push_str(&normalized_import[pos + pkg_name.len()..]);

    let mut new_items: Vec<String> = items.iter().map(|s| s.to_string()).collect();
    new_items[NORMALIZED_IMPORT_POS] = new_import;
    std::borrow::Cow::Owned(new_items.join(&SEPARATOR.to_string()))
}

// ── Import visitor utilities ─────────────────────────────────────────────────

/// Return `true` if the OHM URL belongs to an external (HSP) package.
///
/// Upstream `IsExternalPkgNames` in `commonUtil.h`.
pub fn is_external_pkg_name(ohmurl: &str, external_pkg_names: &BTreeSet<Box<str>>) -> bool {
    let Some(pkg_name) = pkg_name_from_normalized_ohmurl(ohmurl) else {
        return false;
    };
    external_pkg_names.contains(pkg_name)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_pkg_info(version: &str) -> PkgInfo {
        PkgInfo {
            package_name: Box::from(""),
            version: Box::from(version),
        }
    }

    #[test]
    fn update_package_version_replaces_version_in_normalized_url() {
        let ohmurl = "@normalized:N&entry&bundleName&entry/src/main/ets/pages/Index&1.0.0";
        let mut pkg_ctx: BTreeMap<Box<str>, PkgInfo> = BTreeMap::new();
        pkg_ctx.insert(Box::from("entry"), make_pkg_info("2.0.1"));

        let updated = update_package_version_if_needed(ohmurl, &pkg_ctx);
        assert_eq!(
            updated.as_ref(),
            "@normalized:N&entry&bundleName&entry/src/main/ets/pages/Index&2.0.1",
            "version must be replaced with pkg_context_info value"
        );
    }

    #[test]
    fn update_package_version_skips_non_normalized_url() {
        let ohmurl = "@bundle:com.example/entry/ets/pages/Index";
        let pkg_ctx: BTreeMap<Box<str>, PkgInfo> = BTreeMap::new();
        let updated = update_package_version_if_needed(ohmurl, &pkg_ctx);
        assert_eq!(updated.as_ref(), ohmurl, "non-normalized URLs must not be modified");
    }

    #[test]
    fn update_package_version_skips_unknown_package() {
        let ohmurl = "@normalized:N&entry&bundle&unknown_pkg/index&1.0.0";
        let mut pkg_ctx: BTreeMap<Box<str>, PkgInfo> = BTreeMap::new();
        pkg_ctx.insert(Box::from("other_pkg"), make_pkg_info("2.0.0"));
        let updated = update_package_version_if_needed(ohmurl, &pkg_ctx);
        assert_eq!(updated.as_ref(), ohmurl, "unknown package must not be modified");
    }

    #[test]
    fn update_bundle_name_fills_empty_bundle_name() {
        let ohmurl = "@normalized:N&entry&&entry/src/index&1.0.0";
        let external: BTreeSet<Box<str>> = BTreeSet::new();
        let updated = update_bundle_name_if_needed(ohmurl, "com.example.app", &external);
        assert!(
            updated.contains("com.example.app"),
            "empty bundle name must be filled; got: {}",
            updated
        );
    }

    #[test]
    fn update_bundle_name_skips_already_filled_bundle_name() {
        let ohmurl = "@normalized:N&entry&existing.bundle&entry/src/index&1.0.0";
        let external: BTreeSet<Box<str>> = BTreeSet::new();
        let updated = update_bundle_name_if_needed(ohmurl, "new.bundle", &external);
        assert_eq!(
            updated.as_ref(),
            ohmurl,
            "non-empty bundle name must not be overwritten"
        );
    }

    #[test]
    fn pkg_name_from_ohmurl_extracts_entry_package() {
        let ohmurl = "@normalized:N&m&b&entry/src/main/ets/pages/Index&1.0.0";
        assert_eq!(
            pkg_name_from_normalized_ohmurl(ohmurl),
            Some("entry"),
        );
    }

    #[test]
    fn pkg_name_from_ohmurl_extracts_scoped_package() {
        let ohmurl = "@normalized:N&m&b&@scope/pkg/src/index&1.0.0";
        assert_eq!(
            pkg_name_from_normalized_ohmurl(ohmurl),
            Some("@scope/pkg"),
        );
    }

    #[test]
    fn record_name_from_ohmurl_matches_upstream_format() {
        let ohmurl = "@normalized:N&module&bundleName&entry/src/main/ets/pages/Index&1.0.0";
        let record_name = record_name_from_normalized_ohmurl(ohmurl).unwrap();
        assert_eq!(
            record_name,
            "bundleName&entry/src/main/ets/pages/Index&1.0.0",
            "record name must be bundleName&normalizedImport&version"
        );
    }
}
