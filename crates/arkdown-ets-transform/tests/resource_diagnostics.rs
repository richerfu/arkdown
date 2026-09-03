use std::path::PathBuf;
use std::sync::Arc;

use arkdown_ets_transform::transform::component_meta::ComponentMetadata;
use arkdown_ets_transform::{
    ResourceModuleReference, ResourceReference, ResourceTransformContext, RolldownEtsTransformPlan,
    RolldownEtsTransformer, TransformSourceSnapshot,
};
use rustc_hash::{FxHashMap, FxHashSet};

fn plan(
    references: FxHashMap<String, ResourceReference>,
    context: ResourceTransformContext,
) -> RolldownEtsTransformPlan {
    RolldownEtsTransformPlan::new(
        "com.example",
        "entry",
        PathBuf::from("/project"),
        PathBuf::from("/project/entry/src/main/ets"),
        Vec::new(),
        18,
        references,
        ComponentMetadata::default(),
        TransformSourceSnapshot::default(),
    )
    .with_resource_context(context)
}

fn app_references() -> FxHashMap<String, ResourceReference> {
    FxHashMap::from_iter([(
        "app.string.title".to_string(),
        ResourceReference {
            id: 0x0100_0001,
            runtime_type: 10003,
            bundle_name: "com.example".to_string(),
            module_name: "entry".to_string(),
        },
    )])
}

#[test]
fn reports_source_resource_format_type_and_name_codes_in_sdk_order() {
    let cases = [
        ("$r('two.parts')", "10905332"),
        ("$r('wrong.string.title')", "10903331"),
        ("$r('app.media.logo')", "10903330"),
        ("$r('app.string.missing')", "10903329"),
    ];
    for (source, code) in cases {
        let error = RolldownEtsTransformer::new(Arc::new(plan(
            app_references(),
            ResourceTransformContext::default(),
        )))
        .transform(source, "/project/entry/src/main/ets/Resources.ts")
        .expect_err("invalid static resource must stop the transform");
        assert!(error.to_string().contains(code), "{error}");
    }
}

#[test]
fn template_resource_diagnostic_is_a_warning_and_uses_dynamic_lowering() {
    let output = RolldownEtsTransformer::new(Arc::new(plan(
        app_references(),
        ResourceTransformContext::default(),
    )))
    .transform(
        "$r(`app.media.missing`)",
        "/project/entry/src/main/ets/Resources.ts",
    )
    .unwrap()
    .unwrap();

    assert_eq!(output.warnings.len(), 1);
    assert!(output.warnings[0].to_string().contains("10903330"));
    assert!(output.code.contains("\"id\": -1"), "{}", output.code);
    assert!(
        output.code.contains("`app.media.missing`"),
        "{}",
        output.code
    );
}

#[test]
fn validates_current_module_rawfile_inventory() {
    let context = ResourceTransformContext {
        validate_rawfiles: true,
        rawfiles: Arc::new(FxHashSet::from_iter(["data/config.json".to_string()])),
        ..ResourceTransformContext::default()
    };
    let transformer = RolldownEtsTransformer::new(Arc::new(plan(app_references(), context)));

    let valid = transformer
        .transform(
            "$rawfile('data/config.json')",
            "/project/entry/src/main/ets/Resources.ts",
        )
        .unwrap()
        .unwrap();
    assert!(valid.code.contains("\"id\": 0"), "{}", valid.code);

    let error = transformer
        .transform(
            "$rawfile('missing.json')",
            "/project/entry/src/main/ets/Resources.ts",
        )
        .expect_err("unknown rawfile must fail");
    assert!(error.to_string().contains("10904333"), "{error}");
}

#[test]
fn validates_hsp_resources_and_rawfiles_from_the_native_inventory() {
    let module = ResourceModuleReference::new(
        FxHashSet::from_iter(["string.title".to_string()]),
        FxHashSet::from_iter(["data/shared.json".to_string()]),
        true,
        true,
    );
    let context = ResourceTransformContext {
        validate_resource_modules: true,
        resource_modules: Arc::new(FxHashMap::from_iter([("shared".to_string(), module)])),
        ..ResourceTransformContext::default()
    };
    let transformer = RolldownEtsTransformer::new(Arc::new(plan(app_references(), context)));

    let output = transformer
        .transform(
            concat!(
                "const title = $r('[shared].string.title');\n",
                "const data = $rawfile('[shared].data/shared.json');"
            ),
            "/project/entry/src/main/ets/Resources.ts",
        )
        .unwrap()
        .unwrap();
    assert!(output.warnings.is_empty());
    assert_eq!(output.code.matches("\"moduleName\": \"shared\"").count(), 2);

    let warning = transformer
        .transform(
            "$r('[unknown].string.title')",
            "/project/entry/src/main/ets/Resources.ts",
        )
        .unwrap()
        .unwrap();
    assert!(warning.code.contains("$r('[unknown].string.title')"));
    assert_eq!(warning.warnings.len(), 1);
    assert!(warning.warnings[0].to_string().contains("10903331"));
}

#[test]
fn rejects_existing_but_unsupported_resource_type() {
    let mut references = app_references();
    references.insert(
        "app.profile.pages".to_string(),
        ResourceReference {
            id: 0x0100_0002,
            runtime_type: 0,
            bundle_name: "com.example".to_string(),
            module_name: "entry".to_string(),
        },
    );
    let error = RolldownEtsTransformer::new(Arc::new(plan(
        references,
        ResourceTransformContext::default(),
    )))
    .transform(
        "$r('app.profile.pages')",
        "/project/entry/src/main/ets/Resources.ts",
    )
    .expect_err("profile is present in the table but unsupported by $r");

    assert!(error.to_string().contains("10906334"), "{error}");
}
