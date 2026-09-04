use std::io::Write;
use std::path::Path;

use arkdown_core::SigningConfig;
use arkdown_sign::SignBackendRunner;
use hapsigner::{
    ApplicationVerifier, DevelopmentMaterialBuilder, DevelopmentProfileOptions, InputFormat,
    SigningAlgorithm, SigningKey,
};
use zip::write::SimpleFileOptions;

#[test]
fn build_profile_adapter_uses_published_sha384_backend_for_in_place_signing() {
    let temporary = tempfile::tempdir().expect("temporary signing fixture");
    let application_path = temporary.path().join("entry.hap");
    write_unsigned_application(&application_path);

    let keystore_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test.p12");
    let keystore = fs_err::read(&keystore_path).expect("read PKCS#12 fixture");
    let signing_key = SigningKey::from_pkcs12(&keystore, "test123", "1", "test123")
        .expect("load PKCS#12 fixture through published hapsigner");
    let certificate_path = temporary.path().join("application.cer");
    fs_err::write(&certificate_path, &signing_key.cert_chain[0])
        .expect("write application certificate");

    let profile = DevelopmentMaterialBuilder::new(DevelopmentProfileOptions {
        bundle_name: "com.example.arkdown.signing".into(),
        ..DevelopmentProfileOptions::default()
    })
    .build()
    .expect("create valid signed profile");
    let profile_path = temporary.path().join("profile.p7b");
    fs_err::write(&profile_path, profile.signed_profile()).expect("write signed profile");

    let config = SigningConfig {
        name: None,
        cert_path: certificate_path,
        key_alias: "1".into(),
        key_password: "test123".into(),
        profile_path,
        sign_alg: "SHA384withRSA/PSS".into(),
        store_file: keystore_path,
        store_password: "test123".into(),
    };
    SignBackendRunner::new()
        .sign_or_skip(&application_path, &application_path, Some(&config))
        .expect("sign in place through Arkdown adapter");

    let signed = fs_err::read(&application_path).expect("read signed application");
    let verification = ApplicationVerifier::new(&signed)
        .verify(InputFormat::Zip)
        .expect("verify signed application through published hapsigner");
    assert_eq!(verification.algorithm, Some(SigningAlgorithm::RsaPssSha384));
    assert_eq!(verification.signing_block_version, Some(3));
}

fn write_unsigned_application(path: &Path) {
    let file = fs_err::File::create(path).expect("create unsigned application");
    let mut archive = zip::ZipWriter::new(file);
    archive
        .start_file("module.json", SimpleFileOptions::default())
        .expect("start module profile entry");
    archive
        .write_all(br#"{"module":{"name":"entry","type":"entry"}}"#)
        .expect("write module profile entry");
    archive.finish().expect("finish unsigned application");
}
