use super::generate_har_declarations::HarDeclarationGenerator;
use super::layout::ModuleBuildLayout;

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

  HarDeclarationGenerator::new_har(temp.path(), &source_dir)
    .generate()
    .expect("generate declarations");

  let declaration = std::fs::read_to_string(
    ModuleBuildLayout::new(temp.path())
      .declarations_dir()
      .join("model/User.d.ets"),
  )
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

  let error = HarDeclarationGenerator::new_har(temp.path(), &source_dir)
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

  HarDeclarationGenerator::new_hsp(temp.path(), &source_dir, "shared")
    .generate()
    .expect("generate declarations");

  assert!(ModuleBuildLayout::new(temp.path())
    .declarations_dir()
    .join("shared/src/main/ets/Index.d.ets")
    .is_file());
}
