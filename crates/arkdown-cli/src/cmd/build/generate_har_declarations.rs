//! K4 — Generate basic `.d.ts`/`.d.ets` declaration stubs for source HAR packages.
//!
//! Scans the HAR entry file and its direct exports to generate declaration files
//! that downstream consumers can use for type resolution.  The output is not a
//! full TypeScript type-checked declaration (that requires `tsc`/`ets_checker`),
//! but a best-effort `export * from` re-export stub that enables module
//! resolution for simple project layouts.
//!
//! Matches hvigor's `processDeclarationEntry()` for source HAR packages:
//! - Reads `oh-package.json5#main` to find the entry
//! - Scans the entry for exported symbols
//! - Emits `export declare …` stubs in `index.d.ts`
//! - Copies to `index.d.ets` as a duplicate (ArkTS-specific declaration)

use std::path::{Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;

/// Generate basic declaration files for a source HAR.
///
/// Writes:
/// - `intermediates/har/default/index.d.ts`
/// - `intermediates/har/default/index.d.ets`
///
/// The declaration stubs re-export from each `.ets`/`.ts` file in the HAR.
pub fn generate_har_declarations(
    module_root: &Path,
    source_dir: &Path,
    module_name: &str,
) -> Result<()> {
    let out_dir = module_root.join("build/default/intermediates/har/default");
    std::fs::create_dir_all(&out_dir)?;

    let mut declarations = String::new();
    declarations.push_str(&format!(
        "// Auto-generated declaration stubs for HAR module: {}\n",
        module_name
    ));

    // Find the main entry file from oh-package.json5.
    let pkg_json_path = module_root.join("oh-package.json5");
    let main_entry = if let Ok(content) = std::fs::read_to_string(&pkg_json_path) {
        #[derive(serde::Deserialize)]
        struct PkgEntry {
            #[serde(default)]
            main: Option<String>,
        }
        json5::from_str::<PkgEntry>(&content)
            .ok()
            .and_then(|p| p.main)
            .filter(|m| !m.is_empty())
    } else {
        None
    };

    // If we have a main entry, re-export from it.
    if let Some(main) = &main_entry {
        let main_no_ext = main
            .trim_start_matches("./")
            .trim_end_matches(".ets")
            .trim_end_matches(".ts");
        declarations.push_str(&format!("export * from '{}';\n", main_no_ext));
    }

    // Also re-export from all top-level .ets/.ts files in src/main/ets/.
    if source_dir.exists() {
        let top_level_files: Vec<PathBuf> = WalkDir::new(source_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map_or(false, |x| x == "ets" || x == "ts")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        for file_path in &top_level_files {
            let file_name = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();

            // Skip files already covered by main entry.
            let rel = format!("./src/main/ets/{}", file_name);
            if main_entry
                .as_ref()
                .map_or(false, |m| m.contains(file_name))
            {
                continue;
            }
            declarations.push_str(&format!("export * from '{}';\n", rel));
        }
    }

    // Write index.d.ts and index.d.ets.
    let dts_path = out_dir.join("index.d.ts");
    std::fs::write(&dts_path, &declarations)?;

    let dets_path = out_dir.join("index.d.ets");
    std::fs::write(&dets_path, &declarations)?;

    Ok(())
}
