//! hvigor/arkdown artifact parity harness for HAP, HAR, HSP, and APP fixtures.
//!
//! Artifact parity tests are environment-gated: without local hvigor
//! configuration they print a skip line and return. Set `ARKDOWN_PARITY_HVIGORW`
//! or `ARKDOWN_PARITY_RUN=1` to generate reports, and set
//! `ARKDOWN_PARITY_STRICT=1` to turn differences or missing hvigor into hard
//! failures.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use hapsigner::{ApplicationVerifier, InputFormat};
use serde_json::{json, Value};
use walkdir::WalkDir;
use zip::ZipArchive;

const ENV_HVIGORW: &str = "ARKDOWN_PARITY_HVIGORW";
const ENV_ARKDOWN_CLI: &str = "ARKDOWN_PARITY_ARKDOWN_CLI";
const ENV_REPORT: &str = "ARKDOWN_PARITY_REPORT";
const ENV_RUN: &str = "ARKDOWN_PARITY_RUN";
const ENV_STRICT: &str = "ARKDOWN_PARITY_STRICT";
const ENV_KEEP_WORKDIR: &str = "ARKDOWN_PARITY_KEEP_WORKDIR";

// ---------------------------------------------------------------------------
// Artifact type — determines hvigor task, arkdown target, and archive format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactType {
  Hap,
  Har,
  Hsp,
  App,
  /// ColdReload quick-fix package (`assembleDevHqf` /
  /// `--action assemble-quickfix`). The parity harness first executes the
  /// source-owned HotReload base-build protocol so both implementations
  /// consume a real symbol map rather than a test-fabricated compiler input.
  Hqf,
  /// Local unit-test build (`hvigorw test` / `--target test`). No HAP/HSP
  /// archive is produced; the `.test` build root carries the generated
  /// testability templates, TestAbility-injected profiles and compiled
  /// assets.
  Test,
  /// `ohosTest` target build (`hvigorw assembleHap -p module=entry@ohosTest`
  /// / `--target hap --build-target-name ohosTest`): compiles the
  /// `src/ohosTest` module and produces the test HAP under `outputs/ohosTest`.
  OhosTest,
}

impl ArtifactType {
  fn ext(self) -> &'static str {
    match self {
      ArtifactType::Hap => "hap",
      ArtifactType::Har => "har",
      ArtifactType::Hsp => "hsp",
      ArtifactType::App => "app",
      ArtifactType::Hqf => "hqf",
      ArtifactType::Test => "test",
      ArtifactType::OhosTest => "hap",
    }
  }

  fn hvigor_task(self) -> &'static str {
    match self {
      ArtifactType::Hap => "assembleHap",
      ArtifactType::Har => "assembleHar",
      ArtifactType::Hsp => "assembleHsp",
      ArtifactType::App => "assembleApp",
      ArtifactType::Hqf => "assembleDevHqf",
      // `hvigorw test`; `setTestDefaultParameters` injects
      // `ohos-test-coverage=true` unless overridden.
      ArtifactType::Test => "test",
      ArtifactType::OhosTest => "assembleHap",
    }
  }

  fn arkdown_target(self) -> &'static str {
    match self {
      ArtifactType::Hap => "hap",
      ArtifactType::Har => "har",
      ArtifactType::Hsp => "hsp",
      ArtifactType::App => "app",
      ArtifactType::Hqf => "hap",
      ArtifactType::Test => "test",
      ArtifactType::OhosTest => "hap",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParityBuildMode {
  Debug,
  Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParityFixtureSetup {
  None,
  Signed,
  SignedAppPackages,
}

impl ParityFixtureSetup {
  fn name(self) -> &'static str {
    match self {
      Self::None => "none",
      Self::Signed => "signed",
      Self::SignedAppPackages => "signed-app-packages",
    }
  }

  fn materialize(self, root: &Path) -> io::Result<()> {
    match self {
      Self::None => Ok(()),
      Self::Signed => SigningParityMaterializer::new(root, false).materialize(),
      Self::SignedAppPackages => SigningParityMaterializer::new(root, true).materialize(),
    }
  }

  fn configure_hvigor_args(self, args: &mut Vec<&'static str>) {
    if self != Self::None {
      // SignHap/SignHsp/SignApp read this exact global Hvigor property in
      // doTaskAction; a configured signingConfig alone does not enable them.
      args.extend(["-p", "enableSignTask=true"]);
    }
  }

  fn signs_embedded_app_packages(self) -> bool {
    matches!(self, Self::SignedAppPackages)
  }
}

struct SigningParityMaterializer<'a> {
  project_root: &'a Path,
  app_with_signed_packages: bool,
}

impl<'a> SigningParityMaterializer<'a> {
  const DEVECO_COMPONENT: [u8; 16] = [
    49, 243, 9, 115, 214, 175, 91, 184, 211, 190, 177, 88, 101, 131, 192, 119,
  ];
  const CONFIG_NAME: &'static str = "arkdownParity";
  const PASSWORD: &'static str = "test123";
  const APP_KEY_ALIAS: &'static str = "oh-app1-key-v1";
  const PROFILE_KEY_ALIAS: &'static str = "oh-profile1-key-v1";
  const ROOT_KEY_ALIAS: &'static str = "oh-root-ca-key-v1";
  const APP_CA_KEY_ALIAS: &'static str = "oh-app-sign-srv-ca-key-v1";
  const PROFILE_CA_KEY_ALIAS: &'static str = "oh-profile-sign-srv-ca-key-v1";
  const ROOT_SUBJECT: &'static str = "C=CN,O=OpenHarmony,OU=OpenHarmony Community,CN=Root CA";
  const APP_CA_SUBJECT: &'static str =
    "C=CN,O=OpenHarmony,OU=OpenHarmony Community,CN=Application Signature Service CA";
  const PROFILE_CA_SUBJECT: &'static str =
    "C=CN,O=OpenHarmony,OU=OpenHarmony Community,CN=Profile Signature Service CA";
  const APP_SUBJECT: &'static str =
    "C=CN,O=OpenHarmony,OU=OpenHarmony Community,CN=Arkdown Parity Application";
  const PROFILE_SUBJECT: &'static str =
    "C=CN,O=OpenHarmony,OU=OpenHarmony Community,CN=Arkdown Parity Profile";
  const SIGN_ALGORITHM: &'static str = "SHA256withECDSA";

  fn new(project_root: &'a Path, app_with_signed_packages: bool) -> Self {
    Self {
      project_root,
      app_with_signed_packages,
    }
  }

  fn materialize(&self) -> io::Result<()> {
    let bundle_name = self.bundle_name()?;
    let sign_dir = self.project_root.join("SignFile");
    fs::create_dir_all(&sign_dir)?;
    let sign_tool = self.sign_tool()?;
    self.generate_authority(&sign_tool, &sign_dir)?;
    self.write_unsigned_profile(&sign_dir, &bundle_name)?;
    self.run_sign_tool(
      &sign_tool,
      &sign_dir,
      &[
        "sign-profile",
        "-keyAlias",
        Self::PROFILE_KEY_ALIAS,
        "-signAlg",
        Self::SIGN_ALGORITHM,
        "-mode",
        "localSign",
        "-profileCertFile",
        "profile.pem",
        "-inFile",
        "profile.json",
        "-keystoreFile",
        "debug.jks",
        "-outFile",
        "debug.p7b",
        "-keyPwd",
        Self::PASSWORD,
        "-keystorePwd",
        Self::PASSWORD,
      ],
    )?;
    let encrypted_password = self.write_password_material(&sign_dir)?;
    self.write_build_profile(&encrypted_password)
  }

  fn generate_authority(&self, sign_tool: &Path, sign_dir: &Path) -> io::Result<()> {
    for alias in [Self::APP_KEY_ALIAS, Self::PROFILE_KEY_ALIAS] {
      self.run_sign_tool(
        sign_tool,
        sign_dir,
        &[
          "generate-keypair",
          "-keyAlias",
          alias,
          "-keyAlg",
          "ECC",
          "-keySize",
          "NIST-P-256",
          "-keystoreFile",
          "debug.jks",
          "-keyPwd",
          Self::PASSWORD,
          "-keystorePwd",
          Self::PASSWORD,
        ],
      )?;
    }
    self.generate_ca(
      sign_tool,
      sign_dir,
      Self::ROOT_KEY_ALIAS,
      Self::ROOT_SUBJECT,
      "root.cer",
      None,
    )?;
    self.generate_ca(
      sign_tool,
      sign_dir,
      Self::APP_CA_KEY_ALIAS,
      Self::APP_CA_SUBJECT,
      "app-ca.cer",
      Some((Self::ROOT_SUBJECT, Self::ROOT_KEY_ALIAS)),
    )?;
    self.generate_ca(
      sign_tool,
      sign_dir,
      Self::PROFILE_CA_KEY_ALIAS,
      Self::PROFILE_CA_SUBJECT,
      "profile-ca.cer",
      Some((Self::ROOT_SUBJECT, Self::ROOT_KEY_ALIAS)),
    )?;
    self.generate_leaf(
      sign_tool,
      sign_dir,
      "generate-app-cert",
      Self::APP_KEY_ALIAS,
      Self::APP_SUBJECT,
      Self::APP_CA_SUBJECT,
      Self::APP_CA_KEY_ALIAS,
      "app-ca.cer",
      "app.pem",
    )?;
    self.generate_leaf(
      sign_tool,
      sign_dir,
      "generate-profile-cert",
      Self::PROFILE_KEY_ALIAS,
      Self::PROFILE_SUBJECT,
      Self::PROFILE_CA_SUBJECT,
      Self::PROFILE_CA_KEY_ALIAS,
      "profile-ca.cer",
      "profile.pem",
    )
  }

  fn generate_ca(
    &self,
    sign_tool: &Path,
    sign_dir: &Path,
    alias: &str,
    subject: &str,
    output: &str,
    issuer: Option<(&str, &str)>,
  ) -> io::Result<()> {
    let mut args = vec![
      "generate-ca",
      "-keyAlias",
      alias,
      "-signAlg",
      Self::SIGN_ALGORITHM,
      "-keyAlg",
      "ECC",
      "-keySize",
      "NIST-P-256",
      "-subject",
      subject,
      "-keystoreFile",
      "debug.jks",
      "-outFile",
      output,
      "-keyPwd",
      Self::PASSWORD,
      "-keystorePwd",
      Self::PASSWORD,
      "-issuerKeyPwd",
      Self::PASSWORD,
      "-validity",
      "3650",
    ];
    if let Some((issuer_subject, issuer_alias)) = issuer {
      args.extend(["-issuer", issuer_subject, "-issuerKeyAlias", issuer_alias]);
    }
    self.run_sign_tool(sign_tool, sign_dir, &args)
  }

  #[allow(clippy::too_many_arguments)]
  fn generate_leaf(
    &self,
    sign_tool: &Path,
    sign_dir: &Path,
    command: &str,
    alias: &str,
    subject: &str,
    issuer: &str,
    issuer_alias: &str,
    sub_ca: &str,
    output: &str,
  ) -> io::Result<()> {
    self.run_sign_tool(
      sign_tool,
      sign_dir,
      &[
        command,
        "-keyAlias",
        alias,
        "-signAlg",
        Self::SIGN_ALGORITHM,
        "-issuer",
        issuer,
        "-issuerKeyAlias",
        issuer_alias,
        "-subject",
        subject,
        "-keystoreFile",
        "debug.jks",
        "-subCaCertFile",
        sub_ca,
        "-rootCaCertFile",
        "root.cer",
        "-outForm",
        "certChain",
        "-outFile",
        output,
        "-keyPwd",
        Self::PASSWORD,
        "-keystorePwd",
        Self::PASSWORD,
        "-issuerKeyPwd",
        Self::PASSWORD,
        "-validity",
        "3650",
      ],
    )
  }

  fn write_unsigned_profile(&self, sign_dir: &Path, bundle_name: &str) -> io::Result<()> {
    let cert_chain = fs::read_to_string(sign_dir.join("app.pem"))?;
    let certificate_end = "-----END CERTIFICATE-----";
    let end = cert_chain
      .find(certificate_end)
      .map(|index| index + certificate_end.len())
      .ok_or_else(|| io::Error::other("generated application certificate is not PEM"))?;
    let application_certificate = format!("{}\n", &cert_chain[..end]);
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_err(|error| io::Error::other(format!("invalid system clock: {error}")))?
      .as_secs();
    let profile = json!({
      "version-name": "2.0.0",
      "version-code": 2,
      "app-distribution-type": "os_integration",
      "uuid": "01234567-89ab-cdef-0123-456789abcdef",
      "validity": {
        "not-before": now.saturating_sub(300),
        "not-after": now + 315_360_000,
      },
      "type": "release",
      "bundle-info": {
        "developer-id": "OpenHarmony",
        "distribution-certificate": application_certificate,
        "bundle-name": bundle_name,
        "apl": "normal",
        "app-feature": "hos_normal_app",
      },
      "acls": { "allowed-acls": [""] },
      "permissions": { "restricted-permissions": [] },
      "issuer": "pki_internal",
    });
    fs::write(sign_dir.join("profile.json"), serde_json::to_vec(&profile)?)
  }

  fn sign_tool(&self) -> io::Result<PathBuf> {
    let sdk_root = std::env::var_os("DEVECO_SDK_HOME")
      .map(PathBuf::from)
      .ok_or_else(|| io::Error::other("DEVECO_SDK_HOME is required for signed parity"))?;
    [
      sdk_root.join("toolchains/lib/hap-sign-tool.jar"),
      sdk_root.join("default/openharmony/toolchains/lib/hap-sign-tool.jar"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::NotFound,
        format!("hap-sign-tool.jar is missing under {}", sdk_root.display()),
      )
    })
  }

  fn run_sign_tool(&self, sign_tool: &Path, sign_dir: &Path, args: &[&str]) -> io::Result<()> {
    let output = Command::new("java")
      .arg("-jar")
      .arg(sign_tool)
      .args(args)
      .current_dir(sign_dir)
      .output()?;
    if output.status.success() {
      return Ok(());
    }
    Err(io::Error::other(format!(
      "{} {:?} failed: {}{}",
      sign_tool.display(),
      args,
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr),
    )))
  }

  /// Materializes the exact three-level key derivation consumed by current
  /// Hvigor `DecipherUtil`: XOR `fd` components, PBKDF2 the `ac` salt, decrypt
  /// the `ce` work key, then decrypt each build-profile password.
  fn write_password_material(&self, sign_dir: &Path) -> io::Result<String> {
    const PASSWORD_MATERIAL: [u8; 16] = *b"arkdown-sign-key";
    const SALT: [u8; 16] = *b"arkdown-test-slt";
    const WORK_KEY: [u8; 16] = [0x42; 16];

    let material_root = sign_dir.join("material");
    for directory in ["fd/0", "fd/1", "fd/2", "ac", "ce"] {
      fs::create_dir_all(material_root.join(directory))?;
    }
    let mut fd0 = [0_u8; 16];
    for index in 0..fd0.len() {
      fd0[index] = PASSWORD_MATERIAL[index] ^ Self::DEVECO_COMPONENT[index];
    }
    fs::write(material_root.join("fd/0/value"), fd0)?;
    fs::write(material_root.join("fd/1/value"), [0_u8; 16])?;
    fs::write(material_root.join("fd/2/value"), [0_u8; 16])?;
    fs::write(material_root.join("ac/value"), SALT)?;

    let mut root_key = [0_u8; 16];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(&PASSWORD_MATERIAL, &SALT, 10_000, &mut root_key);
    fs::write(
      material_root.join("ce/value"),
      Self::encrypt_password_material(&root_key, &[1_u8; 12], &WORK_KEY)?,
    )?;
    let encrypted_password =
      Self::encrypt_password_material(&WORK_KEY, &[2_u8; 12], Self::PASSWORD.as_bytes())?;
    Ok(hex::encode(encrypted_password))
  }

  fn encrypt_password_material(
    key: &[u8; 16],
    nonce: &[u8; 12],
    plaintext: &[u8],
  ) -> io::Result<Vec<u8>> {
    use aes_gcm::{aead::Aead, Aes128Gcm, KeyInit, Nonce};

    let cipher = Aes128Gcm::new_from_slice(key)
      .map_err(|error| io::Error::other(format!("invalid DevEco test key: {error}")))?;
    let encrypted = cipher
      .encrypt(Nonce::from_slice(nonce), plaintext)
      .map_err(|error| io::Error::other(format!("cannot encrypt DevEco test material: {error}")))?;
    let encrypted_length = u32::try_from(encrypted.len())
      .map_err(|_| io::Error::other("DevEco test material exceeds u32"))?;
    let mut output = Vec::with_capacity(4 + nonce.len() + encrypted.len());
    output.extend_from_slice(&encrypted_length.to_be_bytes());
    output.extend_from_slice(nonce);
    output.extend_from_slice(&encrypted);
    Ok(output)
  }

  fn bundle_name(&self) -> io::Result<String> {
    let path = self.project_root.join("AppScope/app.json5");
    let source = fs::read_to_string(&path)?;
    let value: Value = json5::from_str(&source).map_err(|error| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
          "invalid signing fixture app profile {}: {error}",
          path.display()
        ),
      )
    })?;
    value["app"]["bundleName"]
      .as_str()
      .map(str::to_string)
      .ok_or_else(|| io::Error::other(format!("missing app.bundleName in {}", path.display())))
  }

  fn write_build_profile(&self, encrypted_password: &str) -> io::Result<()> {
    let path = self.project_root.join("build-profile.json5");
    let source = fs::read_to_string(&path)?;
    let mut value: Value = json5::from_str(&source).map_err(|error| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
          "invalid signing fixture build profile {}: {error}",
          path.display()
        ),
      )
    })?;
    let app = value
      .get_mut("app")
      .and_then(Value::as_object_mut)
      .ok_or_else(|| io::Error::other(format!("missing app object in {}", path.display())))?;
    let products = app
      .get_mut("products")
      .and_then(Value::as_array_mut)
      .ok_or_else(|| io::Error::other(format!("missing app.products in {}", path.display())))?;
    for product in products {
      let product = product.as_object_mut().ok_or_else(|| {
        io::Error::other(format!("invalid app.products entry in {}", path.display()))
      })?;
      product.insert(
        "signingConfig".to_string(),
        Value::String(Self::CONFIG_NAME.to_string()),
      );
      if self.app_with_signed_packages {
        let build_option = product
          .entry("buildOption")
          .or_insert_with(|| json!({}))
          .as_object_mut()
          .ok_or_else(|| {
            io::Error::other(format!(
              "invalid app.products[].buildOption in {}",
              path.display()
            ))
          })?;
        let pack_options = build_option
          .entry("packOptions")
          .or_insert_with(|| json!({}))
          .as_object_mut()
          .ok_or_else(|| {
            io::Error::other(format!(
              "invalid app.products[].buildOption.packOptions in {}",
              path.display()
            ))
          })?;
        pack_options.insert("appWithSignedPkg".to_string(), Value::Bool(true));
      }
    }
    app.insert(
      "signingConfigs".to_string(),
      json!([{
        "name": Self::CONFIG_NAME,
        "type": "HarmonyOS",
        "material": {
          "certpath": "./SignFile/app.pem",
          "keyAlias": Self::APP_KEY_ALIAS,
          "keyPassword": encrypted_password,
          "profile": "./SignFile/debug.p7b",
          "signAlg": Self::SIGN_ALGORITHM,
          "storeFile": "./SignFile/debug.jks",
          "storePassword": encrypted_password,
        }
      }]),
    );
    fs::write(&path, serde_json::to_vec_pretty(&value)?)
  }
}

impl ParityBuildMode {
  fn as_str(self) -> &'static str {
    match self {
      ParityBuildMode::Debug => "debug",
      ParityBuildMode::Release => "release",
    }
  }
}

// ---------------------------------------------------------------------------
// Top-level test functions
// ---------------------------------------------------------------------------

#[test]
fn minimal_hap_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-hap",
    "minimal-hap",
    ArtifactType::Hap,
    ParityBuildMode::Debug,
  );
}

/// Local unit-test chain (`hvigorw test`) parity: generated testability
/// templates, TestAbility injection, loader/main_pages override, source maps
/// and compile inputs, compared against real hvigor 6.24.3. The fixture
/// carries `src/test/List.test.ets` + `src/ohosTest/ets/test/List.test.ets`.
#[test]
fn minimal_hap_test_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-hap",
    "minimal-hap-test",
    ArtifactType::Test,
    ParityBuildMode::Debug,
  );
}

/// ohosTest target build parity (`hvigorw assembleHap -p module=entry@ohosTest`):
/// src/ohosTest module processing, template generation, test HAP packaging.
#[test]
fn minimal_hap_ohos_test_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-hap",
    "minimal-hap-ohos-test",
    ArtifactType::OhosTest,
    ParityBuildMode::Debug,
  );
}

#[test]
fn minimal_har_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-har",
    "minimal-har",
    ArtifactType::Har,
    ParityBuildMode::Debug,
  );
}

#[test]
fn minimal_hsp_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-hsp",
    "minimal-hsp",
    ArtifactType::Hsp,
    ParityBuildMode::Debug,
  );
}

/// Real ComponentV2 lifecycle golden. This fixture imports the lifecycle
/// decorators from `@kit.ArkUI`, exercises both appearance boundaries, and
/// compares the complete debug source-map / ABC handoff against latest hvigor.
#[test]
fn component_v2_lifecycle_hap_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/component-v2-lifecycle-hap",
    "component-v2-lifecycle-hap",
    ArtifactType::Hap,
    ParityBuildMode::Debug,
  );
}

/// Parity test for ByteCode HAR dependency — validates that BC HAR packages
/// with `metadata.byteCodeHar: true` are detected, their OHM URLs appear
/// in `compiledImportees`, and `byteCodeHarInfo` references the correct .abc path.
///
/// Covers the section 3.8 gap:
///   "ByteCode HAR/source HAR/HAR declarations/obfuscation files"
#[test]
fn bc_har_dep_hap_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-hap-with-bc-har",
    "bc-har-dep-hap",
    ArtifactType::Hap,
    ParityBuildMode::Debug,
  );
}

/// Multi-module parity: an entry HAP depending on a source HAR module.
/// Validates module.json app/module field order, loader.json module maps,
/// cross-module cache TS, and the merged ABC against hvigor.
/// The installed source package is not a project module and ships only
/// module.json5. MergeProfile.mergeDependsLibs / transformJson2Opts must skip
/// its unpublished profile, including its permissions and metadata.
#[test]
fn multi_module_hap_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/multi-module-hap",
    "multi-module-hap",
    ArtifactType::Hap,
    ParityBuildMode::Debug,
  );
}

/// Widget parity: a form widget HSP (`formExtensionModule: "entry"`) paired
/// with an entry HAP carrying the `formWidgetModule` mapping and the form
/// extension ability. Validates `widgets.abc`, `widget_loader.json`
/// (`pkgName2OtherCompileFiles`), the widget component collection, and the
/// packaged HSP against hvigor.
#[test]
fn widget_hap_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/widget-hap",
    "widget-hap",
    ArtifactType::Hsp,
    ParityBuildMode::Debug,
  );
}

/// Project APP parity: two HAPs (entry + feature), two HSPs (form-widget +
/// ordinary shared), project `pack.info`/`pac.json`, nested package contents,
/// `pkgSdkInfo.json`, restored form abilities, widget bytecode, and module
/// bytecode are compared against the real project-mode `assembleApp` task.
#[test]
fn widget_app_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/widget-hap",
    "widget-app",
    ArtifactType::App,
    ParityBuildMode::Debug,
  );
}

/// Current Hvigor SignHap/SignHsp/SignApp parity with real PKCS#12,
/// application certificate and signed provisioning profile inputs. The
/// material is generated into each clean comparison root so no host-local
/// DevEco signing state participates in the oracle.
#[test]
fn signed_hap_hsp_and_app_hvigor_parity_report() {
  for (fixture, report_name, artifact_type) in [
    (
      "tests/fixtures/minimal-hap",
      "signed-minimal-hap",
      ArtifactType::Hap,
    ),
    (
      "tests/fixtures/minimal-hsp",
      "signed-minimal-hsp",
      ArtifactType::Hsp,
    ),
    (
      "tests/fixtures/widget-hap",
      "signed-widget-app",
      ArtifactType::App,
    ),
  ] {
    run_parity_report_test_with_setup(
      fixture,
      report_name,
      artifact_type,
      ParityBuildMode::Debug,
      ParityFixtureSetup::Signed,
    );
  }
}

/// `SignPackagesFromApp` parity: unpack the ordinary APP, sign every embedded
/// HAP/HSP, repack the `all` APP, and finally sign both outer APP variants.
#[test]
fn signed_app_with_embedded_packages_hvigor_parity_report() {
  run_parity_report_test_with_setup(
    "tests/fixtures/widget-hap",
    "signed-widget-all-app",
    ArtifactType::App,
    ParityBuildMode::Debug,
    ParityFixtureSetup::SignedAppPackages,
  );
}

/// `HotReloadArkTS -> ColdReloadArkTS -> PrepareQuickfix -> PackageHqf ->
/// SignHqf` parity with a real es2abc symbol-table handoff, SDK HQF packer and
/// PKCS#12/profile signing material.
#[test]
fn signed_hqf_hvigor_parity_report() {
  run_parity_report_test_with_setup(
    "tests/fixtures/minimal-hap",
    "signed-minimal-hqf",
    ArtifactType::Hqf,
    ParityBuildMode::Debug,
    ParityFixtureSetup::Signed,
  );
}

/// Parity test for share config — validates that crossAppSharedConfig entries
/// exported by a HAR dependency survive profile processing and are written to
/// the correct resource profile. Hvigor's ProcessShareConfig consumes
/// `getHarDependencies()`, not HSP dependencies.
/// Covers the P0 fix: share config was being overwritten by static intermediate files.
#[test]
fn share_config_hap_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/share-config-hap",
    "share-config-hap",
    ArtifactType::Hap,
    ParityBuildMode::Debug,
  );
}

/// Parity test for startup config — validates appStartup task compilation and
/// startup profile resource rewrite.
#[test]
fn startup_config_hap_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/startup-config-hap",
    "startup-config-hap",
    ArtifactType::Hap,
    ParityBuildMode::Debug,
  );
}

/// Parity test for release HAP — validates that release obfuscation is applied
/// to the CompileArkTS handoff inputs (the .ts files that feed into es2abc),
/// not just the intermediate .js chunks.
#[test]
fn release_hap_obfuscation_parity_report() {
  run_parity_report_test(
    "tests/fixtures/release-obfuscation-hap",
    "release-obfuscation-hap",
    ArtifactType::Hap,
    ParityBuildMode::Release,
  );
}

/// Parity test for minimal release HAP (no obfuscation) — validates that the
/// pack.info has correct bundle type, version, and device types.
#[test]
fn minimal_hap_release_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-hap",
    "minimal-hap-release",
    ArtifactType::Hap,
    ParityBuildMode::Release,
  );
}

/// Release HSP golden: validates the shared-package archive and release
/// CompileArkTS/source-map layout against latest hvigor.
#[test]
fn minimal_hsp_release_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-hsp",
    "minimal-hsp-release",
    ArtifactType::Hsp,
    ParityBuildMode::Release,
  );
}

/// Release source HAR golden: validates the HAR task graph, declaration
/// output, release BuildProfile location, and source-map handling.
#[test]
fn minimal_har_release_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/minimal-har",
    "minimal-har-release",
    ArtifactType::Har,
    ParityBuildMode::Release,
  );
}

/// ByteCode HAR producer golden. Existing consumer coverage only verifies a
/// prebuilt dependency; this validates the producer's own `modules.abc`,
/// declaration metadata, source map, and archive layout.
#[test]
fn bytecode_har_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/bytecode-har",
    "bytecode-har",
    ArtifactType::Har,
    ParityBuildMode::Debug,
  );
}

#[test]
fn bytecode_har_release_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/bytecode-har",
    "bytecode-har-release",
    ArtifactType::Har,
    ParityBuildMode::Release,
  );
}

/// ComponentV2 release golden extends source-map coverage beyond minimal HAP
/// and verifies lifecycle lowering survives the release task graph.
#[test]
fn component_v2_lifecycle_hap_release_hvigor_parity_report() {
  run_parity_report_test(
    "tests/fixtures/component-v2-lifecycle-hap",
    "component-v2-lifecycle-hap-release",
    ArtifactType::Hap,
    ParityBuildMode::Release,
  );
}

/// Same-root cache golden. It runs a cold build, an unchanged rebuild, then a
/// source mutation on both hvigor and arkdown copies. This guards cache reuse
/// and invalidation without conflating them with fresh-build parity.
#[test]
fn component_v2_lifecycle_incremental_hvigor_parity_report() {
  run_incremental_parity_report_test(
    "tests/fixtures/component-v2-lifecycle-hap",
    "component-v2-lifecycle-incremental",
    ArtifactType::Hap,
    ParityBuildMode::Debug,
    ParitySourceMutation::new(
      "entry/src/main/ets/pages/Index.ets",
      "\"initial\"",
      "\"updated-after-incremental-build\"",
    ),
  );
}

/// ComponentV2 decorator diagnostics are semantic contracts too: hvigor
/// rejects `@Require` combined with `@Local` before package output exists.
/// Keep arkdown tied to the same source-owned diagnostic boundary.
#[test]
fn component_v2_require_diagnostic_matches_hvigor() {
  run_component_v2_require_diagnostic_parity_test();
}

fn run_parity_report_test(
  fixture: &str,
  report_name: &str,
  artifact_type: ArtifactType,
  build_mode: ParityBuildMode,
) {
  run_parity_report_test_with_setup(
    fixture,
    report_name,
    artifact_type,
    build_mode,
    ParityFixtureSetup::None,
  );
}

fn run_parity_report_test_with_setup(
  fixture: &str,
  report_name: &str,
  artifact_type: ArtifactType,
  build_mode: ParityBuildMode,
  setup: ParityFixtureSetup,
) {
  let repo_root = repo_root();
  let fixture_abs = repo_root.join(fixture);
  if !should_run_parity(&fixture_abs) {
    tracing::error!(
      "skipping hvigor parity [{fixture}]; set {ENV_HVIGORW}, {ENV_RUN}=1, or {ENV_STRICT}=1"
    );
    return;
  }

  let report_path = report_path_for(&repo_root, report_name);
  let report =
    parity_for_fixture_with_setup(fixture, artifact_type, build_mode, setup, report_path)
      .expect("parity harness failed before report generation");
  print_report_summary(&report);
  assert_strict_parity(report_name, &report);
}

fn run_incremental_parity_report_test(
  fixture: &str,
  report_name: &str,
  artifact_type: ArtifactType,
  build_mode: ParityBuildMode,
  mutation: ParitySourceMutation,
) {
  let repo_root = repo_root();
  let fixture_abs = repo_root.join(fixture);
  if !should_run_parity(&fixture_abs) {
    tracing::error!(
      "skipping hvigor incremental parity [{fixture}]; set {ENV_HVIGORW}, {ENV_RUN}=1, or {ENV_STRICT}=1"
    );
    return;
  }

  let report_path = report_path_for(&repo_root, report_name);
  let report =
    incremental_parity_for_fixture(fixture, artifact_type, build_mode, mutation, report_path)
      .expect("incremental parity harness failed before report generation");
  print_report_summary(&report);
  assert_strict_parity(report_name, &report);
}

#[derive(Debug, Clone)]
struct ParitySourceMutation {
  relative_path: PathBuf,
  before: String,
  after: String,
}

impl ParitySourceMutation {
  fn new(
    relative_path: impl Into<PathBuf>,
    before: impl Into<String>,
    after: impl Into<String>,
  ) -> Self {
    Self {
      relative_path: relative_path.into(),
      before: before.into(),
      after: after.into(),
    }
  }

  fn apply(&self, root: &Path) -> io::Result<()> {
    let path = root.join(&self.relative_path);
    let source = fs::read_to_string(&path)?;
    if !source.contains(&self.before) {
      return Err(io::Error::other(format!(
        "incremental parity mutation marker {:?} is missing from {}",
        self.before,
        path.display()
      )));
    }
    fs::write(&path, source.replacen(&self.before, &self.after, 1))
  }
}

fn assert_strict_parity(report_name: &str, report: &Value) {
  if env_truthy(ENV_STRICT) && !report["equal"].as_bool().unwrap_or(false) {
    panic!(
      "strict parity failed for {report_name}; see {}",
      report["report_path"].as_str().unwrap_or("<unknown>")
    );
  }
}

fn should_run_parity(fixture_root: &Path) -> bool {
  should_run_parity_with_options(
    env_truthy(ENV_STRICT),
    env_truthy(ENV_RUN),
    std::env::var_os(ENV_HVIGORW).is_some(),
    fixture_root.join("hvigorw").exists(),
  )
}

fn should_run_parity_with_options(
  strict: bool,
  force_run: bool,
  hvigorw_env: bool,
  fixture_hvigorw: bool,
) -> bool {
  strict || force_run || hvigorw_env || fixture_hvigorw
}

#[test]
fn parity_run_gate_defaults_to_skip_without_hvigor_config() {
  assert!(!should_run_parity_with_options(false, false, false, false));
  assert!(should_run_parity_with_options(true, false, false, false));
  assert!(should_run_parity_with_options(false, true, false, false));
  assert!(should_run_parity_with_options(false, false, true, false));
  assert!(should_run_parity_with_options(false, false, false, true));
}

#[test]
fn parity_rejects_arkdown_compatibility_cli_as_hvigor_reference() {
  let temp = tempfile::tempdir().expect("create fake hvigor directory");
  let fake_hvigor = temp.path().join("hvigorw");
  fs::write(
    &fake_hvigor,
    "const { HVIGOR_PARITY_VERSION, parseArkdownBuildCliArgs } = require('arkdown');\n",
  )
  .expect("write fake arkdown compatibility CLI");

  let error = validate_hvigorw_program(fake_hvigor.as_os_str())
    .expect_err("arkdown compatibility CLI must not be accepted as a hvigor reference");
  assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
  assert!(error.to_string().contains("real DevEco hvigorw executable"));
}

fn print_report_summary(report: &Value) {
  let report_path = report["report_path"]
    .as_str()
    .unwrap_or("<unknown report path>");
  let equal = report["equal"].as_bool().unwrap_or(false);
  let diff_count = report["diffs"]
    .as_array()
    .map(|diffs| diffs.len())
    .unwrap_or(0);
  let fixture = report["fixture"].as_str().unwrap_or("<unknown fixture>");

  tracing::info!(
    "hvigor parity report [{fixture}] written to {report_path}; equal={equal}; diff_count={diff_count}"
  );
}

#[test]
fn parity_command_args_include_selected_build_mode() {
  assert_eq!(
    hvigor_args(ArtifactType::Hap, ParityBuildMode::Debug),
    vec![
      "--mode",
      "module",
      "-p",
      "product=default",
      "-p",
      "buildMode=debug",
      "assembleHap",
      "--no-daemon"
    ]
  );
  assert_eq!(
    hvigor_args(ArtifactType::Hap, ParityBuildMode::Release),
    vec![
      "--mode",
      "module",
      "-p",
      "product=default",
      "-p",
      "buildMode=release",
      "assembleHap",
      "--no-daemon"
    ]
  );
  let arkdown = arkdown_args(ArtifactType::Hap, ParityBuildMode::Release);
  assert!(Path::new(&arkdown[0]).ends_with("packages/arkdown/js/arkdown-cli.js"));
  assert_eq!(
    &arkdown[1..],
    ["build", "--target", "hap", "--mode", "release"]
  );
}

#[test]
fn parity_referenced_fixtures_exist() {
  let repo_root = repo_root();
  for fixture in [
    "tests/fixtures/minimal-hap",
    "tests/fixtures/minimal-hap-with-bc-har",
    "tests/fixtures/minimal-har",
    "tests/fixtures/minimal-hsp",
    "tests/fixtures/component-v2-lifecycle-hap",
    "tests/fixtures/bytecode-har",
    "tests/fixtures/share-config-hap",
    "tests/fixtures/startup-config-hap",
    "tests/fixtures/release-obfuscation-hap",
    "tests/fixtures/multi-module-hap",
    "tests/fixtures/widget-hap",
  ] {
    let path = repo_root.join(fixture);
    assert!(path.is_dir(), "parity fixture must exist: {fixture}");
  }
}

// ---------------------------------------------------------------------------
// Core parity harness — generic over fixture path and artifact type
// ---------------------------------------------------------------------------

fn parity_for_fixture_with_setup(
  fixture: &str,
  artifact_type: ArtifactType,
  build_mode: ParityBuildMode,
  setup: ParityFixtureSetup,
  report_path: PathBuf,
) -> Result<Value, Box<dyn std::error::Error>> {
  let repo_root = repo_root();
  let fixture_abs = repo_root.join(fixture);
  let fixture_name = fixture_abs
    .file_name()
    .map(|n| n.to_string_lossy().into_owned())
    .unwrap_or_else(|| fixture.to_string());

  fs::create_dir_all(report_path.parent().expect("report path has parent"))?;

  let temp = tempfile::Builder::new()
    .prefix("arkdown-hvigor-parity-")
    .tempdir()?;
  let temp_root = temp.path().to_path_buf();
  // `@ohos/coverage` local-test templates embed the absolute project path in
  // executable source. Building two copies under `hvigor/` and `arkdown/`
  // therefore changes both source-map generated columns and the ABC string
  // pool before either implementation is compared. Run the cold builds at
  // the same absolute path and keep the first build as an owned snapshot.
  let project_root = temp.path().join(format!("project/{fixture_name}"));
  copy_clean_fixture(&fixture_abs, &project_root)?;
  materialize_local_file_dependencies(&project_root)?;
  setup.materialize(&project_root)?;

  let hvigorw = hvigorw_program(&project_root)?;
  let arkdown_bin = arkdown_program();
  let mut hvigor_args = hvigor_args(artifact_type, build_mode);
  setup.configure_hvigor_args(&mut hvigor_args);

  let hqf_base = if artifact_type == ArtifactType::Hqf {
    match prepare_hvigor_hqf_base_build(&hvigorw, &project_root) {
      Ok(base) => Some(base),
      Err(error) => {
        if env_truthy(ENV_KEEP_WORKDIR) {
          let preserved = temp.keep();
          eprintln!(
            "preserved failed HQF parity workspace at {}",
            preserved.display()
          );
        }
        return Err(error);
      }
    }
  } else {
    None
  };

  let hvigor_command = run_command("hvigor", &hvigorw, &hvigor_args, &project_root);
  let hvigor = collect_build("hvigor", &project_root, hvigor_command, artifact_type)?;
  if artifact_type == ArtifactType::Hqf {
    stop_hvigor_daemon(&hvigorw, &project_root);
  }

  fs::remove_dir_all(&project_root)?;
  copy_clean_fixture(&fixture_abs, &project_root)?;
  materialize_local_file_dependencies(&project_root)?;
  setup.materialize(&project_root)?;

  if artifact_type == ArtifactType::Hqf {
    prepare_hqf_cold_request(
      &project_root,
      hqf_base
        .as_ref()
        .expect("HQF base state captured from Hvigor base build"),
    )?;
  }

  let arkdown_project_arg = project_root.to_string_lossy().into_owned();
  let mut arkdown_args = arkdown_args(artifact_type, build_mode);
  arkdown_args.push("--project".to_string());
  arkdown_args.push(arkdown_project_arg);
  let arkdown_command = run_command("arkdown", &arkdown_bin, &arkdown_args, &project_root);

  let arkdown = collect_build("arkdown", &project_root, arkdown_command, artifact_type)?;
  if setup != ParityFixtureSetup::None {
    verify_signed_build(&hvigor, artifact_type, setup.signs_embedded_app_packages())?;
    verify_signed_build(&arkdown, artifact_type, setup.signs_embedded_app_packages())?;
  }
  let diffs = compare_builds(&hvigor, &arkdown);

  let equal = diffs.is_empty()
    && hvigor.command.success
    && arkdown.command.success
    && hvigor.command.status == "ran"
    && arkdown.command.status == "ran";

  let mut report = json!({
      "fixture": fixture,
      "artifact_type": artifact_type.ext(),
      "build_mode": build_mode.as_str(),
      "fixture_setup": setup.name(),
      "report_path": report_path.to_string_lossy(),
      "equal": equal,
      "strict": env_truthy(ENV_STRICT),
      "env": {
          "ARKDOWN_PARITY_HVIGORW": std::env::var(ENV_HVIGORW).ok(),
          "ARKDOWN_PARITY_ARKDOWN_CLI": std::env::var(ENV_ARKDOWN_CLI).ok(),
          "ARKDOWN_PARITY_REPORT": std::env::var(ENV_REPORT).ok(),
          "ARKDOWN_PARITY_KEEP_WORKDIR": std::env::var(ENV_KEEP_WORKDIR).ok(),
      },
      "workdir": env_truthy(ENV_KEEP_WORKDIR).then(|| temp_root.to_string_lossy().into_owned()),
      "builds": {
          "hvigor": hvigor.to_json(),
          "arkdown": arkdown.to_json(),
      },
      "diffs": diffs,
  });

  let pretty = serde_json::to_string_pretty(&report)?;
  fs::write(&report_path, pretty)?;
  report["report_path"] = json!(report_path.to_string_lossy());
  if env_truthy(ENV_KEEP_WORKDIR) {
    let _ = temp.keep();
  }
  Ok(report)
}

fn verify_signed_build(
  build: &BuildSnapshot,
  artifact_type: ArtifactType,
  verify_embedded_packages: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  let archive = build.archive.as_ref().ok_or_else(|| {
    io::Error::other(format!(
      "{} signed {} archive is missing",
      build.tool,
      artifact_type.ext()
    ))
  })?;
  let bytes = fs::read(&archive.path)?;
  ApplicationVerifier::new(&bytes)
    .verify(InputFormat::Zip)
    .map_err(|error| {
      io::Error::other(format!(
        "{} signed archive verification failed for {}: {error}",
        build.tool,
        archive.path.display()
      ))
    })?;

  // `SignApp` signs only the outer APP. `SignPackagesFromApp` is selected by
  // `packOptions.appWithSignedPkg` and is the source-owned stage that signs
  // every embedded HAP/HSP before the outer archive is rebuilt.
  if artifact_type == ArtifactType::App && verify_embedded_packages {
    for (name, bytes) in &archive.contents {
      if !matches!(
        Path::new(name).extension().and_then(OsStr::to_str),
        Some("hap" | "hsp")
      ) {
        continue;
      }
      ApplicationVerifier::new(bytes)
        .verify(InputFormat::Zip)
        .map_err(|error| {
          io::Error::other(format!(
            "{} embedded signed package verification failed for {name}: {error}",
            build.tool
          ))
        })?;
    }
  }
  Ok(())
}

/// Materialize the debugger handoff consumed by the current Hvigor
/// `HotReloadArkCompile` and `ColdReloadArkCompile` tasks, execute a genuine
/// dump-symbol-table base build, then switch the same paths to the cold-reload
/// request consumed by `assembleDevHqf`.
struct HqfBaseState {
  symbol_map: Vec<u8>,
  ets_state: Vec<u8>,
}

fn prepare_hvigor_hqf_base_build(
  program: &OsStr,
  project_root: &Path,
) -> Result<HqfBaseState, Box<dyn std::error::Error>> {
  let module_root = project_root.join("entry");
  let loader_out_ets = module_root.join("build/default/intermediates/loader_out/default/ets");
  let patch_root = module_root.join("build/default/intermediates/patch/default");
  let changed_file_list = patch_root.join("changedFileList.json");
  let patch_abc_path = patch_root.join("ets");
  let build_config = module_root.join("build/config/buildConfig.json");

  write_reload_build_config(
    &build_config,
    "hotReload",
    &loader_out_ets,
    &changed_file_list,
    &patch_abc_path,
  )?;

  let args = [
    "--mode",
    "module",
    "-p",
    "product=default",
    "-p",
    "buildMode=debug",
    "HotReloadBuild",
    "--daemon",
  ];
  let mut command = run_command("hvigor", program, &args, project_root);
  if command.success && !build_config.is_file() {
    // Hvigor prepends `Clean` to the first daemon HotReloadBuild request and
    // removes the externally-owned buildConfig. Re-materialize that same
    // debugger handoff and submit the next public HotReloadBuild request.
    write_reload_build_config(
      &build_config,
      "hotReload",
      &loader_out_ets,
      &changed_file_list,
      &patch_abc_path,
    )?;
    command = run_command("hvigor", program, &args, project_root);
  }
  if !command.success {
    return Err(
      io::Error::other(format!(
        "hvigor HotReload base build failed (exit {:?}):\n{}",
        command.exit_code,
        command.output_text()
      ))
      .into(),
    );
  }
  let symbol_map = loader_out_ets.join("symbolMap.map");
  // The daemon client schedules HotReloadBuild on its persistent worker. Wait
  // for the source-defined graph and symbol-table outputs rather than using a
  // fixed build delay.
  let ets_state = patch_root.join("ets.json");
  for _ in 0..150 {
    if symbol_map.is_file() && ets_state.is_file() {
      break;
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
  }
  if !symbol_map.is_file() || !ets_state.is_file() {
    stop_hvigor_daemon(program, project_root);
    let generated_symbol_maps = WalkDir::new(&module_root)
      .into_iter()
      .filter_map(Result::ok)
      .filter(|entry| entry.file_type().is_file() && entry.file_name() == "symbolMap.map")
      .map(|entry| entry.path().display().to_string())
      .collect::<Vec<_>>();
    return Err(
      io::Error::other(format!(
        "hvigor HotReload base build did not produce {} and {}; found {:?}; command output:\n{}",
        symbol_map.display(),
        ets_state.display(),
        generated_symbol_maps,
        command.output_text()
      ))
      .into(),
    );
  }

  let base = HqfBaseState {
    symbol_map: fs::read(&symbol_map)?,
    ets_state: fs::read(&ets_state)?,
  };
  write_reload_build_config(
    &build_config,
    "coldReload",
    &loader_out_ets,
    &changed_file_list,
    &patch_abc_path,
  )?;
  if let Some(parent) = changed_file_list.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(
    changed_file_list,
    serde_json::to_vec(&json!({
      "modifiedFiles": ["pages/Index.ets"],
      "removedFiles": []
    }))?,
  )?;
  Ok(base)
}

/// ColdReload consumes the symbol table emitted by the immediately preceding
/// Hvigor HotReload base build. Reusing that compiler-owned input makes both
/// final HQF builds start from the identical base application without
/// fabricating a symbol-table format in the test harness.
fn prepare_hqf_cold_request(
  project_root: &Path,
  base: &HqfBaseState,
) -> Result<(), Box<dyn std::error::Error>> {
  let module_root = project_root.join("entry");
  let loader_out_ets = module_root.join("build/default/intermediates/loader_out/default/ets");
  let patch_root = module_root.join("build/default/intermediates/patch/default");
  let changed_file_list = patch_root.join("changedFileList.json");
  let patch_abc_path = patch_root.join("ets");
  let build_config = module_root.join("build/config/buildConfig.json");
  write_reload_build_config(
    &build_config,
    "coldReload",
    &loader_out_ets,
    &changed_file_list,
    &patch_abc_path,
  )?;
  fs::create_dir_all(&loader_out_ets)?;
  fs::write(loader_out_ets.join("symbolMap.map"), &base.symbol_map)?;
  fs::create_dir_all(&patch_root)?;
  fs::write(patch_root.join("ets.json"), &base.ets_state)?;
  fs::write(
    changed_file_list,
    serde_json::to_vec(&json!({
      "modifiedFiles": ["pages/Index.ets"],
      "removedFiles": []
    }))?,
  )?;
  Ok(())
}

fn stop_hvigor_daemon(program: &OsStr, project_root: &Path) {
  let report = run_command(
    "hvigor",
    program,
    &["--stop-daemon", "--no-daemon"],
    project_root,
  );
  if !report.success {
    eprintln!(
      "failed to stop parity Hvigor daemon for {}:\n{}",
      project_root.display(),
      report.output_text()
    );
  }
}

fn write_reload_build_config(
  path: &Path,
  mode: &str,
  old_map_file_path: &Path,
  changed_file_list: &Path,
  patch_abc_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(
    path,
    serde_json::to_vec_pretty(&json!({
      "patchConfig": {
        "enableMap": "true",
        "mode": mode,
        "oldMapFilePath": old_map_file_path,
        "changedFileList": changed_file_list,
        "patchAbcPath": patch_abc_path,
        "removeChangedFileListInSdk": "true"
      }
    }))?,
  )?;
  Ok(())
}

/// Run cold, unchanged, and invalidated builds in the same project copies.
///
/// Hvigor's cache is intentionally preserved between each invocation.  The
/// resulting snapshots compare compiler handoff/cache content as well as final
/// archives, so a stale transform or ABC cache cannot be hidden by a correct
/// fresh build.
fn incremental_parity_for_fixture(
  fixture: &str,
  artifact_type: ArtifactType,
  build_mode: ParityBuildMode,
  mutation: ParitySourceMutation,
  report_path: PathBuf,
) -> Result<Value, Box<dyn std::error::Error>> {
  let repo_root = repo_root();
  let fixture_abs = repo_root.join(fixture);
  let fixture_name = fixture_abs
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| fixture.to_string());

  fs::create_dir_all(report_path.parent().expect("report path has parent"))?;
  let temp = tempfile::Builder::new()
    .prefix("arkdown-hvigor-incremental-parity-")
    .tempdir()?;
  let temp_root = temp.path().to_path_buf();
  let hvigor_root = temp_root.join(format!("hvigor/{fixture_name}"));
  let arkdown_root = temp_root.join(format!("arkdown/{fixture_name}"));
  copy_clean_fixture(&fixture_abs, &hvigor_root)?;
  copy_clean_fixture(&fixture_abs, &arkdown_root)?;
  materialize_local_file_dependencies(&hvigor_root)?;
  materialize_local_file_dependencies(&arkdown_root)?;

  let hvigorw = hvigorw_program(&hvigor_root)?;
  let arkdown_bin = arkdown_program();
  let hvigor_args = hvigor_args(artifact_type, build_mode);
  let arkdown_project_arg = arkdown_root.to_string_lossy().into_owned();
  let mut arkdown_args = arkdown_args(artifact_type, build_mode);
  arkdown_args.push("--project".to_string());
  arkdown_args.push(arkdown_project_arg);

  let cold_hvigor = collect_build(
    "hvigor",
    &hvigor_root,
    run_command("hvigor", &hvigorw, &hvigor_args, &hvigor_root),
    artifact_type,
  )?;
  let cold_arkdown = collect_build(
    "arkdown",
    &arkdown_root,
    run_command("arkdown", &arkdown_bin, &arkdown_args, &arkdown_root),
    artifact_type,
  )?;
  let cold_diffs = compare_builds(&cold_hvigor, &cold_arkdown);

  let unchanged_hvigor = collect_build(
    "hvigor",
    &hvigor_root,
    run_command("hvigor", &hvigorw, &hvigor_args, &hvigor_root),
    artifact_type,
  )?;
  let unchanged_arkdown = collect_build(
    "arkdown",
    &arkdown_root,
    run_command("arkdown", &arkdown_bin, &arkdown_args, &arkdown_root),
    artifact_type,
  )?;
  let unchanged_diffs = compare_builds(&unchanged_hvigor, &unchanged_arkdown);

  mutation.apply(&hvigor_root)?;
  mutation.apply(&arkdown_root)?;
  let changed_hvigor = collect_build(
    "hvigor",
    &hvigor_root,
    run_command("hvigor", &hvigorw, &hvigor_args, &hvigor_root),
    artifact_type,
  )?;
  let changed_arkdown = collect_build(
    "arkdown",
    &arkdown_root,
    run_command("arkdown", &arkdown_bin, &arkdown_args, &arkdown_root),
    artifact_type,
  )?;
  let changed_diffs = compare_builds(&changed_hvigor, &changed_arkdown);

  let hvigor_cache_stable = compare_builds(&cold_hvigor, &unchanged_hvigor).is_empty();
  let arkdown_cache_stable = compare_builds(&cold_arkdown, &unchanged_arkdown).is_empty();
  let hvigor_cache_invalidated = !compare_builds(&cold_hvigor, &changed_hvigor).is_empty();
  let arkdown_cache_invalidated = !compare_builds(&cold_arkdown, &changed_arkdown).is_empty();
  let equal = cold_diffs.is_empty()
    && unchanged_diffs.is_empty()
    && changed_diffs.is_empty()
    && hvigor_cache_stable
    && arkdown_cache_stable
    && hvigor_cache_invalidated
    && arkdown_cache_invalidated;

  let mut report = json!({
      "fixture": fixture,
      "artifact_type": artifact_type.ext(),
      "build_mode": build_mode.as_str(),
      "report_path": report_path.to_string_lossy(),
      "equal": equal,
      "strict": env_truthy(ENV_STRICT),
      "mutation": {
          "path": mutation.relative_path.to_string_lossy(),
          "before": mutation.before,
          "after": mutation.after,
      },
      "workdir": env_truthy(ENV_KEEP_WORKDIR).then(|| temp_root.to_string_lossy().into_owned()),
      "builds": {
          "cold": {
              "hvigor": cold_hvigor.to_json(),
              "arkdown": cold_arkdown.to_json(),
          },
          "unchanged": {
              "hvigor": unchanged_hvigor.to_json(),
              "arkdown": unchanged_arkdown.to_json(),
          },
          "changed": {
              "hvigor": changed_hvigor.to_json(),
              "arkdown": changed_arkdown.to_json(),
          },
      },
      "cache_contract": {
          "hvigor_stable_on_unchanged_rebuild": hvigor_cache_stable,
          "arkdown_stable_on_unchanged_rebuild": arkdown_cache_stable,
          "hvigor_invalidates_after_source_change": hvigor_cache_invalidated,
          "arkdown_invalidates_after_source_change": arkdown_cache_invalidated,
      },
      "diffs": {
          "cold": cold_diffs,
          "unchanged": unchanged_diffs,
          "changed": changed_diffs,
      },
  });

  fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
  report["report_path"] = json!(report_path.to_string_lossy());
  if env_truthy(ENV_KEEP_WORKDIR) {
    let _ = temp.keep();
  }
  Ok(report)
}

fn run_component_v2_require_diagnostic_parity_test() {
  let fixture = "tests/fixtures/component-v2-lifecycle-hap";
  let repo_root = repo_root();
  let fixture_abs = repo_root.join(fixture);
  if !should_run_parity(&fixture_abs) {
    tracing::error!(
      "skipping ComponentV2 @Require diagnostic parity; set {ENV_HVIGORW}, {ENV_RUN}=1, or {ENV_STRICT}=1"
    );
    return;
  }

  let temp = tempfile::Builder::new()
    .prefix("arkdown-hvigor-component-v2-diagnostic-")
    .tempdir()
    .expect("create diagnostic parity workspace");
  let hvigor_root = temp.path().join("hvigor/component-v2-lifecycle-hap");
  let arkdown_root = temp.path().join("arkdown/component-v2-lifecycle-hap");
  copy_clean_fixture(&fixture_abs, &hvigor_root).expect("copy hvigor diagnostic fixture");
  copy_clean_fixture(&fixture_abs, &arkdown_root).expect("copy arkdown diagnostic fixture");
  materialize_local_file_dependencies(&hvigor_root)
    .expect("materialize hvigor diagnostic fixture dependencies");
  materialize_local_file_dependencies(&arkdown_root)
    .expect("materialize arkdown diagnostic fixture dependencies");
  let mutation = ParitySourceMutation::new(
    "entry/src/main/ets/pages/Index.ets",
    "@Local message: string = \"initial\";",
    "@Require\n  @Local message: string = \"initial\";",
  );
  mutation
    .apply(&hvigor_root)
    .expect("mutate hvigor ComponentV2 source");
  mutation
    .apply(&arkdown_root)
    .expect("mutate arkdown ComponentV2 source");

  let artifact_type = ArtifactType::Hap;
  let build_mode = ParityBuildMode::Debug;
  let hvigorw = hvigorw_program(&hvigor_root).expect("resolve a real hvigor executable");
  let hvigor = run_command(
    "hvigor",
    &hvigorw,
    &hvigor_args(artifact_type, build_mode),
    &hvigor_root,
  );
  let arkdown_project_arg = arkdown_root.to_string_lossy().into_owned();
  let mut arkdown_args = arkdown_args(artifact_type, build_mode);
  arkdown_args.push("--project".to_string());
  arkdown_args.push(arkdown_project_arg);
  let arkdown = run_command("arkdown", &arkdown_program(), &arkdown_args, &arkdown_root);
  let expected = "In a struct decorated with '@ComponentV2', '@Require' can only be used with '@Param' and '@BuilderParam'.";
  assert!(
    !hvigor.success,
    "latest hvigor accepted invalid ComponentV2 @Require property:\n{}",
    hvigor.output_text()
  );
  assert!(
    hvigor.output_text().contains(expected),
    "latest hvigor did not emit the ComponentV2 @Require diagnostic {expected:?}:\n{}",
    hvigor.output_text()
  );
  assert!(
    !arkdown.success,
    "arkdown accepted a ComponentV2 @Require property that hvigor rejects:\n{}",
    arkdown.output_text()
  );
  assert!(
    arkdown.output_text().contains(expected),
    "arkdown did not preserve hvigor's ComponentV2 @Require diagnostic {expected:?}:\n{}",
    arkdown.output_text()
  );
}

fn hvigor_args(artifact_type: ArtifactType, build_mode: ParityBuildMode) -> Vec<&'static str> {
  let mut args = vec![
    "--mode",
    if artifact_type == ArtifactType::App {
      "project"
    } else {
      "module"
    },
    "-p",
    "product=default",
    "-p",
    match build_mode {
      ParityBuildMode::Debug => "buildMode=debug",
      ParityBuildMode::Release => "buildMode=release",
    },
    artifact_type.hvigor_task(),
    if artifact_type == ArtifactType::Hqf {
      "--daemon"
    } else {
      "--no-daemon"
    },
  ];
  if artifact_type == ArtifactType::Test {
    // Default `ohos-test-coverage=true` instruments source; disable it so
    // both sides compile the same sources.
    args.push("-p");
    args.push("ohos-test-coverage=false");
  }
  if artifact_type == ArtifactType::OhosTest {
    args.push("-p");
    args.push("module=entry@ohosTest");
  }
  args
}

fn arkdown_args(artifact_type: ArtifactType, build_mode: ParityBuildMode) -> Vec<String> {
  let mut args = vec![
    arkdown_cli_path().to_string_lossy().into_owned(),
    "build".to_string(),
    "--target".to_string(),
    artifact_type.arkdown_target().to_string(),
    "--mode".to_string(),
    build_mode.as_str().to_string(),
  ];
  if artifact_type == ArtifactType::Test {
    args.push("--ohos-test-coverage".to_string());
    args.push("false".to_string());
  }
  if artifact_type == ArtifactType::OhosTest {
    args.push("--build-target-name".to_string());
    args.push("ohosTest".to_string());
  }
  if artifact_type == ArtifactType::Hqf {
    args.push("--action".to_string());
    args.push("assemble-quickfix".to_string());
  }
  args
}

fn repo_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("crates directory")
    .parent()
    .expect("repo root")
    .to_path_buf()
}

fn report_path_for(repo_root: &Path, fixture_name: &str) -> PathBuf {
  std::env::var_os(ENV_REPORT)
    .map(PathBuf::from)
    .unwrap_or_else(|| repo_root.join(format!("target/parity/{fixture_name}-hvigor-diff.json")))
}

fn hvigorw_program(project_root: &Path) -> io::Result<OsString> {
  let candidate = if let Some(path) = std::env::var_os(ENV_HVIGORW) {
    path
  } else {
    let local = project_root.join("hvigorw");
    if local.exists() {
      local.into_os_string()
    } else {
      OsString::from("hvigorw")
    }
  };
  validate_hvigorw_program(&candidate)?;
  Ok(candidate)
}

fn validate_hvigorw_program(candidate: &OsStr) -> io::Result<()> {
  let candidate_path = Path::new(candidate);
  let resolved = if candidate_path.components().count() > 1 || candidate_path.is_absolute() {
    Some(candidate_path.to_path_buf())
  } else {
    std::env::var_os("PATH").and_then(|paths| {
      std::env::split_paths(&paths)
        .map(|directory| directory.join(candidate_path))
        .find(|path| path.is_file())
    })
  };
  let Some(resolved) = resolved else {
    return Ok(());
  };
  let canonical = fs::canonicalize(&resolved).unwrap_or(resolved);
  let marker = fs::read_to_string(&canonical).unwrap_or_default();
  if canonical
    .to_string_lossy()
    .contains("/node_modules/@ohos-rs/arkdown/")
    || marker.contains("HVIGOR_PARITY_VERSION")
    || marker.contains("parseArkdownBuildCliArgs")
  {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!(
        "{ENV_HVIGORW} resolved to the arkdown compatibility CLI at {}; point it to the real DevEco hvigorw executable",
        canonical.display()
      ),
    ));
  }
  Ok(())
}

fn arkdown_program() -> OsString {
  std::env::var_os("NODE_HOME")
    .map(PathBuf::from)
    .map(|home| home.join(arkdown_path::HostPlatform::CURRENT.executable_name("node")))
    .map(PathBuf::into_os_string)
    .unwrap_or_else(|| OsString::from(arkdown_path::HostPlatform::CURRENT.executable_name("node")))
}

fn arkdown_cli_path() -> PathBuf {
  std::env::var_os(ENV_ARKDOWN_CLI)
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("js/arkdown-cli.js"))
}

fn env_truthy(name: &str) -> bool {
  matches!(
    std::env::var(name).ok().as_deref(),
    Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
  )
}

fn copy_clean_fixture(src: &Path, dst: &Path) -> io::Result<()> {
  for entry in WalkDir::new(src)
    .follow_links(false)
    .into_iter()
    .filter_entry(|entry| should_copy_fixture_entry(src, entry.path()))
  {
    let entry = entry?;
    let rel = entry.path().strip_prefix(src).expect("entry under fixture");
    if rel.as_os_str().is_empty() {
      continue;
    }

    let target = dst.join(rel);
    let file_type = entry.file_type();
    if file_type.is_dir() {
      fs::create_dir_all(&target)?;
    } else if file_type.is_symlink() {
      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
      }
      copy_symlink(entry.path(), &target)?;
    } else if file_type.is_file() {
      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::copy(entry.path(), &target)?;
      fs::set_permissions(&target, fs::metadata(entry.path())?.permissions())?;
    }
  }
  Ok(())
}

fn materialize_local_file_dependencies(root: &Path) -> io::Result<()> {
  let manifests = WalkDir::new(root)
    .follow_links(false)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file() && entry.file_name() == "oh-package.json5")
    .map(|entry| entry.into_path())
    .collect::<Vec<_>>();
  let canonical_root = fs::canonicalize(root)?;

  for manifest_path in manifests {
    let manifest_text = fs::read_to_string(&manifest_path)?;
    let manifest: Value = json5::from_str(&manifest_text).map_err(|error| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
          "invalid fixture manifest {}: {error}",
          manifest_path.display()
        ),
      )
    })?;
    let module_root = manifest_path.parent().expect("manifest has parent");
    for group in ["dependencies", "devDependencies", "dynamicDependencies"] {
      let Some(dependencies) = manifest.get(group).and_then(Value::as_object) else {
        continue;
      };
      for (package_name, specifier) in dependencies {
        let Some(relative) = specifier
          .as_str()
          .and_then(|value| value.strip_prefix("file:"))
        else {
          continue;
        };
        let source = fs::canonicalize(module_root.join(relative))?;
        if !source.starts_with(&canonical_root) || !source.is_dir() {
          return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
              "fixture local dependency {package_name} resolves outside {}",
              root.display()
            ),
          ));
        }
        let package_path = Path::new(package_name);
        if package_path.is_absolute()
          || package_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
          return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid fixture package name: {package_name}"),
          ));
        }
        let destination = module_root.join("oh_modules").join(package_path);
        if destination.exists() {
          continue;
        }
        fs::create_dir_all(
          destination
            .parent()
            .expect("dependency destination has parent"),
        )?;
        materialize_local_dependency(&source, &destination)?;
      }
    }
  }
  Ok(())
}

#[cfg(unix)]
fn materialize_local_dependency(source: &Path, destination: &Path) -> io::Result<()> {
  std::os::unix::fs::symlink(source, destination)
}

#[cfg(windows)]
fn materialize_local_dependency(source: &Path, destination: &Path) -> io::Result<()> {
  copy_clean_fixture(source, destination)
}

fn should_copy_fixture_entry(root: &Path, path: &Path) -> bool {
  let Ok(rel) = path.strip_prefix(root) else {
    return true;
  };
  !rel.components().any(|component| {
    let Component::Normal(name) = component else {
      return false;
    };
    // `.test` is the local unit-test build root — stale local test builds
    // embed the repo path into generated templates/source maps.
    name == "build" || name == ".hvigor" || name == ".idea" || name == ".test"
  })
}

#[cfg(unix)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
  std::os::unix::fs::symlink(fs::read_link(src)?, dst)
}

#[cfg(windows)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
  let target = fs::read_link(src)?;
  if src.metadata()?.is_dir() {
    std::os::windows::fs::symlink_dir(target, dst)
  } else {
    std::os::windows::fs::symlink_file(target, dst)
  }
}

#[derive(Clone)]
struct CommandReport {
  tool: String,
  command: Vec<String>,
  cwd: PathBuf,
  status: String,
  success: bool,
  exit_code: Option<i32>,
  duration_ms: u128,
  stdout_tail: String,
  stderr_tail: String,
  error: Option<String>,
}

fn run_command<T: AsRef<OsStr>>(
  tool: &str,
  program: &OsStr,
  args: &[T],
  cwd: &Path,
) -> CommandReport {
  let start = Instant::now();
  let command = std::iter::once(program.to_string_lossy().into_owned())
    .chain(
      args
        .iter()
        .map(|arg| arg.as_ref().to_string_lossy().into_owned()),
    )
    .collect::<Vec<_>>();

  match Command::new(program).args(args).current_dir(cwd).output() {
    Ok(output) => CommandReport {
      tool: tool.to_string(),
      command,
      cwd: cwd.to_path_buf(),
      status: "ran".to_string(),
      success: output.status.success(),
      exit_code: output.status.code(),
      duration_ms: start.elapsed().as_millis(),
      stdout_tail: tail_string(&output.stdout, 16_384),
      stderr_tail: tail_string(&output.stderr, 16_384),
      error: None,
    },
    Err(err) if err.kind() == io::ErrorKind::NotFound => CommandReport {
      tool: tool.to_string(),
      command,
      cwd: cwd.to_path_buf(),
      status: "not_found".to_string(),
      success: false,
      exit_code: None,
      duration_ms: start.elapsed().as_millis(),
      stdout_tail: String::new(),
      stderr_tail: String::new(),
      error: Some(err.to_string()),
    },
    Err(err) => CommandReport {
      tool: tool.to_string(),
      command,
      cwd: cwd.to_path_buf(),
      status: "failed_to_start".to_string(),
      success: false,
      exit_code: None,
      duration_ms: start.elapsed().as_millis(),
      stdout_tail: String::new(),
      stderr_tail: String::new(),
      error: Some(err.to_string()),
    },
  }
}

fn tail_string(bytes: &[u8], max_len: usize) -> String {
  let start = bytes.len().saturating_sub(max_len);
  String::from_utf8_lossy(&bytes[start..]).into_owned()
}

impl CommandReport {
  fn output_text(&self) -> String {
    format!("{}\n{}", self.stdout_tail, self.stderr_tail)
  }

  fn to_json(&self) -> Value {
    json!({
        "tool": self.tool,
        "command": self.command,
        "cwd": self.cwd.to_string_lossy(),
        "status": self.status,
        "success": self.success,
        "exit_code": self.exit_code,
        "duration_ms": self.duration_ms,
        "stdout_tail": self.stdout_tail,
        "stderr_tail": self.stderr_tail,
        "error": self.error,
    })
  }
}

#[derive(Clone)]
struct BuildSnapshot {
  tool: String,
  root: PathBuf,
  command: CommandReport,
  archive: Option<ArchiveSnapshot>,
  artifacts: BTreeMap<String, ArtifactSnapshot>,
  build_files: BTreeMap<String, ArtifactSnapshot>,
  loader_out_js: BTreeMap<String, ArtifactSnapshot>,
}

impl BuildSnapshot {
  fn to_json(&self) -> Value {
    let artifacts = self
      .artifacts
      .iter()
      .map(|(name, artifact)| (name.clone(), artifact.to_json()))
      .collect::<serde_json::Map<_, _>>();
    let loader_out_js = self
      .loader_out_js
      .iter()
      .map(|(name, artifact)| (name.clone(), artifact.to_json()))
      .collect::<serde_json::Map<_, _>>();
    let build_files = self
      .build_files
      .iter()
      .map(|(name, artifact)| (name.clone(), artifact.to_json()))
      .collect::<serde_json::Map<_, _>>();

    json!({
        "tool": self.tool,
        "root": self.root.to_string_lossy(),
        "command": self.command.to_json(),
        "archive": self.archive.as_ref().map(ArchiveSnapshot::to_json),
        "artifacts": artifacts,
        "build_files": build_files,
        "loader_out_js": loader_out_js,
    })
  }
}

fn collect_build(
  tool: &str,
  root: &Path,
  command: CommandReport,
  artifact_type: ArtifactType,
) -> Result<BuildSnapshot, Box<dyn std::error::Error>> {
  let archive = find_archive(root, artifact_type.ext(), tool)
    .map(|path| read_archive_snapshot(&path, root, artifact_type))
    .transpose()?;
  let mut artifacts = BTreeMap::new();

  match artifact_type {
    ArtifactType::Hap | ArtifactType::Hsp => {
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "module.json",
        Some("module.json"),
        &[
          "intermediates/package",
          "intermediates/res",
          "intermediates/process_profile",
        ],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "pack.info",
        Some("pack.info"),
        &["outputs", "build/default"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "pkgContextInfo.json",
        Some("pkgContextInfo.json"),
        &["pkgContextInfo", "outputs", "build/default"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "loader.json",
        None,
        &["intermediates/loader"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "modules.abc",
        Some("ets/modules.abc"),
        &["intermediates/ets", "intermediates/loader_out"],
        true,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "sourceMaps.map",
        Some("ets/sourceMaps.map"),
        &["intermediates/source_map", "intermediates/loader_out"],
        false,
      )?;
      // Resource compiler config — important for verifying effective
      // resource directories and dependency resource tables.
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "resConfig.json",
        None,
        &["intermediates/res", "build/default"],
        false,
      )?;
      // CompileArkTS handoff: filesInfo.txt lists all modules fed to es2abc.
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "filesInfo.txt",
        None,
        &["cache/default/default@CompileArkTS", "cache/default"],
        false,
      )?;
      // npmEntries.txt: package module entries written by the bundler.
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "npmEntries.txt",
        None,
        &["cache/default/default@CompileArkTS", "cache/default"],
        false,
      )?;
      // modules.cache: module → protoBin cache paths for es2abc incremental cache.
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "modules.cache",
        None,
        &["cache/default/default@CompileArkTS", "cache/default"],
        false,
      )?;
      // importee.json: compiledImportees / nonEntryImportees (includes BC HAR OHM URLs).
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "importee.json",
        None,
        &["intermediates/importee"],
        false,
      )?;
      // Widget (form card) artifacts: the widget compile pass emits
      // `widgets.abc`, the widget loader JSON with `pkgName2OtherCompileFiles`,
      // and the widget component collection consumed by the IDE.
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "widgets.abc",
        Some("ets/widgets.abc"),
        &["intermediates/loader_out"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "widget_loader.json",
        None,
        &["intermediates/loader"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "component_collection.json",
        None,
        &["intermediates/loader_out"],
        false,
      )?;
      // Section 3.5 "扩展 oracle 收集 cache root 所有文件":
      // Collect ALL files from the CompileArkTS cache root so the oracle
      // can compare every intermediate file (handoff .ts/.js, .protoBin,
      // filesInfo.txt, modules.cache, npmEntries.txt) against hvigor.
      collect_all_cache_root_files(&mut artifacts, root)?;
    }
    ArtifactType::App => {
      // MakeProjectPackInfo/ProcessProjectPrivacyProfile/PackageApp own these
      // project-level entries. Nested HAP/HSP payloads are compared recursively
      // by the ZIP normalization used for the APP archive.
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "pack.info",
        Some("pack.info"),
        &["intermediates/app", "build/outputs"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "pac.json",
        Some("pac.json"),
        &["intermediates/privacy_profile", "build/outputs"],
        false,
      )?;
    }
    ArtifactType::Hqf => {
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "patch.json",
        Some("patch.json"),
        &["intermediates/patch"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "modules.abc",
        Some("ets/modules.abc"),
        &["intermediates/patch"],
        true,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "sourceMaps.map",
        Some("ets/sourceMaps.map"),
        &["intermediates/patch"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "quickfix.json",
        None,
        &["outputs/default"],
        false,
      )?;
    }
    ArtifactType::Har => {
      // HAR archives contain oh-package.json5, source ETS files, declaration stubs,
      // and optionally index.abc
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "oh-package.json5",
        Some("package/oh-package.json5"),
        &["outputs/har", "build/default/outputs"],
        false,
      )?;
      // HAR declaration stubs (hvigor processDeclarationEntry).
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "index.d.ts",
        Some("package/index.d.ts"),
        &["intermediates/har", "build/default/outputs/har"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "index.d.ets",
        Some("package/index.d.ets"),
        &["intermediates/har", "build/default/outputs/har"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "loader.json",
        None,
        &["intermediates/loader"],
        false,
      )?;
      // Bytecode HAR: index.abc at archive root; source HAR may omit it
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "index.abc",
        Some("package/index.abc"),
        &["intermediates/ets", "intermediates/loader_out"],
        true,
      )?;
      // ByteCode HAR's current package entry is `ets/modules.abc`.
      // Keep `index.abc` above for legacy SDK layouts, but compare the modern
      // producer artifact directly as well.
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "modules.abc",
        Some("ets/modules.abc"),
        &["intermediates/ets", "intermediates/loader_out"],
        true,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        archive.as_ref(),
        "sourceMaps.map",
        Some("package/sourceMaps.map"),
        &["intermediates/source_map", "intermediates/loader_out"],
        false,
      )?;
      // HAR uses `default@HarCompileArkTS`, not the HAP/HSP task cache name.
      // Include its full compiler handoff root in the strict golden.
      collect_all_cache_root_files(&mut artifacts, root)?;
    }
    ArtifactType::OhosTest => {
      // ohosTest target HAP: compare the packaged artifacts plus profiles
      // under the `ohosTest` target directories. `modules.abc` debug-info
      // line numbers differ by template formatting (same D-class item as
      // unit-test empty function bodies); normalize those compiler-owned
      // fields before strict comparison.
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "module.json",
        Some("module.json"),
        &[
          "intermediates/process_profile/ohosTest",
          "intermediates/res/ohosTest",
        ],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "loader.json",
        Some("loader.json"),
        &["intermediates/loader/ohosTest"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "merge_profile.json",
        Some("merge_profile.json"),
        &["intermediates/merge_profile/ohosTest"],
        false,
      )?;
    }
    ArtifactType::Test => {
      // Local unit-test build root: generated templates, TestAbility-injected
      // profiles, loader, page override and compiled assets. `modules.abc`
      // debug-info line numbers differ by the empty-function-body formatting
      // (hvigor TS emit `{\n}` vs OXC `{ }`), so it is compared structurally
      // below rather than byte-for-byte here.
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "TestAbility.ets",
        None,
        &[".test/testability"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "Index.ets",
        None,
        &[".test/testability/pages"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "phoneSettingConfig_Phone.json",
        None,
        &[".test/testability"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "module.json",
        Some("module.json"),
        &["intermediates/process_profile", "intermediates/res"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "ark_module.json",
        Some("ark_module.json"),
        &["intermediates/res"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "loader.json",
        Some("loader.json"),
        &["intermediates/loader"],
        false,
      )?;
      collect_named_artifact(
        &mut artifacts,
        root,
        None,
        "main_pages.json",
        Some("main_pages.json"),
        &["intermediates/res/resources/base/profile"],
        false,
      )?;
      // `sourceMaps.map` module set and sources are identical, but the
      // `.test/testability/pages/Index.ts` mappings differ in the
      // empty-function-body formatting (hvigor TS emit `{\n}` vs OXC
      // `{ }`) — same known difference as the abc debug-info line numbers.
    }
  }

  // `filesInfo.txt` and `modules.cache` are unordered line manifests because
  // current ets2bundle fills them as concurrent resolver work completes.
  // ArtifactNormalization handles that source-owned nondeterminism, so test
  // targets retain the same complete build/cache comparison as normal modules.
  let build_files = if artifact_type == ArtifactType::Hqf {
    collect_hqf_build_files(root)?
  } else {
    collect_build_default_files(root)?
  };
  let loader_out_js = if artifact_type == ArtifactType::Hqf {
    BTreeMap::new()
  } else {
    collect_loader_out_js(root)?
  };

  Ok(BuildSnapshot {
    tool: tool.to_string(),
    root: root.to_path_buf(),
    command,
    archive,
    artifacts,
    build_files,
    loader_out_js,
  })
}

fn find_archive(root: &Path, ext: &str, tool: &str) -> Option<PathBuf> {
  let mut archives = WalkDir::new(root)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file())
    .filter(|entry| entry.path().extension().and_then(OsStr::to_str) == Some(ext))
    .map(|entry| entry.path().to_path_buf())
    .collect::<Vec<_>>();

  archives.sort_by(|a, b| {
    let a_score = archive_priority(root, a, tool, ext);
    let b_score = archive_priority(root, b, tool, ext);
    a_score.cmp(&b_score).then_with(|| a.cmp(b))
  });
  archives.into_iter().next()
}

fn archive_priority(root: &Path, path: &Path, tool: &str, ext: &str) -> u8 {
  let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
  let canonical = format!("build/default/outputs/default/entry.{ext}");
  if tool == "arkdown" && rel == canonical {
    return 0;
  }
  if rel.contains("-signed.") || rel.ends_with("signed.hap") || rel.ends_with("signed.hsp") {
    return 1;
  }
  if rel == canonical {
    return 2;
  }
  // HAR is typically in outputs/har/
  let har_canonical = format!("build/default/outputs/har/entry.{ext}");
  if rel == har_canonical {
    return 2;
  }
  if rel.contains("unsigned") {
    return 3;
  }
  4
}

#[derive(Clone)]
struct ArchiveSnapshot {
  path: PathBuf,
  entries: BTreeMap<String, ZipEntrySnapshot>,
  contents: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone)]
struct ZipEntrySnapshot {
  size: u64,
  compressed_size: u64,
  content_hash: String,
}

impl ArchiveSnapshot {
  fn to_json(&self) -> Value {
    let entries = self
      .entries
      .iter()
      .map(|(name, entry)| {
        (
          name.clone(),
          json!({
              "size": entry.size,
              "compressed_size": entry.compressed_size,
              "content_hash": entry.content_hash,
          }),
        )
      })
      .collect::<serde_json::Map<_, _>>();

    json!({
        "path": self.path.to_string_lossy(),
        "entry_count": self.entries.len(),
        "entries": entries,
    })
  }
}

fn read_archive_snapshot(
  path: &Path,
  root: &Path,
  artifact_type: ArtifactType,
) -> Result<ArchiveSnapshot, Box<dyn std::error::Error>> {
  if artifact_type == ArtifactType::Har {
    return read_har_archive_snapshot(path, root);
  }
  let file = fs::File::open(path)?;
  let mut archive = ZipArchive::new(file)?;
  let mut entries = BTreeMap::new();
  let mut contents = BTreeMap::new();

  for index in 0..archive.len() {
    let mut file = archive.by_index(index)?;
    if file.is_dir() {
      entries.insert(
        file.name().to_string(),
        ZipEntrySnapshot {
          size: 0,
          compressed_size: file.compressed_size(),
          content_hash: stable_hash_hex(&[]),
        },
      );
      continue;
    }

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let member_normalization = ArtifactNormalization::for_path(Path::new(file.name()));
    let normalized = match String::from_utf8(normalize_project_bytes(&bytes, root)) {
      Ok(text) => member_normalization.normalize_text(text).into_bytes(),
      Err(error) => member_normalization.normalize_binary(error.into_bytes()),
    };
    entries.insert(
      file.name().to_string(),
      ZipEntrySnapshot {
        size: file.size(),
        compressed_size: file.compressed_size(),
        content_hash: stable_hash_hex(&normalized),
      },
    );
    contents.insert(file.name().to_string(), bytes);
  }

  Ok(ArchiveSnapshot {
    path: path.to_path_buf(),
    entries,
    contents,
  })
}

fn read_har_archive_snapshot(
  path: &Path,
  root: &Path,
) -> Result<ArchiveSnapshot, Box<dyn std::error::Error>> {
  let file = fs::File::open(path)?;
  let gzip = flate2::read::GzDecoder::new(file);
  let mut archive = tar::Archive::new(gzip);
  let mut entries = BTreeMap::new();
  let mut contents = BTreeMap::new();

  for entry in archive.entries()? {
    let mut entry = entry?;
    let name = entry.path()?.to_string_lossy().into_owned();
    let size = entry.size();
    let mut bytes = Vec::new();
    if entry.header().entry_type().is_file() {
      entry.read_to_end(&mut bytes)?;
      contents.insert(name.clone(), bytes.clone());
    }
    let normalized = match String::from_utf8(normalize_project_bytes(&bytes, root)) {
      Ok(text) => ArtifactNormalization::for_path(Path::new(&name))
        .normalize_text(text)
        .into_bytes(),
      Err(error) => error.into_bytes(),
    };
    entries.insert(
      name,
      ZipEntrySnapshot {
        size,
        // TAR does not expose a meaningful per-entry compressed size.
        // Content size still detects truncation while the raw container
        // normalization below validates that the artifact remains gzip+tar.
        compressed_size: size,
        content_hash: stable_hash_hex(&normalized),
      },
    );
  }

  Ok(ArchiveSnapshot {
    path: path.to_path_buf(),
    entries,
    contents,
  })
}

fn collect_named_artifact(
  artifacts: &mut BTreeMap<String, ArtifactSnapshot>,
  root: &Path,
  archive: Option<&ArchiveSnapshot>,
  artifact_name: &str,
  archive_entry: Option<&str>,
  preferred_path_fragments: &[&str],
  binary: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  let normalization = ArtifactNormalization::for_file_name(OsStr::new(artifact_name));
  if let (Some(archive), Some(entry_name)) = (archive, archive_entry) {
    if let Some(bytes) = archive.contents.get(entry_name) {
      artifacts.insert(
        artifact_name.to_string(),
        ArtifactSnapshot::from_bytes_with_normalization(
          format!("archive:{entry_name}"),
          bytes.clone(),
          root,
          binary,
          normalization,
        ),
      );
      return Ok(());
    }
  }

  if let Some(path) = find_named_file(root, artifact_name, preferred_path_fragments) {
    let bytes = fs::read(&path)?;
    artifacts.insert(
      artifact_name.to_string(),
      ArtifactSnapshot::from_bytes_with_normalization(
        rel_source(root, &path),
        bytes,
        root,
        binary,
        normalization,
      ),
    );
  }
  Ok(())
}

fn find_named_file(root: &Path, file_name: &str, preferred_fragments: &[&str]) -> Option<PathBuf> {
  let mut candidates = WalkDir::new(root)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file())
    .filter(|entry| entry.file_name() == OsStr::new(file_name))
    .map(|entry| entry.path().to_path_buf())
    .collect::<Vec<_>>();

  candidates.sort_by(|a, b| {
    let a_score = preferred_path_score(root, a, preferred_fragments);
    let b_score = preferred_path_score(root, b, preferred_fragments);
    a_score.cmp(&b_score).then_with(|| a.cmp(b))
  });
  candidates.into_iter().next()
}

fn preferred_path_score(root: &Path, path: &Path, fragments: &[&str]) -> usize {
  let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
  fragments
    .iter()
    .position(|fragment| rel.contains(fragment))
    .unwrap_or(fragments.len())
}

fn collect_build_default_files(
  root: &Path,
) -> Result<BTreeMap<String, ArtifactSnapshot>, Box<dyn std::error::Error>> {
  let mut files = BTreeMap::new();
  for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
    if !entry.file_type().is_file() {
      continue;
    }

    let Some(key) = build_default_key(root, entry.path()) else {
      continue;
    };
    let bytes = fs::read(entry.path())?;
    let mut unique_key = key.clone();
    if files.contains_key(&unique_key) {
      unique_key = rel_source(root, entry.path());
    }
    files.insert(
      unique_key,
      ArtifactSnapshot::from_bytes_with_normalization(
        rel_source(root, entry.path()),
        bytes,
        root,
        is_binary_build_file(entry.path()),
        ArtifactNormalization::for_path(entry.path()),
      ),
    );
  }
  Ok(files)
}

fn collect_hqf_build_files(
  root: &Path,
) -> Result<BTreeMap<String, ArtifactSnapshot>, Box<dyn std::error::Error>> {
  Ok(
    collect_build_default_files(root)?
      .into_iter()
      .filter(|(path, _)| {
        path.contains("/intermediates/patch/")
          || (path.contains("/outputs/")
            && matches!(
              Path::new(path).extension().and_then(OsStr::to_str),
              Some("json" | "hqf")
            ))
      })
      .collect(),
  )
}

fn build_default_key(root: &Path, path: &Path) -> Option<String> {
  let rel = path.strip_prefix(root).ok()?;
  let components = rel
    .components()
    .filter_map(|component| {
      let Component::Normal(name) = component else {
        return None;
      };
      Some(name.to_string_lossy().into_owned())
    })
    .collect::<Vec<_>>();

  for index in 0..components.len().saturating_sub(2) {
    if matches!(components[index].as_str(), "build" | ".test")
      && components[index + 1] == "default"
      && matches!(components[index + 2].as_str(), "intermediates" | "outputs")
    {
      return Some(components[index..].join("/"));
    }
  }
  None
}

fn is_binary_build_file(path: &Path) -> bool {
  matches!(
    path.extension().and_then(OsStr::to_str),
    Some(
      "abc"
        | "hap"
        | "hsp"
        | "app"
        | "hqf"
        | "har"
        | "tgz"
        | "index"
        | "info"
        | "png"
        | "jpg"
        | "jpeg"
        | "webp"
    )
  )
}

fn collect_loader_out_js(
  root: &Path,
) -> Result<BTreeMap<String, ArtifactSnapshot>, Box<dyn std::error::Error>> {
  let mut files = BTreeMap::new();
  for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
    if !entry.file_type().is_file()
      || entry.path().extension().and_then(OsStr::to_str) != Some("js")
      || !entry
        .path()
        .components()
        .any(|component| matches!(component, Component::Normal(name) if name == "loader_out"))
    {
      continue;
    }

    let key = loader_out_key(root, entry.path());
    let bytes = fs::read(entry.path())?;
    let mut unique_key = key.clone();
    if files.contains_key(&unique_key) {
      unique_key = rel_source(root, entry.path());
    }
    files.insert(
      unique_key,
      ArtifactSnapshot::from_bytes(rel_source(root, entry.path()), bytes, root, false),
    );
  }
  Ok(files)
}

fn loader_out_key(root: &Path, path: &Path) -> String {
  let rel = path.strip_prefix(root).unwrap_or(path);
  let mut after_loader_out = Vec::new();
  let mut seen_loader_out = false;
  for component in rel.components() {
    let Component::Normal(name) = component else {
      continue;
    };
    if seen_loader_out {
      after_loader_out.push(name.to_string_lossy().into_owned());
    } else if name == "loader_out" {
      seen_loader_out = true;
    }
  }

  if after_loader_out.first().map(String::as_str) == Some("default")
    && after_loader_out.get(1).map(String::as_str) == Some("ets")
  {
    after_loader_out.drain(0..2);
  }

  if after_loader_out.is_empty() {
    rel.to_string_lossy().into_owned()
  } else {
    after_loader_out.join("/")
  }
}

#[derive(Clone)]
struct ArtifactSnapshot {
  source: String,
  kind: ArtifactKind,
  original_len: usize,
}

#[derive(Clone)]
enum ArtifactKind {
  Json(Value),
  Text(String),
  Binary(Vec<u8>),
}

#[derive(Clone, Copy)]
enum ArtifactNormalization {
  PreserveOrder,
  UnorderedLineManifest,
  JsonObjectInsertionOrder,
  ReloadEtsState,
  CoverageParserInput,
  ZipContainerTimestamps,
  TarContainerMetadata,
}

impl ArtifactNormalization {
  fn for_path(path: &Path) -> Self {
    match path.extension().and_then(OsStr::to_str) {
      Some("hap" | "hsp" | "app" | "hqf") => return Self::ZipContainerTimestamps,
      // `PackageSharedTgz` writes an HSP and its interface HAR into this
      // gzip-compressed tar container. Both hvigor and arkdown intentionally
      // preserve the member payloads, while their compression libraries and
      // build-local archive timestamps are not a parity contract.
      Some("har" | "tgz") => return Self::TarContainerMetadata,
      _ => {}
    }
    if path.file_name().and_then(OsStr::to_str) == Some("ets.json")
      && path
        .components()
        .any(|component| matches!(component, Component::Normal(name) if name == "patch"))
    {
      return Self::ReloadEtsState;
    }
    path
      .file_name()
      .map_or(Self::PreserveOrder, Self::for_file_name)
  }

  fn for_file_name(file_name: &OsStr) -> Self {
    if matches!(file_name.to_str(), Some("filesInfo.txt" | "modules.cache")) {
      Self::UnorderedLineManifest
    } else if matches!(file_name.to_str(), Some("coverage.log")) {
      // @ohos/coverage localTest/previewer.js appends raw Previewer stdout to
      // this file. coverageParser.js consumes only the OHOS_REPORT records and
      // an engine-error stack sequence; timestamps, PIDs, thread IDs, and
      // unrelated engine diagnostics are not build-output semantics.
      Self::CoverageParserInput
    } else if matches!(
      file_name.to_str(),
      Some("sourceMaps.map" | "sourceMaps.json")
    ) {
      // `generate_sourcemap.ts::SourceMapGenerator.updateSpecifySourceMap`
      // assigns entries to a plain object as concurrent Rollup transforms
      // finish. Repeated hvigor builds therefore retain identical maps in a
      // different top-level insertion order.
      Self::JsonObjectInsertionOrder
    } else {
      Self::PreserveOrder
    }
  }

  fn normalize_text(self, text: String) -> String {
    match self {
      Self::PreserveOrder => text,
      Self::UnorderedLineManifest => {
        let has_trailing_newline = text.ends_with('\n');
        let mut lines = text.split_terminator('\n').collect::<Vec<_>>();
        lines.sort_unstable();
        let mut normalized = lines.join("\n");
        if has_trailing_newline {
          normalized.push('\n');
        }
        normalized
      }
      Self::JsonObjectInsertionOrder => serde_json::from_str::<BTreeMap<String, Value>>(&text)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or(text),
      Self::ReloadEtsState => serde_json::from_str::<Value>(&text)
        .and_then(|mut value| {
          if let Some(object) = value.as_object_mut() {
            if object.contains_key("changedFileList") {
              object.insert(
                "changedFileList".to_string(),
                Value::String("<MTIME_MS>".to_string()),
              );
            }
          }
          serde_json::to_string_pretty(&value)
        })
        .unwrap_or(text),
      Self::CoverageParserInput => Self::normalize_coverage_parser_input(&text),
      Self::ZipContainerTimestamps | Self::TarContainerMetadata => text,
    }
  }

  fn normalize_coverage_parser_input(text: &str) -> String {
    const ENGINE_ERROR: &str = "[Engine Log]Error message:";
    const ENGINE_LOG: &str = "[Engine Log]";
    const REPORT_TAGS: [&str; 5] = [
      "OHOS_REPORT_COVERAGE_DATA: ",
      "OHOS_REPORT_STATUS: ",
      "OHOS_REPORT_STATUS_CODE: ",
      "OHOS_REPORT_RESULT: stream=",
      "OHOS_REPORT_COVERAGE_PATH: ",
    ];
    const DURATION_PREFIXES: [&str; 3] = [
      "OHOS_REPORT_STATUS: consuming=",
      "OHOS_REPORT_STATUS: suiteconsuming=",
      "OHOS_REPORT_STATUS: taskconsuming=",
    ];

    let mut engine_error = false;
    let mut records = Vec::<String>::new();
    for line in text.lines() {
      if let Some(index) = line.find(ENGINE_ERROR) {
        engine_error = true;
        records.push(line[index..].to_string());
        continue;
      }
      if engine_error {
        if let Some(index) = line.rfind(ENGINE_LOG) {
          records.push(line[index..].to_string());
          continue;
        }
        engine_error = false;
      }
      let Some(index) = REPORT_TAGS.iter().filter_map(|tag| line.find(tag)).min() else {
        continue;
      };
      let record = &line[index..];
      // Hypium's OhReport writes wall-clock durations into these status
      // records. coverageParser.js does not read them: its consumers are
      // class/test, result/status-code, stack/stream and coverage records.
      // Preserve the status shape while removing run-to-run scheduler timing.
      if let Some(prefix) = DURATION_PREFIXES
        .iter()
        .find(|prefix| record.starts_with(**prefix))
      {
        records.push(format!("{prefix}<DURATION>"));
      } else {
        records.push(record.to_string());
      }
    }

    let mut normalized = records.join("\n");
    if !normalized.is_empty() {
      normalized.push('\n');
    }
    normalized
  }

  fn normalize_binary(self, bytes: Vec<u8>) -> Vec<u8> {
    if matches!(self, Self::TarContainerMetadata) {
      return Self::normalize_tar_container(&bytes).unwrap_or(bytes);
    }
    if !matches!(self, Self::ZipContainerTimestamps) {
      return bytes;
    }
    Self::normalize_zip_container(&bytes).unwrap_or(bytes)
  }

  fn normalize_zip_container(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut members = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
      let mut entry = archive.by_index(index).ok()?;
      let path = entry.name().to_string();
      let is_dir = entry.is_dir();
      let compression = format!("{:?}", entry.compression());
      let unix_mode = entry.unix_mode();
      let compressed_size = entry.compressed_size();
      let mut content = Vec::new();
      entry.read_to_end(&mut content).ok()?;
      let member_normalization = Self::for_path(Path::new(&path));
      let content = match member_normalization {
        Self::UnorderedLineManifest
        | Self::JsonObjectInsertionOrder
        | Self::ReloadEtsState
        | Self::CoverageParserInput => String::from_utf8(content)
          .map(|text| member_normalization.normalize_text(text).into_bytes())
          .ok()?,
        _ => member_normalization.normalize_binary(content),
      };
      members.push((
        path,
        is_dir,
        compression,
        unix_mode,
        compressed_size,
        content,
      ));
    }
    // app_packing_tool receives a set of package inputs. Its ZIP timestamp and
    // member order are container metadata, while the members and their storage
    // properties are the package contract. `sourceMaps.map` additionally uses
    // the source-owned top-level object normalization above because
    // generate_sourcemap.ts fills it as concurrent transforms complete.
    members.sort_by(|left, right| left.0.cmp(&right.0));

    let mut normalized = Vec::new();
    for (path, is_dir, compression, unix_mode, compressed_size, content) in members {
      normalized.extend_from_slice(&(path.len() as u64).to_le_bytes());
      normalized.extend_from_slice(path.as_bytes());
      normalized.push(u8::from(is_dir));
      normalized.extend_from_slice(&(compression.len() as u64).to_le_bytes());
      normalized.extend_from_slice(compression.as_bytes());
      normalized.extend_from_slice(&unix_mode.unwrap_or_default().to_le_bytes());
      normalized.extend_from_slice(&compressed_size.to_le_bytes());
      normalized.extend_from_slice(&(content.len() as u64).to_le_bytes());
      normalized.extend_from_slice(&content);
    }
    Some(normalized)
  }

  fn normalize_tar_container(bytes: &[u8]) -> Option<Vec<u8>> {
    let gzip = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(gzip);
    let mut members = BTreeMap::new();
    for entry in archive.entries().ok()? {
      let mut entry = entry.ok()?;
      let path = entry.path().ok()?.to_string_lossy().into_owned();
      let entry_type = entry.header().entry_type().as_byte();
      let mut content = Vec::new();
      entry.read_to_end(&mut content).ok()?;
      let member_normalization = Self::for_path(Path::new(&path));
      let content = match member_normalization {
        Self::UnorderedLineManifest
        | Self::JsonObjectInsertionOrder
        | Self::ReloadEtsState
        | Self::CoverageParserInput => String::from_utf8(content)
          .map(|text| member_normalization.normalize_text(text).into_bytes())
          .ok()?,
        _ => member_normalization.normalize_binary(content),
      };
      members.insert(path, (entry_type, content));
    }

    let mut normalized = Vec::new();
    for (path, (entry_type, content)) in members {
      normalized.extend_from_slice(&(path.len() as u64).to_le_bytes());
      normalized.extend_from_slice(path.as_bytes());
      normalized.push(entry_type);
      normalized.extend_from_slice(&(content.len() as u64).to_le_bytes());
      normalized.extend_from_slice(&content);
    }
    Some(normalized)
  }
}

impl ArtifactSnapshot {
  fn from_bytes(source: String, bytes: Vec<u8>, root: &Path, force_binary: bool) -> Self {
    Self::from_bytes_with_normalization(
      source,
      bytes,
      root,
      force_binary,
      ArtifactNormalization::PreserveOrder,
    )
  }

  fn from_bytes_with_normalization(
    source: String,
    bytes: Vec<u8>,
    root: &Path,
    force_binary: bool,
    normalization: ArtifactNormalization,
  ) -> Self {
    let original_len = bytes.len();
    if force_binary {
      return ArtifactSnapshot {
        source,
        kind: ArtifactKind::Binary(normalization.normalize_binary(bytes)),
        original_len,
      };
    }

    match String::from_utf8(bytes) {
      Ok(text) => {
        let normalized = normalization.normalize_text(normalize_project_text(&text, root));
        match serde_json::from_str::<Value>(&normalized) {
          Ok(value) => ArtifactSnapshot {
            source,
            kind: ArtifactKind::Json(value),
            original_len,
          },
          Err(_) => ArtifactSnapshot {
            source,
            kind: ArtifactKind::Text(normalized),
            original_len,
          },
        }
      }
      Err(err) => ArtifactSnapshot {
        source,
        kind: ArtifactKind::Binary(err.into_bytes()),
        original_len,
      },
    }
  }

  fn to_json(&self) -> Value {
    let normalized = self.normalized_bytes();
    let mut out = json!({
        "source": self.source,
        "kind": self.kind_name(),
        "original_len": self.original_len,
        "normalized_len": normalized.len(),
        "normalized_hash": stable_hash_hex(&normalized),
    });

    if let ArtifactKind::Json(value) = &self.kind {
      out["normalized_json"] = value.clone();
    }

    out
  }

  fn kind_name(&self) -> &'static str {
    match self.kind {
      ArtifactKind::Json(_) => "json",
      ArtifactKind::Text(_) => "text",
      ArtifactKind::Binary(_) => "binary",
    }
  }

  fn normalized_bytes(&self) -> Vec<u8> {
    match &self.kind {
      ArtifactKind::Json(value) => serde_json::to_vec_pretty(value).unwrap_or_default(),
      ArtifactKind::Text(text) => text.as_bytes().to_vec(),
      ArtifactKind::Binary(bytes) => bytes.clone(),
    }
  }
}

fn normalize_project_bytes(bytes: &[u8], root: &Path) -> Vec<u8> {
  match std::str::from_utf8(bytes) {
    Ok(text) => normalize_project_text(text, root).into_bytes(),
    Err(_) => bytes.to_vec(),
  }
}

fn normalize_project_text(text: &str, root: &Path) -> String {
  let mut normalized = text.to_string();
  for candidate in path_spellings(root) {
    normalized = normalized.replace(&candidate, "<PROJECT>");
  }
  normalized
}

fn path_spellings(path: &Path) -> Vec<String> {
  let mut spellings = vec![path.to_string_lossy().into_owned()];
  if let Ok(canonical) = fs::canonicalize(path) {
    let canonical = canonical.to_string_lossy().into_owned();
    if !spellings.contains(&canonical) {
      spellings.push(canonical);
    }
  }
  spellings.sort_by_key(|spelling| std::cmp::Reverse(spelling.len()));
  spellings
}

#[test]
fn compile_arkts_line_manifests_compare_as_unordered_records() {
  let root = tempfile::tempdir().expect("tempdir");
  for file_name in ["filesInfo.txt", "modules.cache"] {
    let normalization = ArtifactNormalization::for_file_name(OsStr::new(file_name));
    let hvigor = ArtifactSnapshot::from_bytes_with_normalization(
      file_name.to_string(),
      b"entry/pages/Index.ts\nentry/entryability/EntryAbility.ts\n".to_vec(),
      root.path(),
      false,
      normalization,
    );
    let arkdown = ArtifactSnapshot::from_bytes_with_normalization(
      file_name.to_string(),
      b"entry/entryability/EntryAbility.ts\nentry/pages/Index.ts\n".to_vec(),
      root.path(),
      false,
      normalization,
    );

    assert_eq!(
      compare_artifact(file_name, &hvigor, &arkdown)["equal"],
      true
    );
  }
}

#[test]
fn source_map_manifest_compares_entries_independent_of_transform_completion_order() {
  // `generate_sourcemap.ts::SourceMapGenerator.updateSpecifySourceMap`
  // assigns to a plain object from concurrent Rollup transforms. Two real
  // hvigor OHOS Test builds produce different key orders with identical map
  // values, so strict parity compares this manifest as a JSON object.
  let normalization = ArtifactNormalization::for_file_name(OsStr::new("sourceMaps.map"));
  let first = normalization.normalize_text(
    r#"{"entry|dep|1.0.0|index.js":{"version":3},"entry|app|1.0.0|Index.ts":{"version":3}}"#
      .to_string(),
  );
  let second = normalization.normalize_text(
    r#"{"entry|app|1.0.0|Index.ts":{"version":3},"entry|dep|1.0.0|index.js":{"version":3}}"#
      .to_string(),
  );

  assert_eq!(first, second);
}

#[test]
fn coverage_log_compares_only_records_consumed_by_the_oh_coverage_parser() {
  // @ohos/coverage localTest/previewer.js persists the raw process stream,
  // while coverageParser.js locates these records with String#indexOf.
  let normalization = ArtifactNormalization::for_file_name(OsStr::new("coverage.log"));
  let hvigor = normalization.normalize_text(
    "09-08 10:11:12.123 100 200 I Previewer unrelated runtime detail\n\
     09-08 10:11:12.124 100 200 I OHOS_REPORT_STATUS: class=ExampleTest\n\
     09-08 10:11:12.125 100 200 I OHOS_REPORT_STATUS: test=passes\n\
     09-08 10:11:12.126 100 200 I OHOS_REPORT_STATUS_CODE: 0\n"
      .to_string(),
  );
  let arkdown = normalization.normalize_text(
    "09-08 11:22:33.456 900 901 D Previewer another diagnostic\n\
     09-08 11:22:33.457 900 901 I OHOS_REPORT_STATUS: class=ExampleTest\n\
     09-08 11:22:33.458 900 901 I OHOS_REPORT_STATUS: test=passes\n\
     09-08 11:22:33.459 900 901 I OHOS_REPORT_STATUS_CODE: 0\n"
      .to_string(),
  );

  assert_eq!(hvigor, arkdown);
  assert_eq!(
    hvigor,
    "OHOS_REPORT_STATUS: class=ExampleTest\n\
     OHOS_REPORT_STATUS: test=passes\n\
     OHOS_REPORT_STATUS_CODE: 0\n"
  );
}

#[test]
fn coverage_log_ignores_hypium_runtime_durations() {
  let normalization = ArtifactNormalization::for_file_name(OsStr::new("coverage.log"));
  let hvigor = normalization.normalize_text(
    "OHOS_REPORT_STATUS: consuming=0\n\
     OHOS_REPORT_STATUS: suiteconsuming=4\n\
     OHOS_REPORT_STATUS: taskconsuming=9\n"
      .to_string(),
  );
  let arkdown = normalization.normalize_text(
    "OHOS_REPORT_STATUS: consuming=1\n\
     OHOS_REPORT_STATUS: suiteconsuming=12\n\
     OHOS_REPORT_STATUS: taskconsuming=31\n"
      .to_string(),
  );

  assert_eq!(hvigor, arkdown);
}

#[test]
fn ordered_compile_arkts_text_keeps_record_order_significant() {
  let root = tempfile::tempdir().expect("tempdir");
  let hvigor = ArtifactSnapshot::from_bytes(
    "npmEntries.txt".to_string(),
    b"entry-a\nentry-b\n".to_vec(),
    root.path(),
    false,
  );
  let arkdown = ArtifactSnapshot::from_bytes(
    "npmEntries.txt".to_string(),
    b"entry-b\nentry-a\n".to_vec(),
    root.path(),
    false,
  );

  assert_eq!(
    compare_artifact("npmEntries.txt", &hvigor, &arkdown)["equal"],
    false
  );
}

#[test]
fn archive_comparison_ignores_only_sdk_generated_dos_timestamps() {
  let archive_bytes = |timestamp: zip::DateTime| {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
      .compression_method(zip::CompressionMethod::Stored)
      .last_modified_time(timestamp);
    writer
      .start_file("module.json", options)
      .expect("start entry");
    writer.write_all(b"{}").expect("write entry");
    writer.finish().expect("finish archive").into_inner()
  };
  let first =
    archive_bytes(zip::DateTime::from_date_and_time(2026, 7, 22, 10, 0, 0).expect("timestamp"));
  let second =
    archive_bytes(zip::DateTime::from_date_and_time(2026, 7, 22, 10, 5, 0).expect("timestamp"));
  let root = tempfile::tempdir().expect("tempdir");
  let normalization = ArtifactNormalization::for_path(Path::new("entry.hap"));
  let hvigor = ArtifactSnapshot::from_bytes_with_normalization(
    "entry.hap".to_string(),
    first,
    root.path(),
    true,
    normalization,
  );
  let arkdown = ArtifactSnapshot::from_bytes_with_normalization(
    "entry.hap".to_string(),
    second,
    root.path(),
    true,
    normalization,
  );
  assert_eq!(
    compare_artifact("entry.hap", &hvigor, &arkdown)["equal"],
    true
  );

  let mut structurally_different = arkdown.normalized_bytes();
  structurally_different[6] ^= 0x08;
  let structurally_different = ArtifactSnapshot::from_bytes_with_normalization(
    "entry.hap".to_string(),
    structurally_different,
    root.path(),
    true,
    normalization,
  );
  assert_eq!(
    compare_artifact("entry.hap", &hvigor, &structurally_different)["equal"],
    false
  );
}

#[test]
fn shared_tgz_comparison_normalizes_nested_hsp_and_interface_har_containers() {
  let zip_bytes = |timestamp: zip::DateTime, module_json: &[u8]| {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
      .compression_method(zip::CompressionMethod::Stored)
      .last_modified_time(timestamp);
    writer
      .start_file("module.json", options)
      .expect("start HSP module.json");
    writer
      .write_all(module_json)
      .expect("write HSP module.json");
    writer.finish().expect("finish HSP").into_inner()
  };
  let gzip_tar = |members: &[(&str, &[u8])], mtime: u32, compression: flate2::Compression| {
    let gzip = flate2::GzBuilder::new()
      .mtime(mtime)
      .write(Vec::new(), compression);
    let mut archive = tar::Builder::new(gzip);
    for (name, content) in members {
      let mut header = tar::Header::new_gnu();
      header.set_entry_type(tar::EntryType::Regular);
      header.set_size(content.len() as u64);
      header.set_mode(0o644);
      header.set_uid(0);
      header.set_gid(0);
      header.set_mtime(u64::from(mtime));
      header.set_cksum();
      archive
        .append_data(&mut header, name, Cursor::new(*content))
        .expect("append tar member");
    }
    archive.finish().expect("finish tar");
    archive
      .into_inner()
      .expect("take gzip writer")
      .finish()
      .expect("finish gzip")
  };

  let first_hsp = zip_bytes(
    zip::DateTime::from_date_and_time(2026, 8, 4, 9, 29, 46).expect("timestamp"),
    br#"{"module":"entry"}"#,
  );
  let second_hsp = zip_bytes(
    zip::DateTime::from_date_and_time(2026, 8, 4, 9, 29, 50).expect("timestamp"),
    br#"{"module":"entry"}"#,
  );
  let first_har = gzip_tar(
    &[
      ("package/Index.d.ets", b"export {};\n"),
      (
        "package/ets/sourceMaps.map",
        br#"{"second":{"version":3},"first":{"version":3}}"#,
      ),
    ],
    1,
    flate2::Compression::fast(),
  );
  let second_har = gzip_tar(
    &[
      ("package/Index.d.ets", b"export {};\n"),
      (
        "package/ets/sourceMaps.map",
        br#"{"first":{"version":3},"second":{"version":3}}"#,
      ),
    ],
    2,
    flate2::Compression::best(),
  );
  let first = gzip_tar(
    &[
      ("entry-default.hsp", first_hsp.as_slice()),
      ("entry.har", first_har.as_slice()),
    ],
    3,
    flate2::Compression::fast(),
  );
  let second = gzip_tar(
    &[
      ("entry-default.hsp", second_hsp.as_slice()),
      ("entry.har", second_har.as_slice()),
    ],
    4,
    flate2::Compression::best(),
  );
  let root = tempfile::tempdir().expect("tempdir");
  let normalization = ArtifactNormalization::for_path(Path::new("entry-default.tgz"));
  let hvigor = ArtifactSnapshot::from_bytes_with_normalization(
    "entry-default.tgz".to_string(),
    first,
    root.path(),
    true,
    normalization,
  );
  let arkdown = ArtifactSnapshot::from_bytes_with_normalization(
    "entry-default.tgz".to_string(),
    second,
    root.path(),
    true,
    normalization,
  );
  assert_eq!(
    compare_artifact("entry-default.tgz", &hvigor, &arkdown)["equal"],
    true
  );

  let changed_hsp = zip_bytes(
    zip::DateTime::from_date_and_time(2026, 8, 4, 9, 29, 50).expect("timestamp"),
    br#"{"module":"changed"}"#,
  );
  let changed = gzip_tar(
    &[
      ("entry-default.hsp", changed_hsp.as_slice()),
      ("entry.har", second_har.as_slice()),
    ],
    4,
    flate2::Compression::best(),
  );
  let changed = ArtifactSnapshot::from_bytes_with_normalization(
    "entry-default.tgz".to_string(),
    changed,
    root.path(),
    true,
    normalization,
  );
  assert_eq!(
    compare_artifact("entry-default.tgz", &hvigor, &changed)["equal"],
    false
  );
}

fn compare_builds(left: &BuildSnapshot, right: &BuildSnapshot) -> Vec<Value> {
  let mut diffs = Vec::new();

  compare_command(&mut diffs, "hvigor", &left.command);
  compare_command(&mut diffs, "arkdown", &right.command);
  compare_archive_entries(&mut diffs, left.archive.as_ref(), right.archive.as_ref());
  compare_artifact_maps(&mut diffs, "artifacts", &left.artifacts, &right.artifacts);
  compare_artifact_maps(
    &mut diffs,
    "build_files",
    &left.build_files,
    &right.build_files,
  );
  compare_artifact_maps(
    &mut diffs,
    "loader_out_js",
    &left.loader_out_js,
    &right.loader_out_js,
  );

  diffs
}

fn compare_command(diffs: &mut Vec<Value>, tool: &str, command: &CommandReport) {
  if command.status != "ran" || !command.success {
    diffs.push(json!({
        "scope": "command",
        "tool": tool,
        "equal": false,
        "status": command.status,
        "exit_code": command.exit_code,
        "error": command.error,
        "stderr_tail": command.stderr_tail,
    }));
  }
}

fn compare_archive_entries(
  diffs: &mut Vec<Value>,
  left: Option<&ArchiveSnapshot>,
  right: Option<&ArchiveSnapshot>,
) {
  if left.is_none() && right.is_none() {
    // Local unit-test builds produce no archive.
    return;
  }
  match (left, right) {
    (Some(left), Some(right)) => {
      let left_keys = left.entries.keys().cloned().collect::<BTreeSet<_>>();
      let right_keys = right.entries.keys().cloned().collect::<BTreeSet<_>>();
      let left_only = left_keys
        .difference(&right_keys)
        .cloned()
        .collect::<Vec<_>>();
      let right_only = right_keys
        .difference(&left_keys)
        .cloned()
        .collect::<Vec<_>>();
      let mut changed = Vec::new();
      for name in left_keys.intersection(&right_keys) {
        let left_entry = &left.entries[name];
        let right_entry = &right.entries[name];
        // ECDSA's DER encoding has a run-dependent byte length. Signed
        // HAP/HSP members of an APP therefore change raw size even across two
        // Hvigor runs. Their recursively normalized content hash covers every
        // ZIP member and storage property; `verify_signed_build` independently
        // validates both outer and embedded signing blocks.
        let signed_package_with_equal_content = matches!(
          Path::new(name).extension().and_then(OsStr::to_str),
          Some("hap" | "hsp")
        ) && left_entry.content_hash
          == right_entry.content_hash;
        if (!signed_package_with_equal_content
          && (left_entry.size != right_entry.size
            || left_entry.compressed_size != right_entry.compressed_size))
          || left_entry.content_hash != right_entry.content_hash
        {
          changed.push(json!({
              "name": name,
              "left": {
                  "size": left_entry.size,
                  "compressed_size": left_entry.compressed_size,
                  "content_hash": left_entry.content_hash,
              },
              "right": {
                  "size": right_entry.size,
                  "compressed_size": right_entry.compressed_size,
                  "content_hash": right_entry.content_hash,
              },
          }));
        }
      }

      if !left_only.is_empty() || !right_only.is_empty() || !changed.is_empty() {
        diffs.push(json!({
            "scope": "archive_entries",
            "equal": false,
            "left_only": left_only,
            "right_only": right_only,
            "changed": changed,
        }));
      }
    }
    (None, Some(_)) => diffs.push(json!({
        "scope": "archive_entries",
        "equal": false,
        "left_missing": true,
    })),
    (Some(_), None) => diffs.push(json!({
        "scope": "archive_entries",
        "equal": false,
        "right_missing": true,
    })),
    (None, None) => diffs.push(json!({
        "scope": "archive_entries",
        "equal": false,
        "left_missing": true,
        "right_missing": true,
    })),
  }
}

fn compare_artifact_maps(
  diffs: &mut Vec<Value>,
  scope: &str,
  left: &BTreeMap<String, ArtifactSnapshot>,
  right: &BTreeMap<String, ArtifactSnapshot>,
) {
  let left_keys = left.keys().cloned().collect::<BTreeSet<_>>();
  let right_keys = right.keys().cloned().collect::<BTreeSet<_>>();

  for name in left_keys.difference(&right_keys) {
    diffs.push(json!({
        "scope": scope,
        "artifact": name,
        "equal": false,
        "right_missing": true,
        "left": left[name].to_json(),
    }));
  }
  for name in right_keys.difference(&left_keys) {
    diffs.push(json!({
        "scope": scope,
        "artifact": name,
        "equal": false,
        "left_missing": true,
        "right": right[name].to_json(),
    }));
  }
  for name in left_keys.intersection(&right_keys) {
    let artifact_diff = compare_artifact(name, &left[name], &right[name]);
    if !artifact_diff["equal"].as_bool().unwrap_or(false) {
      let mut diff = artifact_diff;
      diff["scope"] = json!(scope);
      diffs.push(diff);
    }
  }
}

fn compare_artifact(name: &str, left: &ArtifactSnapshot, right: &ArtifactSnapshot) -> Value {
  match (&left.kind, &right.kind) {
    (ArtifactKind::Json(left_json), ArtifactKind::Json(right_json)) => {
      let mut changes = Vec::new();
      diff_json("", left_json, right_json, &mut changes);
      json!({
          "artifact": name,
          "equal": changes.is_empty(),
          "kind": "json",
          "left": left.to_json(),
          "right": right.to_json(),
          "json_change_count": changes.len(),
          "json_changes": changes.into_iter().take(200).collect::<Vec<_>>(),
      })
    }
    _ => {
      let left_bytes = left.normalized_bytes();
      let right_bytes = right.normalized_bytes();
      let equal = left_bytes == right_bytes;
      json!({
          "artifact": name,
          "equal": equal,
          "kind": format!("{}:{}", left.kind_name(), right.kind_name()),
          "left": left.to_json(),
          "right": right.to_json(),
          "first_mismatch": if equal {
              Value::Null
          } else {
              first_mismatch_json(&left_bytes, &right_bytes)
          },
      })
    }
  }
}

fn diff_json(path: &str, left: &Value, right: &Value, changes: &mut Vec<Value>) {
  if left == right {
    return;
  }

  match (left, right) {
    (Value::Object(left_obj), Value::Object(right_obj)) => {
      let left_keys = left_obj.keys().cloned().collect::<BTreeSet<_>>();
      let right_keys = right_obj.keys().cloned().collect::<BTreeSet<_>>();
      for key in left_keys.difference(&right_keys) {
        changes.push(json!({
            "path": json_pointer(path, key),
            "left": left_obj[key],
            "right_missing": true,
        }));
      }
      for key in right_keys.difference(&left_keys) {
        changes.push(json!({
            "path": json_pointer(path, key),
            "left_missing": true,
            "right": right_obj[key],
        }));
      }
      for key in left_keys.intersection(&right_keys) {
        diff_json(
          &json_pointer(path, key),
          &left_obj[key],
          &right_obj[key],
          changes,
        );
      }
    }
    (Value::Array(left_arr), Value::Array(right_arr)) => {
      let common = left_arr.len().min(right_arr.len());
      for index in 0..common {
        diff_json(
          &format!("{path}/{index}"),
          &left_arr[index],
          &right_arr[index],
          changes,
        );
      }
      for (index, value) in left_arr.iter().enumerate().skip(common) {
        changes.push(json!({
            "path": format!("{path}/{index}"),
            "left": value,
            "right_missing": true,
        }));
      }
      for (index, value) in right_arr.iter().enumerate().skip(common) {
        changes.push(json!({
            "path": format!("{path}/{index}"),
            "left_missing": true,
            "right": value,
        }));
      }
    }
    _ => changes.push(json!({
        "path": if path.is_empty() { "/" } else { path },
        "left": left,
        "right": right,
    })),
  }
}

fn json_pointer(parent: &str, key: &str) -> String {
  format!("{}/{}", parent, key.replace('~', "~0").replace('/', "~1"))
}

fn first_mismatch_json(left: &[u8], right: &[u8]) -> Value {
  let offset = left
    .iter()
    .zip(right.iter())
    .position(|(l, r)| l != r)
    .unwrap_or_else(|| left.len().min(right.len()));
  json!({
      "offset": offset,
      "left_byte": left.get(offset).copied(),
      "right_byte": right.get(offset).copied(),
  })
}

fn rel_source(root: &Path, path: &Path) -> String {
  path
    .strip_prefix(root)
    .map(|rel| format!("<PROJECT>/{}", rel.to_string_lossy()))
    .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn stable_hash_hex(bytes: &[u8]) -> String {
  let mut hash = 0xcbf2_9ce4_8422_2325u64;
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
  }
  format!("{hash:016x}")
}

/// Collect ALL files from the CompileArkTS cache root directory.
///
/// Section 3.5: "扩展 oracle 收集 cache root 所有文件" — the oracle should
/// compare every file in the cache root, not just the named artifacts.
///
/// In hvigor's CompileArkTS stage, the cache root contains:
/// - `filesInfo.txt` — module list fed to es2abc
/// - `modules.cache` — es2abc ProgramCache entry map
/// - `npmEntries.txt` — npm package entries
/// - `sourceMaps.json` / `sourceMaps.map` — source map outputs
/// - `*.ts` / `*.ets` — compiled TypeScript handoff files
/// - `*.js` — bundled JavaScript output files
/// - `*.protoBin` — per-module ProgramCache binary files
///
/// This function discovers all of these and adds them to `artifacts` under
/// their project-relative path, enabling full cache-root content comparison.
fn collect_all_cache_root_files(
  artifacts: &mut BTreeMap<String, ArtifactSnapshot>,
  root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
  let cache_parent = root
    .join("build")
    .join("default")
    .join("cache")
    .join("default");
  // Latest hvigor uses `default@CompileArkTS` for HAP/HSP and
  // `default@HarCompileArkTS` for HAR. Discover task-scoped roots instead of
  // hardcoding a single module type; this also covers configured targets.
  let mut cache_roots = std::fs::read_dir(&cache_parent)
    .ok()
    .into_iter()
    .flatten()
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| {
      path.is_dir()
        && path
          .file_name()
          .and_then(OsStr::to_str)
          .is_some_and(|name| name.ends_with("CompileArkTS"))
    })
    .collect::<Vec<_>>();
  cache_roots.sort();
  // Old IDEs used the parent directly. Fall back only when no compiler task
  // root exists so files are not collected twice under different keys.
  if cache_roots.is_empty() && cache_parent.is_dir() {
    cache_roots.push(cache_parent);
  }

  for cache_root in &cache_roots {
    let entries = WalkDir::new(cache_root)
      .into_iter()
      .filter_map(Result::ok)
      .filter(|e| e.file_type().is_file());

    for entry in entries {
      let path = entry.path();
      let rel = path
        .strip_prefix(root)
        .map(|r| format!("cache:{}", r.display()))
        .unwrap_or_else(|_| format!("cache:{}", path.display()));

      // Skip extremely large files (e.g. final .abc files) — we only
      // want the intermediate/handoff files.
      let metadata = std::fs::metadata(path);
      if let Ok(m) = metadata {
        if m.len() > 10 * 1024 * 1024 {
          continue; // skip files > 10 MiB
        }
      }

      let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
      // Binary files compared by content hash; text files by content.
      let is_binary = matches!(ext, "abc" | "protoBin" | "bin" | "map");

      if let Ok(bytes) = fs::read(path) {
        artifacts.insert(
          rel.clone(),
          ArtifactSnapshot::from_bytes_with_normalization(
            rel,
            bytes,
            root,
            is_binary,
            ArtifactNormalization::for_path(path),
          ),
        );
      }
    }
  }
  Ok(())
}

// ---------------------------------------------------------------------------
// Multi-target artifact naming tests (section 3.1)
//
// These tests verify that the multi-artifact loop in build.rs produces the
// correct target-specific output filenames when multiple targets apply to a
// module.  They use `enumerate_applicable_targets_for_module` directly to
// avoid requiring a full build.
// ---------------------------------------------------------------------------

/// Verify that when multiple targets apply to a module, the artifact filenames
/// include the target name as a disambiguator.
///
/// hvigor naming convention (section 3.1):
///   {module}-{target}-unsigned.{ext}   e.g. entry-phone-unsigned.hap
///   {module}-{target}-signed.{ext}     e.g. entry-phone-signed.hap
///
/// This test creates a temporary build-profile.json5 with two targets ("phone"
/// and "tablet") both applicable to the "default" product, then verifies that
/// `enumerate_applicable_targets_for_module` returns both targets and that the
/// expected artifact filenames can be derived from them.
#[test]
fn test_multi_target_artifact_filenames_are_target_specific() {
  let tmp = tempfile::tempdir().expect("tempdir");
  let profile_path = tmp.path().join("build-profile.json5");

  // Write a build-profile.json5 with two targets applicable to "default" product.
  let profile_content = r#"{
  app: {
    signingConfigs: [],
    products: [
      {
        name: "default",
        compatibleSdkVersion: "5.0.5(17)",
        runtimeOS: "HarmonyOS",
        buildOption: {
          strictMode: { caseSensitiveCheck: false, useNormalizedOHMUrl: false }
        }
      }
    ],
    buildModeSet: [{ name: "debug" }, { name: "release" }]
  },
  modules: [
    {
      name: "entry",
      srcPath: "./entry",
      targets: [
        {
          name: "phone",
          applyToProducts: ["default"]
        },
        {
          name: "tablet",
          applyToProducts: ["default"]
        }
      ]
    }
  ]
}"#;
  std::fs::write(&profile_path, profile_content).expect("write build-profile.json5");

  // Enumerate applicable targets for the "entry" module.
  let targets = arkdown_config::build_profile::enumerate_applicable_targets_for_module(
    tmp.path(),
    "entry",
    "default",
    "debug",
  );

  // Both "phone" and "tablet" should be returned.
  assert_eq!(
    targets.len(),
    2,
    "both phone and tablet targets must be applicable to the default product; got: {:?}",
    targets.iter().map(|t| &t.name).collect::<Vec<_>>(),
  );
  assert_eq!(targets[0].name, "phone");
  assert_eq!(targets[1].name, "tablet");

  // Verify that target-specific artifact filenames are correct.
  let module_name = "entry";
  let ext = "hap";
  for target in &targets {
    let unsigned = format!("{}-{}-unsigned.{}", module_name, target.name, ext);
    let signed = format!("{}-{}-signed.{}", module_name, target.name, ext);
    assert!(
      unsigned.contains(&target.name),
      "unsigned artifact must contain target name '{}': {}",
      target.name,
      unsigned
    );
    assert!(
      signed.contains(&target.name),
      "signed artifact must contain target name '{}': {}",
      target.name,
      signed
    );
    // Verify the exact expected filenames.
    match target.name.as_str() {
      "phone" => {
        assert_eq!(unsigned, "entry-phone-unsigned.hap");
        assert_eq!(signed, "entry-phone-signed.hap");
      }
      "tablet" => {
        assert_eq!(unsigned, "entry-tablet-unsigned.hap");
        assert_eq!(signed, "entry-tablet-signed.hap");
      }
      other => panic!("unexpected target name: {}", other),
    }
  }
}

/// Verify that when only one target applies, the artifact uses "default" label
/// (backward compatibility with single-target projects).
#[test]
fn test_single_target_uses_default_label() {
  let tmp = tempfile::tempdir().expect("tempdir");
  let profile_path = tmp.path().join("build-profile.json5");

  let profile_content = r#"{
  app: {
    signingConfigs: [],
    products: [
      {
        name: "default",
        compatibleSdkVersion: "5.0.5(17)",
        runtimeOS: "HarmonyOS",
        buildOption: {
          strictMode: { caseSensitiveCheck: false, useNormalizedOHMUrl: false }
        }
      }
    ],
    buildModeSet: [{ name: "debug" }, { name: "release" }]
  },
  modules: [
    {
      name: "entry",
      srcPath: "./entry",
      targets: [
        {
          name: "default",
          applyToProducts: ["default"]
        }
      ]
    }
  ]
}"#;
  std::fs::write(&profile_path, profile_content).expect("write build-profile.json5");

  let targets = arkdown_config::build_profile::enumerate_applicable_targets_for_module(
    tmp.path(),
    "entry",
    "default",
    "debug",
  );

  assert_eq!(
    targets.len(),
    1,
    "single target must return exactly one result"
  );
  assert_eq!(targets[0].name, "default");

  // Single-target artifact uses "default" label.
  let unsigned = format!("entry-{}-unsigned.hap", targets[0].name);
  assert_eq!(unsigned, "entry-default-unsigned.hap");
}
