use std::path::PathBuf;

use arkdown_core::{ArktsLanguageVersion, ModuleSource, ModuleType, ResolvedId};
use arkdown_ets_transform::{
    error::EtsTransformError,
    transform::{component_meta::ComponentMetadata, ArktsTransformer, TransformOptions},
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceContractManifest {
    schema_version: u32,
    arkts_version: String,
    cases: Vec<SourceContractCase>,
}

#[derive(Deserialize)]
struct SourceContractCase {
    id: String,
    owner: String,
    file: String,
    source: String,
    expect: SourceContractExpectation,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceContractExpectation {
    error_code: Option<String>,
    #[serde(default)]
    contains: Vec<String>,
}

struct SourceContractRunner {
    manifest: SourceContractManifest,
    options: TransformOptions,
}

impl SourceContractRunner {
    fn load() -> Self {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/source-parity/arkts11.json");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read source contract {}: {error}", path.display()));
        let manifest = serde_json::from_str(&source)
            .unwrap_or_else(|error| panic!("parse source contract {}: {error}", path.display()));
        let loader = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("ets-loader");
        let component_metadata =
            ComponentMetadata::from_ets_loader_path(&loader).unwrap_or_else(|error| {
                panic!("load fixture SDK metadata {}: {error}", loader.display())
            });
        Self {
            manifest,
            options: TransformOptions::default()
                .with_component_metadata(std::sync::Arc::new(component_metadata)),
        }
    }

    fn run(self) {
        assert_eq!(self.manifest.schema_version, 1);
        assert_eq!(self.manifest.arkts_version, "1.1");
        assert_eq!(ArktsLanguageVersion::default(), ArktsLanguageVersion::V1_1);

        let mut ids = std::collections::BTreeSet::new();
        let options = self.options;
        for case in self.manifest.cases {
            assert!(
                ids.insert(case.id.clone()),
                "duplicate case id: {}",
                case.id
            );
            assert!(
                case.owner.starts_with("compiler/"),
                "case {} must cite its new-source owner, got {}",
                case.id,
                case.owner
            );
            Self::run_case(&options, case);
        }
    }

    fn run_case(options: &TransformOptions, case: SourceContractCase) {
        let source = ModuleSource {
            id: ResolvedId {
                id: case.file,
                external: false,
                module_type: ModuleType::Ets,
                ..ResolvedId::default()
            },
            content: case.source,
            source_map: None,
        };
        let result = ArktsTransformer::with_options(&source, options).transform();
        if let Some(expected_code) = case.expect.error_code {
            match result {
                Err(EtsTransformError::CodedDiagnostic { code, .. }) => assert_eq!(
                    code, expected_code,
                    "source contract {} ({})",
                    case.id, case.owner
                ),
                Err(error) => panic!(
                    "source contract {} ({}) expected code {}, got {error:?}",
                    case.id, case.owner, expected_code
                ),
                Ok(output) => panic!(
                    "source contract {} ({}) expected code {}, got output:\n{}",
                    case.id, case.owner, expected_code, output
                ),
            }
            return;
        }

        let output = result.unwrap_or_else(|error| {
            panic!(
                "source contract {} ({}) unexpectedly failed: {error:?}",
                case.id, case.owner
            )
        });
        for expected in case.expect.contains {
            assert!(
                output.contains(&expected),
                "source contract {} ({}) missing {:?}:\n{}",
                case.id,
                case.owner,
                expected,
                output
            );
        }
    }
}

#[test]
fn arkts11_source_contract_manifest_executes_against_oxc_pipeline() {
    SourceContractRunner::load().run();
}
