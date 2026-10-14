use std::io::Write;
use std::path::Path;

use aes_gcm::{aead::Aead, Aes128Gcm, KeyInit, Nonce};
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

    let fixture_keystore_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test.p12");
    let keystore = fs_err::read(&fixture_keystore_path).expect("read PKCS#12 fixture");
    let signing_key = SigningKey::from_pkcs12(&keystore, "test123", "1", "test123")
        .expect("load PKCS#12 fixture through published hapsigner");
    let keystore_path = temporary.path().join("test.p12");
    fs_err::write(&keystore_path, &keystore).expect("copy PKCS#12 beside DevEco material");
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

    let encrypted_password = write_deveco_password_material(temporary.path(), "test123");
    let config = SigningConfig {
        name: None,
        cert_path: certificate_path,
        key_alias: "1".into(),
        key_password: encrypted_password.clone(),
        profile_path,
        sign_alg: "SHA384withRSA/PSS".into(),
        store_file: keystore_path,
        store_password: encrypted_password,
    };
    let output = temporary.path().join("signed.hap");
    let runner =
        SignBackendRunner::new().with_cache_directory(Some(temporary.path().join("cache")));
    runner
        .sign_or_skip(&application_path, &output, Some(&config))
        .unwrap();
    let signed_before = fs_err::read(&output).unwrap();
    let marker = std::time::UNIX_EPOCH + std::time::Duration::from_secs(123456);
    fs_err::File::options()
        .write(true)
        .open(&output)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(marker))
        .unwrap();
    let saved_time = fs_err::metadata(&output).unwrap().modified().unwrap();
    runner
        .sign_or_skip(&application_path, &output, Some(&config))
        .unwrap();
    assert_eq!(
        fs_err::metadata(&output).unwrap().modified().unwrap(),
        saved_time
    );
    assert_eq!(
        fs_err::read(&output).unwrap(),
        signed_before,
        "unchanged signing inputs must preserve signature bytes"
    );
    fs_err::write(&output, b"corrupt signature").unwrap();
    runner
        .sign_or_skip(&application_path, &output, Some(&config))
        .unwrap();
    ApplicationVerifier::new(&fs_err::read(&output).unwrap())
        .verify(InputFormat::Zip)
        .unwrap();
    let original_profile = fs_err::read(&config.profile_path).unwrap();
    fs_err::write(&config.profile_path, b"invalid profile").unwrap();
    assert!(
        runner
            .sign_or_skip(&application_path, &output, Some(&config))
            .is_err(),
        "changed signing material must invalidate"
    );
    fs_err::write(&config.profile_path, original_profile).unwrap();
    runner
        .sign_or_skip(&application_path, &output, Some(&config))
        .unwrap();
    ApplicationVerifier::new(&fs_err::read(&output).unwrap())
        .verify(InputFormat::Zip)
        .unwrap();
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

fn write_deveco_password_material(root: &Path, password: &str) -> String {
    const COMPONENT: [u8; 16] = [
        49, 243, 9, 115, 214, 175, 91, 184, 211, 190, 177, 88, 101, 131, 192, 119,
    ];
    const PASSWORD_MATERIAL: [u8; 16] = *b"arkdown-sign-key";
    const SALT: [u8; 16] = *b"arkdown-test-slt";
    const WORK_KEY: [u8; 16] = [0x42; 16];

    let material = root.join("material");
    for directory in ["fd/0", "fd/1", "fd/2", "ac", "ce"] {
        fs_err::create_dir_all(material.join(directory)).expect("create DevEco material directory");
    }
    let mut fd0 = [0_u8; 16];
    for index in 0..fd0.len() {
        fd0[index] = PASSWORD_MATERIAL[index] ^ COMPONENT[index];
    }
    fs_err::write(material.join("fd/0/value"), fd0).expect("write first fd component");
    fs_err::write(material.join("fd/1/value"), [0_u8; 16]).expect("write second fd component");
    fs_err::write(material.join("fd/2/value"), [0_u8; 16]).expect("write third fd component");
    fs_err::write(material.join("ac/value"), SALT).expect("write PBKDF2 salt");

    let mut root_key = [0_u8; 16];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(&PASSWORD_MATERIAL, &SALT, 10_000, &mut root_key);
    fs_err::write(
        material.join("ce/value"),
        encrypt_deveco_material(&root_key, &[1_u8; 12], &WORK_KEY),
    )
    .expect("write encrypted work key");
    hex::encode(encrypt_deveco_material(
        &WORK_KEY,
        &[2_u8; 12],
        password.as_bytes(),
    ))
}

fn encrypt_deveco_material(key: &[u8; 16], nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes128Gcm::new_from_slice(key).expect("valid AES-128 key");
    let encrypted = cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .expect("encrypt DevEco test material");
    let encrypted_length = u32::try_from(encrypted.len()).expect("test material length fits u32");
    let mut output = Vec::with_capacity(4 + nonce.len() + encrypted.len());
    output.extend_from_slice(&encrypted_length.to_be_bytes());
    output.extend_from_slice(nonce);
    output.extend_from_slice(&encrypted);
    output
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
