use arkdown_core::ArktsLanguageVersion;
use arkdown_ets_transform::HarDeclarationGenerator;

#[test]
fn source_har_declarations_preserve_public_signatures_and_tree() {
    let temp = tempfile::tempdir().expect("create module root");
    let source_dir = temp.path().join("src/main/ets");
    std::fs::create_dir_all(source_dir.join("model")).expect("create source tree");
    std::fs::write(
        source_dir.join("model/User.ets"),
        r#"
      /** Public user model. */
      export interface User { id: number; name: string }
      export class UserStore {
        constructor(public readonly users: User[]) {}
        find(id: number): User | undefined { return this.users.find(user => user.id === id); }
      }
    "#,
    )
    .expect("write ArkTS source");
    let declarations_dir = temp.path().join("etsFortgz");

    HarDeclarationGenerator::new(
        temp.path(),
        &source_dir,
        declarations_dir.clone(),
        "library",
    )
    .generate()
    .expect("generate declarations");

    let declaration =
        std::fs::read_to_string(declarations_dir.join("library/src/main/ets/model/User.d.ets"))
            .expect("read generated declaration");
    assert!(declaration.contains("interface User"), "{declaration}");
    assert!(declaration.contains("id: number"), "{declaration}");
    assert!(declaration.contains("name: string"), "{declaration}");
    assert!(declaration.contains("class UserStore"), "{declaration}");
    assert!(
        declaration.contains("find(id: number): User | undefined"),
        "{declaration}"
    );
    assert!(!declaration.contains("unknown"), "{declaration}");
    assert!(declaration
        .as_bytes()
        .windows(2)
        .any(|bytes| bytes == b"\r\n"));
}

#[test]
fn invalid_source_har_declaration_input_fails_the_build() {
    let temp = tempfile::tempdir().expect("create module root");
    let source_dir = temp.path().join("src/main/ets");
    std::fs::create_dir_all(&source_dir).expect("create source tree");
    std::fs::write(source_dir.join("Invalid.ets"), "export interface {")
        .expect("write invalid source");

    let error = HarDeclarationGenerator::new(
        temp.path(),
        &source_dir,
        temp.path().join("etsFortgz"),
        "library",
    )
    .generate()
    .expect_err("invalid source must fail declaration generation")
    .to_string();
    assert!(
        error.contains("parse Source HAR declaration input"),
        "{error}"
    );
}

#[test]
fn hsp_declarations_preserve_module_source_tree_for_package_shared_har() {
    let temp = tempfile::tempdir().expect("create module root");
    let source_dir = temp.path().join("src/main/ets");
    std::fs::create_dir_all(&source_dir).expect("create source tree");
    std::fs::write(
        source_dir.join("Index.ets"),
        "export function sharedValue(): number { return 1; }",
    )
    .expect("write source");
    let declarations_dir = temp.path().join("etsFortgz");

    HarDeclarationGenerator::new(temp.path(), &source_dir, declarations_dir.clone(), "shared")
        .generate()
        .expect("generate declarations");

    assert!(declarations_dir
        .join("shared/src/main/ets/Index.d.ets")
        .is_file());
}

#[test]
fn static_arkts_har_declarations_use_oxc_static_struct_emission() {
    let temp = tempfile::tempdir().expect("create module root");
    let source_dir = temp.path().join("src/main/ets");
    std::fs::create_dir_all(&source_dir).expect("create source tree");
    std::fs::write(
        source_dir.join("StaticCard.ets"),
        r#"
      import { Component, Param, Text } from '@ohos.arkui.component'
      @Component
      export struct StaticCard {
        @Param title: string = ''
        build(): void { Text(this.title) }
      }
    "#,
    )
    .expect("write static ArkTS source");
    let declarations_dir = temp.path().join("etsFortgz");

    HarDeclarationGenerator::new(
        temp.path(),
        &source_dir,
        declarations_dir.clone(),
        "library",
    )
    .with_arkts_language_version(ArktsLanguageVersion::V1_2)
    .generate()
    .expect("generate static declarations");

    let declaration =
        std::fs::read_to_string(declarations_dir.join("library/src/main/ets/StaticCard.d.ets"))
            .expect("read static declaration");
    assert!(declaration.contains("@Component"), "{declaration}");
    assert!(
        declaration.contains("declare struct StaticCard"),
        "{declaration}"
    );
    assert!(declaration.contains("@Param"), "{declaration}");
    assert!(declaration.contains("title: string"), "{declaration}");
    assert!(!declaration.contains("Text(this.title)"), "{declaration}");
}

/// Latest IDE `ets-loader::generateSourceFilesInHar` sends both source and
/// generated `ModuleInfo.ts` through TypeScript's declaration emitter for a
/// ByteCode HAR. The declarations are archived as exact CRLF/four-space bytes.
#[test]
fn bytecode_har_declarations_match_sdk_generated_entries_and_printer() {
    let temp = tempfile::tempdir().expect("create module root");
    let source_dir = temp.path().join("src/main/ets");
    let module_info = temp
        .path()
        .join("build/default/generated/profile/default/ModuleInfo.ts");
    std::fs::create_dir_all(&source_dir).expect("create source tree");
    std::fs::create_dir_all(module_info.parent().expect("generated profile parent"))
        .expect("create generated profile tree");
    std::fs::write(
        source_dir.join("Index.ets"),
        concat!(
            "export function formatGreeting(name: string): string {\n",
            "  return `Hello, ${name}!`;\n",
            "}\n\n",
            "export class Counter {\n",
            "  private value: number = 0;\n\n",
            "  increment(): number {\n",
            "    this.value += 1;\n",
            "    return this.value;\n",
            "  }\n",
            "}\n"
        ),
    )
    .expect("write ByteCode HAR source");
    std::fs::write(
        &module_info,
        concat!(
            "\n//@ts-noCheck\n",
            "export const __MODULE_NAME__: string = __getCurrentModuleName__()\n",
            "export const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n"
        ),
    )
    .expect("write generated ModuleInfo");
    let declarations_dir = temp.path().join("etsFortgz");

    HarDeclarationGenerator::new(temp.path(), &source_dir, declarations_dir.clone(), "entry")
        .with_generated_source(&module_info)
        .generate()
        .expect("generate ByteCode HAR declarations");

    assert_eq!(
        std::fs::read_to_string(declarations_dir.join("entry/src/main/ets/Index.d.ets"))
            .expect("read source declaration"),
        concat!(
            "export declare function formatGreeting(name: string): string;\r\n",
            "export declare class Counter {\r\n",
            "    private value;\r\n",
            "    increment(): number;\r\n",
            "}\r\n"
        )
    );
    assert_eq!(
        std::fs::read_to_string(
            declarations_dir.join("entry/build/default/generated/profile/default/ModuleInfo.d.ts"),
        )
        .expect("read generated declaration"),
        concat!(
            "export declare const __MODULE_NAME__: string;\r\n",
            "export declare const __BUNDLE_NAME__: string;\r\n"
        )
    );
}

#[test]
fn bundled_declarations_replace_source_tree_with_self_contained_entry() {
    let temp = tempfile::tempdir().expect("create module root");
    let source_dir = temp.path().join("src/main/ets");
    std::fs::create_dir_all(source_dir.join("model")).expect("create source tree");
    std::fs::write(
        source_dir.join("Index.ets"),
        "export { UserStore as Store } from './model/UserStore';\n",
    )
    .expect("write entry");
    std::fs::write(
        source_dir.join("model/UserStore.ets"),
        concat!(
            "import { User } from './User';\n",
            "export class UserStore { get(): User { return { id: 1 }; } }\n"
        ),
    )
    .expect("write exported declaration");
    std::fs::write(
        source_dir.join("model/User.ets"),
        "export interface User { id: number }\n",
    )
    .expect("write private type");
    let declarations_dir = temp.path().join("etsFortgz");

    HarDeclarationGenerator::new(
        temp.path(),
        &source_dir,
        declarations_dir.clone(),
        "library",
    )
    .with_package_bundled_declarations(true, Some("src/main/ets/Index.ets"), None)
    .generate()
    .expect("generate bundled declarations");

    let declaration_root = declarations_dir.join("library/src/main/ets");
    let entry =
        std::fs::read_to_string(declaration_root.join("Index.d.ets")).expect("read bundled entry");
    assert!(entry.contains("export declare class UserStore"), "{entry}");
    assert!(entry.contains("export { UserStore as Store }"), "{entry}");
    assert!(entry.contains("declare namespace _User"), "{entry}");
    assert!(entry.contains("get(): _User.User"), "{entry}");
    assert!(!declaration_root.join("model/UserStore.d.ets").exists());
    assert!(!declaration_root.join("model/User.d.ets").exists());
}
