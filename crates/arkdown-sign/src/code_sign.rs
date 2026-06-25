use crate::profile_content::ProfileContent;
use crate::{pkcs7, signing_block, zip::HapZip, SignError, SigningKey};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

const PAGE_SIZE: usize = 4096;
const DIGEST_SIZE: usize = 32;
const FS_VERITY_HASH_ALGORITHM_SHA256: u8 = 1;
const FS_VERITY_LOG2_BLOCK_SIZE: u8 = 12;
const FS_VERITY_DESCRIPTOR_SIZE: usize = 256;
const FS_VERITY_DESCRIPTOR_FLAG_STORE_MERKLE_TREE_OFFSET: u32 = 0x1;
const FS_VERITY_DIGEST_MAGIC: &[u8; 8] = b"FSVerity";

const CODE_SIGN_BLOCK_HEADER_SIZE: usize = 32;
const CODE_SIGN_BLOCK_MAGIC: u64 = 0xE046_C8C6_5389_FCCD;
const CODE_SIGN_BLOCK_VERSION: u32 = 1;
const CODE_SIGN_SEGMENT_HEADER_SIZE: usize = 12;
const CODE_SIGN_SEGMENT_COUNT: usize = 3;
const CODE_SIGN_FLAG_MERKLE_TREE_INLINED: u32 = 0x1;
const CODE_SIGN_FLAG_NATIVE_LIB_INCLUDED: u32 = 0x2;

const SEGMENT_FS_VERITY_INFO: u32 = 0x1;
const SEGMENT_HAP_INFO: u32 = 0x2;
const SEGMENT_NATIVE_LIB_INFO: u32 = 0x3;

const FS_VERITY_INFO_MAGIC: u32 = (0x1E38 << 16) + 0x31AB;
const HAP_INFO_MAGIC: u32 = (0xC1B5 << 16) + 0xCC66;
const NATIVE_LIB_INFO_MAGIC: u32 = (0x0ED2 << 16) + 0xE720;

const SIGN_INFO_FIXED_SIZE: usize = 60;
const SIGN_INFO_FLAG_MERKLE_TREE_INCLUDED: u32 = 0x1;
const EXTENSION_MERKLE_TREE_INLINED: u32 = 0x1;
const EXTENSION_MERKLE_TREE_SIZE: u32 = 80;

pub(crate) struct CodeSignPropertyBuilder<'a> {
    signing_key: &'a SigningKey,
    alg_id: u32,
    profile: ProfileContent,
}

impl<'a> CodeSignPropertyBuilder<'a> {
    pub(crate) fn new(
        signing_key: &'a SigningKey,
        alg_id: u32,
        profile_bytes: &[u8],
    ) -> Result<Self, SignError> {
        Ok(Self {
            signing_key,
            alg_id,
            profile: ProfileContent::from_signed_profile(profile_bytes)?,
        })
    }

    pub(crate) fn build_property_block(
        &self,
        hap_zip: &HapZip,
        code_sign_offset: usize,
    ) -> Result<Vec<u8>, SignError> {
        let owner_id = self.profile.owner_id()?;
        let plugin_id = self.plugin_id_if_needed(hap_zip)?;
        let code_sign_block = CodeSignBlockBuilder::new(
            hap_zip,
            self.signing_key,
            self.alg_id,
            owner_id,
            plugin_id,
            code_sign_offset,
        )
        .build()?;

        let mut value = Vec::with_capacity(12 + code_sign_block.len());
        value.extend_from_slice(&signing_block::BLOCK_ID_CODE_SIGN.to_le_bytes());
        value.extend_from_slice(&(code_sign_block.len() as u32).to_le_bytes());
        value.extend_from_slice(&(code_sign_offset as u32).to_le_bytes());
        value.extend_from_slice(&code_sign_block);
        Ok(value)
    }

    fn plugin_id_if_needed(&self, hap_zip: &HapZip) -> Result<Option<String>, SignError> {
        let Ok(module_json) = hap_zip.read_uncompressed_entry("module.json") else {
            return Ok(None);
        };
        let module_json = String::from_utf8(module_json)
            .map_err(|e| SignError::Config(format!("module.json is not UTF-8: {e}")))?;
        let value: serde_json::Value = serde_json::from_str(&module_json)
            .map_err(|e| SignError::Config(format!("module.json is not valid JSON: {e}")))?;
        let bundle_type = value
            .get("app")
            .and_then(|app| app.get("bundleType"))
            .and_then(serde_json::Value::as_str);
        if bundle_type == Some("appPlugin") {
            Ok(Some(self.profile.plugin_id()?))
        } else {
            Ok(None)
        }
    }
}

struct CodeSignBlockBuilder<'a> {
    hap_zip: &'a HapZip,
    signing_key: &'a SigningKey,
    alg_id: u32,
    owner_id: Option<String>,
    plugin_id: Option<String>,
    code_sign_offset: usize,
}

impl<'a> CodeSignBlockBuilder<'a> {
    fn new(
        hap_zip: &'a HapZip,
        signing_key: &'a SigningKey,
        alg_id: u32,
        owner_id: Option<String>,
        plugin_id: Option<String>,
        code_sign_offset: usize,
    ) -> Self {
        Self {
            hap_zip,
            signing_key,
            alg_id,
            owner_id,
            plugin_id,
            code_sign_offset,
        }
    }

    fn build(&self) -> Result<Vec<u8>, SignError> {
        self.reject_hnp_entries()?;
        let signed_data_builder = pkcs7::CodeSignSignedDataBuilder::new(
            &self.signing_key.private_key_der,
            &self.signing_key.cert_chain,
            self.alg_id,
        )?;

        let zero_padding_len = self.merkle_tree_padding_len();
        let fsv_tree_offset = self.code_sign_offset
            + CODE_SIGN_BLOCK_HEADER_SIZE
            + CODE_SIGN_SEGMENT_COUNT * CODE_SIGN_SEGMENT_HEADER_SIZE
            + zero_padding_len;

        let data_size = self.hap_zip.code_sign_data_size()?;
        let hap_data = self.hap_zip.entry_section_prefix(data_size)?;
        let hap_fsverity = FsVerityGenerator::generate(&hap_data, fsv_tree_offset as u64)?;
        let hap_signature =
            self.sign_fsverity_digest(&signed_data_builder, &hap_fsverity.digest)?;
        let mut hap_sign_info = SignInfo::new(
            SIGN_INFO_FLAG_MERKLE_TREE_INCLUDED,
            data_size as u64,
            hap_signature,
        );
        hap_sign_info.add_extension(Extension::MerkleTree {
            size: hap_fsverity.tree.len() as u64,
            offset: fsv_tree_offset as u64,
            root_hash: hap_fsverity.root_hash,
        });

        let native_sign_infos = self.native_sign_infos(&signed_data_builder)?;
        let fs_verity_info_segment = FsVerityInfoSegment.to_bytes();
        let hap_info_segment = HapInfoSegment { hap_sign_info }.to_bytes();
        let native_lib_info_segment = NativeLibInfoSegment {
            entries: native_sign_infos,
        }
        .to_bytes();

        let segment_start = CODE_SIGN_BLOCK_HEADER_SIZE
            + CODE_SIGN_SEGMENT_COUNT * CODE_SIGN_SEGMENT_HEADER_SIZE
            + zero_padding_len
            + hap_fsverity.tree.len();
        let fs_verity_header = SegmentHeader {
            segment_type: SEGMENT_FS_VERITY_INFO,
            offset: segment_start,
            size: fs_verity_info_segment.len(),
        };
        let hap_header = SegmentHeader {
            segment_type: SEGMENT_HAP_INFO,
            offset: segment_start + fs_verity_info_segment.len(),
            size: hap_info_segment.len(),
        };
        let native_header = SegmentHeader {
            segment_type: SEGMENT_NATIVE_LIB_INFO,
            offset: segment_start + fs_verity_info_segment.len() + hap_info_segment.len(),
            size: native_lib_info_segment.len(),
        };

        let block_size = CODE_SIGN_BLOCK_HEADER_SIZE
            + CODE_SIGN_SEGMENT_COUNT * CODE_SIGN_SEGMENT_HEADER_SIZE
            + zero_padding_len
            + hap_fsverity.tree.len()
            + fs_verity_info_segment.len()
            + hap_info_segment.len()
            + native_lib_info_segment.len();
        let flags = CODE_SIGN_FLAG_MERKLE_TREE_INLINED
            | if native_header.size > 12 {
                CODE_SIGN_FLAG_NATIVE_LIB_INCLUDED
            } else {
                0
            };

        let mut output = Vec::with_capacity(block_size);
        CodeSignBlockHeader { block_size, flags }.write_to(&mut output);
        fs_verity_header.write_to(&mut output);
        hap_header.write_to(&mut output);
        native_header.write_to(&mut output);
        output.resize(output.len() + zero_padding_len, 0);
        output.extend_from_slice(&hap_fsverity.tree);
        output.extend_from_slice(&fs_verity_info_segment);
        output.extend_from_slice(&hap_info_segment);
        output.extend_from_slice(&native_lib_info_segment);
        Ok(output)
    }

    fn reject_hnp_entries(&self) -> Result<(), SignError> {
        if self
            .hap_zip
            .entries
            .iter()
            .any(|entry| entry.name.starts_with("hnp/") && entry.name.ends_with(".hnp"))
        {
            return Err(SignError::Config(
                "HNP code signing is not implemented in the Rust signer".to_string(),
            ));
        }
        Ok(())
    }

    fn merkle_tree_padding_len(&self) -> usize {
        let size_without_merkle =
            CODE_SIGN_BLOCK_HEADER_SIZE + CODE_SIGN_SEGMENT_COUNT * CODE_SIGN_SEGMENT_HEADER_SIZE;
        let residual = (self.code_sign_offset + size_without_merkle) % PAGE_SIZE;
        if residual == 0 {
            0
        } else {
            PAGE_SIZE - residual
        }
    }

    fn native_sign_infos(
        &self,
        signed_data_builder: &pkcs7::CodeSignSignedDataBuilder,
    ) -> Result<Vec<(String, SignInfo)>, SignError> {
        let mut entries = self
            .hap_zip
            .native_entry_names()
            .into_par_iter()
            .enumerate()
            .map(|(index, name)| {
                let data = self.hap_zip.read_uncompressed_entry(&name)?;
                let fsverity = FsVerityGenerator::generate(&data, 0)?;
                let signature = self.sign_fsverity_digest(signed_data_builder, &fsverity.digest)?;
                Ok::<_, SignError>((
                    index,
                    (name, SignInfo::new(0, data.len() as u64, signature)),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|(index, _)| *index);
        Ok(entries.into_iter().map(|(_, entry)| entry).collect())
    }

    fn sign_fsverity_digest(
        &self,
        signed_data_builder: &pkcs7::CodeSignSignedDataBuilder,
        digest: &[u8],
    ) -> Result<Vec<u8>, SignError> {
        signed_data_builder.build(digest, self.owner_id.as_deref(), self.plugin_id.as_deref())
    }
}

struct FsVerityGenerator;

struct FsVerityOutput {
    digest: Vec<u8>,
    tree: Vec<u8>,
    root_hash: [u8; DIGEST_SIZE],
}

impl FsVerityGenerator {
    fn generate(data: &[u8], merkle_tree_offset: u64) -> Result<FsVerityOutput, SignError> {
        let merkle = MerkleTree::from_data(data);
        let flags = if merkle_tree_offset == 0 {
            0
        } else {
            FS_VERITY_DESCRIPTOR_FLAG_STORE_MERKLE_TREE_OFFSET
        };
        let descriptor = FsVerityDescriptor {
            file_size: data.len() as u64,
            root_hash: merkle.root_hash,
            flags,
            merkle_tree_offset,
        }
        .digest_bytes();
        let digest = Sha256::digest(&descriptor);
        let mut fsverity_digest = Vec::with_capacity(12 + digest.len());
        fsverity_digest.extend_from_slice(FS_VERITY_DIGEST_MAGIC);
        fsverity_digest.extend_from_slice(&(FS_VERITY_HASH_ALGORITHM_SHA256 as u16).to_le_bytes());
        fsverity_digest.extend_from_slice(&(digest.len() as u16).to_le_bytes());
        fsverity_digest.extend_from_slice(&digest);
        Ok(FsVerityOutput {
            digest: fsverity_digest,
            tree: merkle.tree,
            root_hash: merkle.root_hash,
        })
    }
}

struct MerkleTree {
    tree: Vec<u8>,
    root_hash: [u8; DIGEST_SIZE],
}

impl MerkleTree {
    fn from_data(data: &[u8]) -> Self {
        if data.is_empty() {
            return Self {
                tree: Vec::new(),
                root_hash: [0; DIGEST_SIZE],
            };
        }

        let leaf = Self::hash_level(data);
        if data.len() <= PAGE_SIZE {
            let mut root_hash = [0u8; DIGEST_SIZE];
            root_hash.copy_from_slice(&leaf[..DIGEST_SIZE]);
            return Self {
                tree: Vec::new(),
                root_hash,
            };
        }

        let mut levels = vec![leaf];
        while levels.last().is_some_and(|level| level.len() > PAGE_SIZE) {
            let parent = Self::hash_level(levels.last().expect("level exists"));
            levels.push(parent);
        }
        levels.reverse();
        let tree = levels.concat();
        let root_hash: [u8; DIGEST_SIZE] = Sha256::digest(&tree[..PAGE_SIZE]).into();
        Self { tree, root_hash }
    }

    fn hash_level(data: &[u8]) -> Vec<u8> {
        let chunk_count = data.len().div_ceil(PAGE_SIZE);
        let digest_bytes_len = chunk_count * DIGEST_SIZE;
        let padded_len = digest_bytes_len.div_ceil(PAGE_SIZE) * PAGE_SIZE;
        let mut output = Vec::with_capacity(padded_len);
        for chunk_index in 0..chunk_count {
            let start = chunk_index * PAGE_SIZE;
            let end = (start + PAGE_SIZE).min(data.len());
            let mut page = [0u8; PAGE_SIZE];
            page[..end - start].copy_from_slice(&data[start..end]);
            output.extend_from_slice(&Sha256::digest(page));
        }
        output.resize(padded_len, 0);
        output
    }
}

struct FsVerityDescriptor {
    file_size: u64,
    root_hash: [u8; DIGEST_SIZE],
    flags: u32,
    merkle_tree_offset: u64,
}

impl FsVerityDescriptor {
    fn digest_bytes(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(FS_VERITY_DESCRIPTOR_SIZE);
        output.push(1);
        output.push(FS_VERITY_HASH_ALGORITHM_SHA256);
        output.push(FS_VERITY_LOG2_BLOCK_SIZE);
        output.push(0);
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&self.file_size.to_le_bytes());
        output.extend_from_slice(&self.root_hash);
        output.resize(output.len() + (64 - DIGEST_SIZE), 0);
        output.resize(output.len() + 32, 0);
        output.extend_from_slice(&self.flags.to_le_bytes());
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&self.merkle_tree_offset.to_le_bytes());
        output.resize(FS_VERITY_DESCRIPTOR_SIZE, 0);
        output
    }
}

struct SignInfo {
    flags: u32,
    data_size: u64,
    signature: Vec<u8>,
    extensions: Vec<Extension>,
}

impl SignInfo {
    fn new(flags: u32, data_size: u64, signature: Vec<u8>) -> Self {
        Self {
            flags,
            data_size,
            signature,
            extensions: Vec::new(),
        }
    }

    fn add_extension(&mut self, extension: Extension) {
        self.extensions.push(extension);
    }

    fn to_bytes(&self) -> Vec<u8> {
        let zero_padding_len = (4 - self.signature.len() % 4) % 4;
        let extension_offset = SIGN_INFO_FIXED_SIZE + self.signature.len() + zero_padding_len;
        let extensions_len = self.extensions.iter().map(Extension::size).sum::<usize>();
        let mut output = Vec::with_capacity(extension_offset + extensions_len);
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&(self.signature.len() as u32).to_le_bytes());
        output.extend_from_slice(&self.flags.to_le_bytes());
        output.extend_from_slice(&self.data_size.to_le_bytes());
        output.resize(output.len() + 32, 0);
        output.extend_from_slice(&(self.extensions.len() as u32).to_le_bytes());
        output.extend_from_slice(&(extension_offset as u32).to_le_bytes());
        output.extend_from_slice(&self.signature);
        output.resize(output.len() + zero_padding_len, 0);
        for extension in &self.extensions {
            extension.write_to(&mut output);
        }
        output
    }
}

enum Extension {
    MerkleTree {
        size: u64,
        offset: u64,
        root_hash: [u8; DIGEST_SIZE],
    },
}

impl Extension {
    fn size(&self) -> usize {
        match self {
            Self::MerkleTree { .. } => 8 + EXTENSION_MERKLE_TREE_SIZE as usize,
        }
    }

    fn write_to(&self, output: &mut Vec<u8>) {
        match self {
            Self::MerkleTree {
                size,
                offset,
                root_hash,
            } => {
                output.extend_from_slice(&EXTENSION_MERKLE_TREE_INLINED.to_le_bytes());
                output.extend_from_slice(&EXTENSION_MERKLE_TREE_SIZE.to_le_bytes());
                output.extend_from_slice(&size.to_le_bytes());
                output.extend_from_slice(&offset.to_le_bytes());
                output.extend_from_slice(root_hash);
                output.resize(output.len() + (64 - DIGEST_SIZE), 0);
            }
        }
    }
}

struct FsVerityInfoSegment;

impl FsVerityInfoSegment {
    fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(64);
        output.extend_from_slice(&FS_VERITY_INFO_MAGIC.to_le_bytes());
        output.push(1);
        output.push(FS_VERITY_HASH_ALGORITHM_SHA256);
        output.push(FS_VERITY_LOG2_BLOCK_SIZE);
        output.resize(64, 0);
        output
    }
}

struct HapInfoSegment {
    hap_sign_info: SignInfo,
}

impl HapInfoSegment {
    fn to_bytes(self) -> Vec<u8> {
        let sign_info = self.hap_sign_info.to_bytes();
        let mut output = Vec::with_capacity(4 + sign_info.len());
        output.extend_from_slice(&HAP_INFO_MAGIC.to_le_bytes());
        output.extend_from_slice(&sign_info);
        output
    }
}

struct NativeLibInfoSegment {
    entries: Vec<(String, SignInfo)>,
}

impl NativeLibInfoSegment {
    fn to_bytes(self) -> Vec<u8> {
        let names = self
            .entries
            .iter()
            .map(|(name, _)| name.as_bytes())
            .collect::<Vec<_>>();
        let sign_infos = self
            .entries
            .iter()
            .map(|(_, sign_info)| sign_info.to_bytes())
            .collect::<Vec<_>>();
        let names_len = names.iter().map(|name| name.len()).sum::<usize>();
        let sign_infos_len = sign_infos.iter().map(Vec::len).sum::<usize>();
        let zero_padding_len = (4 - names_len % 4) % 4;
        let pos_table_len = self.entries.len() * 16;
        let name_base = 12 + pos_table_len;
        let sign_info_base = name_base + names_len + zero_padding_len;
        let segment_size = 12 + pos_table_len + names_len + zero_padding_len + sign_infos_len;

        let mut output = Vec::with_capacity(segment_size);
        output.extend_from_slice(&NATIVE_LIB_INFO_MAGIC.to_le_bytes());
        output.extend_from_slice(&(segment_size as u32).to_le_bytes());
        output.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());

        let mut name_offset = 0usize;
        let mut sign_info_offset = 0usize;
        for (name, sign_info) in names.iter().zip(sign_infos.iter()) {
            output.extend_from_slice(&((name_base + name_offset) as u32).to_le_bytes());
            output.extend_from_slice(&(name.len() as u32).to_le_bytes());
            output.extend_from_slice(&((sign_info_base + sign_info_offset) as u32).to_le_bytes());
            output.extend_from_slice(&(sign_info.len() as u32).to_le_bytes());
            name_offset += name.len();
            sign_info_offset += sign_info.len();
        }

        for name in names {
            output.extend_from_slice(name);
        }
        output.resize(output.len() + zero_padding_len, 0);
        for sign_info in sign_infos {
            output.extend_from_slice(&sign_info);
        }
        output
    }
}

struct CodeSignBlockHeader {
    block_size: usize,
    flags: u32,
}

impl CodeSignBlockHeader {
    fn write_to(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&CODE_SIGN_BLOCK_MAGIC.to_le_bytes());
        output.extend_from_slice(&CODE_SIGN_BLOCK_VERSION.to_le_bytes());
        output.extend_from_slice(&(self.block_size as u32).to_le_bytes());
        output.extend_from_slice(&(CODE_SIGN_SEGMENT_COUNT as u32).to_le_bytes());
        output.extend_from_slice(&self.flags.to_le_bytes());
        output.resize(output.len() + 8, 0);
    }
}

struct SegmentHeader {
    segment_type: u32,
    offset: usize,
    size: usize,
}

impl SegmentHeader {
    fn write_to(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.segment_type.to_le_bytes());
        output.extend_from_slice(&(self.offset as u32).to_le_bytes());
        output.extend_from_slice(&(self.size as u32).to_le_bytes());
    }
}
