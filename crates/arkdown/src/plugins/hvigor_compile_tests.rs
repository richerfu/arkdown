use super::hvigor_compile::{
    HvigorCompileOrderCallbackError, HvigorCompilePluginDescriptor, HvigorCompilePluginOrder,
    HvigorCompilePluginPlanConfig, HvigorCompilePluginPlanner, HvigorNativePluginSlot,
    HvigorPluginAttributeShape, HvigorPluginShape, HvigorPluginShapeValidator,
    HvigorPluginValueKind, HvigorPluginValueShape, RolldownPackagePluginDescriptor,
    RolldownPackagePluginPlanner,
};

fn descriptor(
    source_index: usize,
    name: &str,
    order: HvigorCompilePluginOrder,
) -> HvigorCompilePluginDescriptor {
    HvigorCompilePluginDescriptor {
        source_index,
        name: name.to_string(),
        module_root: None,
        target_name: None,
        order,
    }
}

fn value_shape(
    kind: HvigorPluginValueKind,
    truthy: bool,
    string_value: Option<&str>,
    description: &str,
) -> HvigorPluginValueShape {
    HvigorPluginValueShape {
        kind,
        truthy,
        string_value: string_value.map(str::to_string),
        description: description.to_string(),
    }
}

fn plugin_shape(
    name: HvigorPluginValueShape,
    order: HvigorPluginValueShape,
    attributes: Vec<HvigorPluginAttributeShape>,
) -> HvigorPluginShape {
    HvigorPluginShape {
        source_index: 0,
        source_file: Some("/fixture/plugin.ts".to_string()),
        value: value_shape(HvigorPluginValueKind::Object, true, None, "object"),
        name,
        order,
        attributes,
    }
}

#[test]
fn latest_hvigor_order_callback_observes_full_names_and_maps_native_slots() {
    let planner = HvigorCompilePluginPlanner::new(HvigorCompilePluginPlanConfig {
        compile_mode: Some("esmodule".to_string()),
        ..Default::default()
    });
    let plugins = vec![
        descriptor(0, "last-plugin", HvigorCompilePluginOrder::Last),
        descriptor(1, "first-a", HvigorCompilePluginOrder::First),
        descriptor(2, "first-b", HvigorCompilePluginOrder::First),
        descriptor(3, "after-module-meta", HvigorCompilePluginOrder::Callback),
    ];
    let mut observed = Vec::new();
    let placements = planner
        .plan(plugins, |_, names| {
            observed = names.to_vec();
            Ok(names
                .iter()
                .find_map(|(name, index)| (name == "moduleInfoMetaPlugin").then_some(*index + 1))
                .expect("moduleInfoMetaPlugin") as f64)
        })
        .expect("plugin plan");

    assert!(observed.iter().any(|(name, _)| name == "commonPlugin"));
    assert!(observed.iter().any(|(name, _)| name == "apiTransform"));
    assert_eq!(
        placements
            .iter()
            .map(|placement| (placement.source_index, placement.order_index))
            .collect::<Vec<_>>(),
        vec![(2, 0), (1, 1), (3, 4), (0, 11)]
    );
    assert_eq!(
        placements
            .iter()
            .map(|placement| {
                (
                    placement.source_index,
                    placement.anchor_rank,
                    placement.anchor_sequence,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (2, HvigorNativePluginSlot::DuplicateCheck.rank(), 0),
            (1, HvigorNativePluginSlot::DuplicateCheck.rank(), 1),
            (3, HvigorNativePluginSlot::OhosResolve.rank(), 0),
            (0, HvigorNativePluginSlot::END_RANK, 0),
        ]
    );
}

#[test]
fn latest_hvigor_preserves_index_zero_truthiness_quirk() {
    let planner = HvigorCompilePluginPlanner::new(HvigorCompilePluginPlanConfig::default());
    let plugins = vec![
        descriptor(0, "first", HvigorCompilePluginOrder::First),
        descriptor(1, "first", HvigorCompilePluginOrder::Callback),
    ];
    let mut observed = Vec::new();
    let error = planner
        .plan(plugins, |_, names| {
            observed = names.to_vec();
            Err(HvigorCompileOrderCallbackError::InvalidReturn(
                "test".to_string(),
            ))
        })
        .expect_err("callback diagnostic");

    assert_eq!(error.code, "INVALID_PLUGIN_EXPORT");
    assert_eq!(
        observed.iter().filter(|(name, _)| name == "first").count(),
        1
    );
    assert_eq!(
        observed
            .iter()
            .find(|(name, _)| name == "first")
            .map(|(_, index)| *index),
        Some(0)
    );
}

#[test]
fn duplicate_names_are_checked_per_module_target_scope() {
    let planner = HvigorCompilePluginPlanner::new(HvigorCompilePluginPlanConfig::default());
    let plugins = vec![
        HvigorCompilePluginDescriptor {
            module_root: Some("module-a".to_string()),
            ..descriptor(0, "same-name", HvigorCompilePluginOrder::Last)
        },
        HvigorCompilePluginDescriptor {
            module_root: Some("module-b".to_string()),
            ..descriptor(1, "same-name", HvigorCompilePluginOrder::Last)
        },
    ];
    let placements = planner
        .plan(plugins, |_, _| unreachable!("no callbacks"))
        .expect("independent scope plans");

    assert_eq!(placements.len(), 2);
    assert_eq!(placements[0].order_index, placements[1].order_index);
}

#[test]
fn release_terser_is_the_only_native_minify_slot() {
    let debug = HvigorCompilePluginPlanner::new(HvigorCompilePluginPlanConfig::default())
        .plan(
            vec![descriptor(0, "last", HvigorCompilePluginOrder::Last)],
            |_, _| unreachable!("no callbacks"),
        )
        .expect("debug plan");
    let release = HvigorCompilePluginPlanner::new(HvigorCompilePluginPlanConfig {
        mode: Some("release".to_string()),
        ..Default::default()
    })
    .plan(
        vec![descriptor(0, "last", HvigorCompilePluginOrder::Last)],
        |_, _| unreachable!("no callbacks"),
    )
    .expect("release plan");

    assert_eq!(release[0].order_index, debug[0].order_index + 1);
    assert_eq!(debug[0].anchor_rank, HvigorNativePluginSlot::END_RANK);
    assert_eq!(release[0].anchor_rank, HvigorNativePluginSlot::END_RANK);
}

#[test]
fn explicit_package_callbacks_use_safe_own_key_records() {
    let plugins = vec![
        RolldownPackagePluginDescriptor {
            source_index: 0,
            name: "alpha".to_string(),
            has_order_callback: false,
        },
        RolldownPackagePluginDescriptor {
            source_index: 1,
            name: "alpha".to_string(),
            has_order_callback: false,
        },
        RolldownPackagePluginDescriptor {
            source_index: 2,
            name: "ordered".to_string(),
            has_order_callback: true,
        },
    ];
    let mut observed = Vec::new();
    let placements = RolldownPackagePluginPlanner::plan(plugins, |_, names| {
        observed = names.to_vec();
        Ok(1.0)
    })
    .expect("explicit plugin order");

    assert_eq!(
        observed,
        vec![("alpha".to_string(), 0), ("alpha_1".to_string(), 1)]
    );
    assert_eq!(
        placements
            .iter()
            .map(|placement| (placement.source_index, placement.callback_resolved))
            .collect::<Vec<_>>(),
        vec![(0, false), (2, true), (1, false)]
    );
}

#[test]
fn latest_hvigor_compile_plugin_shape_validation_is_rust_owned() {
    let name = value_shape(
        HvigorPluginValueKind::String,
        true,
        Some("fixture"),
        "\"fixture\"",
    );
    let undefined = value_shape(HvigorPluginValueKind::Undefined, false, None, "undefined");
    let invalid_hook = plugin_shape(
        name.clone(),
        undefined.clone(),
        vec![HvigorPluginAttributeShape {
            name: "transform".to_string(),
            value: value_shape(
                HvigorPluginValueKind::String,
                true,
                Some("invalid"),
                "\"invalid\"",
            ),
        }],
    );
    let outcome = HvigorPluginShapeValidator::validate_compile(&invalid_hook);
    assert_eq!(
        outcome.error.expect("invalid hook diagnostic").code,
        "PLUGIN_NAME_WITH_UNKNOWN_HOOK"
    );

    let ordered_handler = plugin_shape(
        name.clone(),
        undefined.clone(),
        vec![HvigorPluginAttributeShape {
            name: "transform".to_string(),
            value: value_shape(HvigorPluginValueKind::Object, true, None, "object"),
        }],
    );
    let outcome = HvigorPluginShapeValidator::validate_compile(&ordered_handler);
    assert_eq!(
        outcome.error.expect("ordered handler diagnostic").code,
        "PLUGIN_NAME_WITH_UNKNOWN_HOOK"
    );

    let arkdown_only_hook = plugin_shape(
        name.clone(),
        undefined.clone(),
        vec![HvigorPluginAttributeShape {
            name: "beforeBuild".to_string(),
            value: value_shape(HvigorPluginValueKind::Function, true, None, "function"),
        }],
    );
    let outcome = HvigorPluginShapeValidator::validate_compile(&arkdown_only_hook);
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].code, "UNKNOWN_PLUGIN_ATTR");

    let unknown_order = plugin_shape(
        name,
        value_shape(
            HvigorPluginValueKind::String,
            true,
            Some("middle"),
            "\"middle\"",
        ),
        Vec::new(),
    );
    let outcome = HvigorPluginShapeValidator::validate_compile(&unknown_order);
    assert_eq!(
        outcome.error.expect("invalid order diagnostic").code,
        "PLUGIN_NAME_WITH_UNKNOWN_ORDER"
    );
}

#[test]
fn explicit_package_shape_validation_returns_warnings_and_typed_errors() {
    let shape = plugin_shape(
        value_shape(HvigorPluginValueKind::Undefined, false, None, "undefined"),
        value_shape(
            HvigorPluginValueKind::String,
            true,
            Some("middle"),
            "\"middle\"",
        ),
        vec![HvigorPluginAttributeShape {
            name: "cleanUp".to_string(),
            value: value_shape(HvigorPluginValueKind::Function, true, None, "function"),
        }],
    );
    let outcome = HvigorPluginShapeValidator::validate_package(&shape);

    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].code, "PLUGIN_NAME_NOT_EXIST");
    let error = outcome.error.expect("invalid package order");
    assert_eq!(error.code, "PLUGIN_NAME_WITH_UNKNOWN_ORDER");
    assert!(error.message.contains("Expected 'first', 'last'"));
}
