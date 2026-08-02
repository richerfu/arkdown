use rolldown_sourcemap::{collapse_sourcemaps, OwnedSourceMap};

use super::api_transform::RollupHiresCompatibilityMap;

#[test]
fn rollup_hires_map_matches_latest_ide_module_info_chain() {
    // DevEco Studio 6.1.1 / ets-loader apiTransform uses MagicString 0.27.0
    // with `hires: true`. Rollup 3.19.1-htrunk1 Link.traceSegment composes
    // this TypeScript emit map into the mapping captured by genAbc.
    let code = "//@ts-noCheck\r\n\
export const __MODULE_NAME__ = __getCurrentModuleName__();\r\n\
export const __BUNDLE_NAME__ = __getCurrentBundleName__();\r\n\
//# sourceMappingURL=ModuleInfo.js.map";
    let raw_map = OwnedSourceMap::from_json_string(
        r#"{"version":3,"sources":["ModuleInfo.ts"],"names":[],"mappings":"AACA,aAAa;AACb,MAAM,CAAC,MAAM,eAAe,GAAW,wBAAwB,EAAE,CAAA;AACjE,MAAM,CAAC,MAAM,eAAe,GAAW,wBAAwB,EAAE,CAAA"}"#,
    )
    .expect("latest TypeScript emit map")
    .into_inner();
    let api_map = RollupHiresCompatibilityMap::new(code, "ModuleInfo.ts", Some(&raw_map)).build();
    let combined = collapse_sourcemaps(&[&raw_map, &api_map]);

    let value: serde_json::Value =
        serde_json::from_str(&combined.to_json_string()).expect("combined source map JSON");
    assert_eq!(
        value["mappings"],
        "AACA,CAAa,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA;\
AACb,CAAA,CAAA,CAAA,CAAA,CAAA,CAAM,CAAC,CAAM,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAe,CAAW,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAwB,EAAE,CAAA,CAAA;\
AACjE,CAAA,CAAA,CAAA,CAAA,CAAA,CAAM,CAAC,CAAM,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAe,CAAW,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAA,CAAwB,EAAE,CAAA,CAAA"
    );
}

#[test]
fn rollup_hires_map_is_utf16_identity_without_a_prior_map() {
    let code = "a😀b\r\nc";
    let map = RollupHiresCompatibilityMap::new(code, "unicode.ts", None).build();
    let tokens = map
        .get_tokens()
        .map(|token| {
            (
                token.get_dst_line(),
                token.get_dst_col(),
                token.get_src_line(),
                token.get_src_col(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            (0, 0, 0, 0),
            (0, 1, 0, 1),
            (0, 2, 0, 2),
            (0, 3, 0, 3),
            (0, 4, 0, 4),
            (0, 5, 0, 5),
            (1, 0, 1, 0),
        ]
    );
}
