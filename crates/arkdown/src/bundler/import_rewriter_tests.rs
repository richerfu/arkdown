use super::*;
use crate::chunk_module_request::HvigorChunkImportMap;
use crate::plan::{BundleRuntimePackageContext, BundleRuntimePackageInfo};

#[test]
fn compile_arkts_rewrites_lazy_native_import_in_ts_dependency_like_sdk() {
    let module_id = "/workspace/oh_modules/mtmediaplayer/src/main/ts/MTMediaPlayerCache.ts";
    let source = r#"import lazy nativePlayer from "libaemonplayer.so";
nativePlayer.cacheInit();"#;

    assert_eq!(
        HvigorModuleRequestSpecifiers::new(source, module_id).collect(),
        vec!["libaemonplayer.so".to_string()],
        "SDK ModuleSourceFile.processTransformedTsModuleRequest recognizes lazy imports in .ts"
    );

    let mut packages = FxHashMap::default();
    packages.insert(
        "libaemonplayer.so".to_string(),
        BundleRuntimePackageContext {
            package_name: "libaemonplayer.so".to_string(),
            is_so: true,
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };
    let normalized_request = HvigorBytecodeHarImports::new(&runtime_packages, &[], true)
        .ohm_url_for_request("libaemonplayer.so")
        .expect("native package context lookup")
        .expect("native package context must produce an OHM URL");
    let imports =
        HvigorChunkImportMap::from_iter([("libaemonplayer.so".to_string(), normalized_request)]);
    let rewritten = HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

    assert!(
        rewritten.contains("import lazy nativePlayer from \"@normalized:Y&&&libaemonplayer.so&\";"),
        "CompileArkTS must retain lazy semantics and rewrite native requests: {rewritten}"
    );
}

#[test]
fn module_source_rewriter_preserves_commonjs_relative_require_like_hvigor() {
    let module_id = "/workspace/oh_modules/protobufjs/src/converter.js";
    let source = r#"var Enum = require("./enum");
import staticValue from "./static";
const lazyValue = import("./lazy");"#;
    let imports = HvigorChunkImportMap::from_iter([
        (
            "./enum".to_string(),
            "@package:pkg_modules/protobufjs/src/enum".to_string(),
        ),
        (
            "./static".to_string(),
            "@package:pkg_modules/protobufjs/src/static".to_string(),
        ),
        (
            "./lazy".to_string(),
            "@package:pkg_modules/protobufjs/src/lazy".to_string(),
        ),
    ]);

    assert_eq!(
        HvigorModuleRequestSpecifiers::new(source, module_id).collect(),
        vec!["./static".to_string(), "./lazy".to_string()],
        "latest ets-loader ModuleSourceFile only visits static ESM and import() requests"
    );
    let rewritten = HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

    assert!(
        rewritten.contains("require(\"./enum\")"),
        "CommonJS relative require must retain runtime resolution: {rewritten}"
    );
    assert!(
        !rewritten.contains("@package:pkg_modules/protobufjs/src/enum"),
        "CommonJS require must not become an es2abc package request: {rewritten}"
    );
    assert!(
        rewritten.contains("@package:pkg_modules/protobufjs/src/static")
            && rewritten.contains("@package:pkg_modules/protobufjs/src/lazy"),
        "hvigor still rewrites static ESM and dynamic import requests: {rewritten}"
    );
}

#[test]
fn module_source_rewriter_preserves_js_request_quote_style_like_latest_hvigor() {
    let module_id = "/workspace/oh_modules/@ohos/lottie/src/main/js/modules/full.js";
    let source = r#"import lottie from './main';
export { registerRenderer } from "../renderers/main";
const lazyRenderer = import('./renderers/lazy');"#;
    let imports = HvigorChunkImportMap::from_iter([
        (
            "./main".to_string(),
            "@package:pkg_modules/@ohos/lottie/src/main/js/modules/main".to_string(),
        ),
        (
            "../renderers/main".to_string(),
            "@package:pkg_modules/@ohos/lottie/src/main/js/renderers/main".to_string(),
        ),
        (
            "./renderers/lazy".to_string(),
            "@package:pkg_modules/@ohos/lottie/src/main/js/modules/renderers/lazy".to_string(),
        ),
    ]);

    let rewritten = HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

    assert_eq!(
        rewritten,
        r#"import lottie from '@package:pkg_modules/@ohos/lottie/src/main/js/modules/main';
export { registerRenderer } from "@package:pkg_modules/@ohos/lottie/src/main/js/renderers/main";
const lazyRenderer = import('@package:pkg_modules/@ohos/lottie/src/main/js/modules/renderers/lazy');"#,
        "latest ets-loader module_source_file.js::processJsModuleRequest reuses its captured quote delimiter"
    );
}

#[test]
fn module_source_rewriter_uses_generated_double_quotes_for_transformed_ts_like_hvigor() {
    let module_id = "/workspace/entry/src/main/ets/pages/MainPage.ets";
    let source = r#"import page from './Page';
export { ViewModel } from '../model/ViewModel';
const lazyPage = import('./lazy/Page');"#;
    let imports = HvigorChunkImportMap::from_iter([
        (
            "./Page".to_string(),
            "@bundle:com.example/entry/ets/pages/Page".to_string(),
        ),
        (
            "../model/ViewModel".to_string(),
            "@bundle:com.example/entry/ets/model/ViewModel".to_string(),
        ),
        (
            "./lazy/Page".to_string(),
            "@bundle:com.example/entry/ets/pages/lazy/Page".to_string(),
        ),
    ]);

    let rewritten = HvigorModuleSourceImportRewriter::new(source, module_id, &imports).rewrite();

    assert_eq!(
        rewritten,
        r#"import page from "@bundle:com.example/entry/ets/pages/Page";
export { ViewModel } from "@bundle:com.example/entry/ets/model/ViewModel";
const lazyPage = import("@bundle:com.example/entry/ets/pages/lazy/Page");"#,
        "latest ets-loader ModuleSourceFile.processTransformedTsModuleRequest uses ts.factory.createStringLiteral for import, export, and import() requests"
    );
}
