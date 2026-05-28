//! ZIP manipulation for HAP signing: parse, sort, align, insert signing block.
//!
//! Manual ZIP parsing — does not use the `zip` crate, since we need precise
//! control over the local-header extra field for alignment padding.

use crate::error::SignError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LOCAL_FILE_HEADER_SIG: u32 = 0x04034b50;
const CENTRAL_DIR_SIG: u32 = 0x02014b50;
const EOCD_SIG: u32 = 0x06054b50;
const LOCAL_HEADER_FIXED: usize = 30; // bytes before filename

/// Alignment for runnable files (.abc, .so): 4096 bytes.
pub const ALIGNMENT_RUNNABLE: usize = 4096;
/// Alignment for normal files: 4 bytes.
pub const ALIGNMENT_NORMAL: usize = 4;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A single entry parsed from a ZIP file.
#[derive(Debug, Clone)]
pub struct ZipEntry {
    /// Offset of this entry's local file header in the original file (used for CD parsing).
    pub original_offset: u32,
    /// Fixed part of the local header (30 bytes), without filename/extra.
    pub local_header_fixed: [u8; LOCAL_HEADER_FIXED],
    /// The filename bytes as stored in the local header.
    pub filename: Vec<u8>,
    /// Extra field bytes from the local header.
    pub extra: Vec<u8>,
    /// Compressed (or stored) file data.
    pub data: Vec<u8>,
    /// Human-readable name.
    pub name: String,
    /// Compression method (0 = stored, 8 = deflate).
    pub method: u16,
    /// True if the file is runnable (.abc or .so extension).
    pub is_runnable: bool,
    /// Raw central directory entry bytes (verbatim from the source file).
    pub cd_entry: Vec<u8>,
}

/// Parsed End of Central Directory record.
#[derive(Debug, Clone)]
pub struct EocdRecord {
    /// Total entries in the central directory.
    pub total_entries: u16,
    /// Size of the central directory (bytes).
    pub cd_size: u32,
    /// Offset of the central directory from start of disk.
    pub cd_offset: u32,
    /// ZIP file comment bytes.
    pub comment: Vec<u8>,
}

/// A parsed (and optionally aligned) HAP ZIP file.
pub struct HapZip {
    pub entries: Vec<ZipEntry>,
    pub eocd: EocdRecord,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Read a little-endian u16 from `buf` at `offset`.
#[inline]
fn read_u16(buf: &[u8], offset: usize) -> Result<u16, SignError> {
    buf.get(offset..offset + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(SignError::InvalidZip("unexpected EOF reading u16"))
}

/// Read a little-endian u32 from `buf` at `offset`.
#[inline]
fn read_u32(buf: &[u8], offset: usize) -> Result<u32, SignError> {
    buf.get(offset..offset + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(SignError::InvalidZip("unexpected EOF reading u32"))
}

impl HapZip {
    /// Parse an unsigned HAP (raw bytes).
    pub fn parse(bytes: &[u8]) -> Result<Self, SignError> {
        let eocd = find_eocd(bytes)?;

        // Parse central directory to get the list of entries + their offsets.
        let cd_start = eocd.cd_offset as usize;
        let cd_end = cd_start + eocd.cd_size as usize;
        if cd_end > bytes.len() {
            return Err(SignError::InvalidZip(
                "central directory extends beyond file",
            ));
        }

        let mut entries = Vec::with_capacity(eocd.total_entries as usize);
        let mut pos = cd_start;

        for _ in 0..eocd.total_entries {
            if pos + 46 > bytes.len() {
                return Err(SignError::InvalidZip("central directory entry too short"));
            }
            let sig = read_u32(bytes, pos)?;
            if sig != CENTRAL_DIR_SIG {
                return Err(SignError::InvalidZip(
                    "central directory signature mismatch",
                ));
            }

            let method = read_u16(bytes, pos + 10)?;
            let fn_len = read_u16(bytes, pos + 28)? as usize;
            let ef_len = read_u16(bytes, pos + 30)? as usize;
            let cm_len = read_u16(bytes, pos + 32)? as usize;
            let local_offset = read_u32(bytes, pos + 42)?;

            let cd_entry_len = 46 + fn_len + ef_len + cm_len;
            if pos + cd_entry_len > bytes.len() {
                return Err(SignError::InvalidZip(
                    "central directory entry data out of bounds",
                ));
            }

            let cd_entry = bytes[pos..pos + cd_entry_len].to_vec();
            let name_bytes = bytes[pos + 46..pos + 46 + fn_len].to_vec();
            let name = String::from_utf8_lossy(&name_bytes).into_owned();

            pos += cd_entry_len;

            // Now parse the local file header at local_offset.
            let lh_pos = local_offset as usize;
            if lh_pos + LOCAL_HEADER_FIXED > bytes.len() {
                return Err(SignError::InvalidZip("local header offset out of bounds"));
            }
            let lh_sig = read_u32(bytes, lh_pos)?;
            if lh_sig != LOCAL_FILE_HEADER_SIG {
                return Err(SignError::InvalidZip(
                    "local file header signature mismatch",
                ));
            }

            let lh_fn_len = read_u16(bytes, lh_pos + 26)? as usize;
            let lh_ef_len = read_u16(bytes, lh_pos + 28)? as usize;

            let mut local_header_fixed = [0u8; LOCAL_HEADER_FIXED];
            local_header_fixed.copy_from_slice(&bytes[lh_pos..lh_pos + LOCAL_HEADER_FIXED]);

            let filename_bytes = bytes
                [lh_pos + LOCAL_HEADER_FIXED..lh_pos + LOCAL_HEADER_FIXED + lh_fn_len]
                .to_vec();
            let extra_bytes = bytes[lh_pos + LOCAL_HEADER_FIXED + lh_fn_len
                ..lh_pos + LOCAL_HEADER_FIXED + lh_fn_len + lh_ef_len]
                .to_vec();

            let data_start = lh_pos + LOCAL_HEADER_FIXED + lh_fn_len + lh_ef_len;

            // Compressed size comes from CD (more reliable, handles data descriptor).
            let compressed_size = read_u32(bytes, pos - cd_entry_len + 46 - 4 + 4 + 4)?;
            // Actually read from local header: bytes 18..22
            let local_comp_size = read_u32(bytes, lh_pos + 18)? as usize;
            // Use whichever is non-zero
            let actual_comp_size = if local_comp_size > 0 {
                local_comp_size
            } else {
                compressed_size as usize
            };

            // Recalculate from cd entry: bytes 20..24 of cd entry = compressed size
            let cd_comp_size = read_u32(&cd_entry, 20)? as usize;
            let data_size = if cd_comp_size > 0 {
                cd_comp_size
            } else {
                actual_comp_size
            };

            if data_start + data_size > bytes.len() {
                return Err(SignError::InvalidZip("file data extends beyond file"));
            }

            let data = bytes[data_start..data_start + data_size].to_vec();

            let is_runnable = name.ends_with(".abc") || name.ends_with(".so");

            entries.push(ZipEntry {
                original_offset: local_offset,
                local_header_fixed,
                filename: filename_bytes,
                extra: extra_bytes,
                data,
                name,
                method,
                is_runnable,
                cd_entry,
            });
        }

        Ok(HapZip { entries, eocd })
    }

    /// Sort entries: runnable uncompressed (.abc, .so) first, then normal, then compressed.
    pub fn sort_entries(&mut self) {
        self.entries.sort_by_key(|e| {
            if e.is_runnable && e.method == 0 {
                0u8
            } else if e.method == 0 {
                1u8
            } else {
                2u8
            }
        });
    }

    /// Align entries by padding the local header's extra field.
    ///
    /// Alignment is achieved so that the file data of each entry starts at the
    /// required alignment boundary from the beginning of the output stream.
    pub fn align(&mut self) {
        // We need to calculate the running offset to determine alignment padding.
        // First pass: compute current total sizes without any modification.
        let mut current_offset: usize = 0;

        for entry in &mut self.entries {
            let alignment = if entry.is_runnable && entry.method == 0 {
                ALIGNMENT_RUNNABLE
            } else {
                ALIGNMENT_NORMAL
            };

            let fn_len = entry.filename.len();
            let header_before_data = LOCAL_HEADER_FIXED + fn_len + entry.extra.len();
            let data_start = current_offset + header_before_data;

            let remainder = data_start % alignment;
            let needed_padding = if remainder == 0 {
                0
            } else {
                alignment - remainder
            };

            if needed_padding > 0 {
                // Pad the extra field with zero bytes.
                entry
                    .extra
                    .extend(std::iter::repeat(0u8).take(needed_padding));
                // Update ef_len in the fixed header (bytes 28..30 LE).
                let new_ef_len = entry.extra.len() as u16;
                let ef_len_bytes = new_ef_len.to_le_bytes();
                entry.local_header_fixed[28] = ef_len_bytes[0];
                entry.local_header_fixed[29] = ef_len_bytes[1];
            }

            let total_entry_size =
                LOCAL_HEADER_FIXED + entry.filename.len() + entry.extra.len() + entry.data.len();
            current_offset += total_entry_size;
        }
    }

    /// Returns the three byte sections needed for the content digest:
    /// - entries section (all local headers + data)
    /// - central directory section (with updated offsets)
    /// - EOCD section (with CD offset zeroed)
    pub fn sections_for_signing(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let (entries_bytes, cd_bytes, _cd_offset) = self.build_entries_and_cd(0);

        // EOCD with CD offset zeroed
        let eocd_bytes = self.build_eocd(0, &cd_bytes);
        let mut eocd_zeroed = eocd_bytes.clone();
        // Bytes 16..20 of EOCD are the CD offset field
        if eocd_zeroed.len() >= 20 {
            eocd_zeroed[16] = 0;
            eocd_zeroed[17] = 0;
            eocd_zeroed[18] = 0;
            eocd_zeroed[19] = 0;
        }

        (entries_bytes, cd_bytes, eocd_zeroed)
    }

    /// Insert signing block between entries and CD, return final signed HAP bytes.
    pub fn with_signing_block(&self, signing_block: &[u8]) -> Vec<u8> {
        let (entries_bytes, cd_bytes, _) = self.build_entries_and_cd(signing_block.len());
        let cd_offset = entries_bytes.len() + signing_block.len();
        let eocd_bytes = self.build_eocd(cd_offset as u32, &cd_bytes);

        let mut result = Vec::with_capacity(
            entries_bytes.len() + signing_block.len() + cd_bytes.len() + eocd_bytes.len(),
        );
        result.extend_from_slice(&entries_bytes);
        result.extend_from_slice(signing_block);
        result.extend_from_slice(&cd_bytes);
        result.extend_from_slice(&eocd_bytes);
        result
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Build the serialized entries and (updated) central directory.
    /// `signing_block_len` is added to the CD offset base (for final assembly).
    fn build_entries_and_cd(&self, signing_block_len: usize) -> (Vec<u8>, Vec<u8>, usize) {
        let mut entries_bytes = Vec::new();
        let mut cd_bytes = Vec::new();
        let mut entry_offsets = Vec::with_capacity(self.entries.len());

        // Serialize all entries and track their actual offsets.
        for entry in &self.entries {
            let this_offset = entries_bytes.len();
            entry_offsets.push(this_offset);

            // Local file header
            entries_bytes.extend_from_slice(&entry.local_header_fixed);
            entries_bytes.extend_from_slice(&entry.filename);
            entries_bytes.extend_from_slice(&entry.extra);
            // File data
            entries_bytes.extend_from_slice(&entry.data);
        }

        // Build updated central directory entries.
        let cd_base = entries_bytes.len() + signing_block_len;
        for (i, entry) in self.entries.iter().enumerate() {
            let local_offset = entry_offsets[i] as u32;
            let mut cd = entry.cd_entry.clone();
            // Update local header offset (bytes 42..46 of CD entry)
            if cd.len() >= 46 {
                let offset_bytes = local_offset.to_le_bytes();
                cd[42] = offset_bytes[0];
                cd[43] = offset_bytes[1];
                cd[44] = offset_bytes[2];
                cd[45] = offset_bytes[3];
            }
            cd_bytes.extend_from_slice(&cd);
        }

        (entries_bytes, cd_bytes, cd_base)
    }

    /// Build the EOCD record with the given CD offset.
    fn build_eocd(&self, cd_offset: u32, cd_bytes: &[u8]) -> Vec<u8> {
        let cd_size = cd_bytes.len() as u32;
        let total_entries = self.entries.len() as u16;

        let mut eocd = Vec::with_capacity(22 + self.eocd.comment.len());
        // Signature
        eocd.extend_from_slice(&EOCD_SIG.to_le_bytes());
        // Disk number
        eocd.extend_from_slice(&0u16.to_le_bytes());
        // Disk with start of CD
        eocd.extend_from_slice(&0u16.to_le_bytes());
        // Entries on this disk
        eocd.extend_from_slice(&total_entries.to_le_bytes());
        // Total entries
        eocd.extend_from_slice(&total_entries.to_le_bytes());
        // CD size
        eocd.extend_from_slice(&cd_size.to_le_bytes());
        // CD offset
        eocd.extend_from_slice(&cd_offset.to_le_bytes());
        // Comment length
        eocd.extend_from_slice(&(self.eocd.comment.len() as u16).to_le_bytes());
        // Comment
        eocd.extend_from_slice(&self.eocd.comment);
        eocd
    }
}

// ---------------------------------------------------------------------------
// EOCD finding
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Alignment constants ──────────────────────────────────────────────

    #[test]
    fn test_alignment_runnable_is_4096() {
        assert_eq!(ALIGNMENT_RUNNABLE, 4096);
    }

    #[test]
    fn test_alignment_normal_is_4() {
        assert_eq!(ALIGNMENT_NORMAL, 4);
    }

    // ── Helper: build a minimal single-entry stored ZIP in memory ────────

    /// Build a minimal ZIP with a single stored (uncompressed) file.
    ///
    /// Returns raw ZIP bytes that `HapZip::parse` should accept.
    fn make_zip_bytes(filename: &[u8], data: &[u8]) -> Vec<u8> {
        let fn_len = filename.len() as u16;
        let data_len = data.len() as u32;

        let mut zip: Vec<u8> = Vec::new();

        // ── Local file header ──
        zip.extend_from_slice(&0x04034b50u32.to_le_bytes()); // signature
        zip.extend_from_slice(&20u16.to_le_bytes()); // version needed
        zip.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod time
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod date
        zip.extend_from_slice(&0u32.to_le_bytes()); // crc32
        zip.extend_from_slice(&data_len.to_le_bytes()); // compressed size
        zip.extend_from_slice(&data_len.to_le_bytes()); // uncompressed size
        zip.extend_from_slice(&fn_len.to_le_bytes()); // filename length
        zip.extend_from_slice(&0u16.to_le_bytes()); // extra length
        zip.extend_from_slice(filename); // filename
        zip.extend_from_slice(data); // file data

        let cd_offset = zip.len() as u32;

        // ── Central directory entry ──
        let cd_start = zip.len();
        zip.extend_from_slice(&0x02014b50u32.to_le_bytes()); // signature
        zip.extend_from_slice(&20u16.to_le_bytes()); // version made by
        zip.extend_from_slice(&20u16.to_le_bytes()); // version needed
        zip.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod time
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod date
        zip.extend_from_slice(&0u32.to_le_bytes()); // crc32
        zip.extend_from_slice(&data_len.to_le_bytes()); // compressed size
        zip.extend_from_slice(&data_len.to_le_bytes()); // uncompressed size
        zip.extend_from_slice(&fn_len.to_le_bytes()); // filename length
        zip.extend_from_slice(&0u16.to_le_bytes()); // extra length
        zip.extend_from_slice(&0u16.to_le_bytes()); // comment length
        zip.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        zip.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        zip.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        zip.extend_from_slice(&0u32.to_le_bytes()); // local header offset = 0
        zip.extend_from_slice(filename); // filename

        let cd_size = (zip.len() - cd_start) as u32;

        // ── End of central directory ──
        zip.extend_from_slice(&0x06054b50u32.to_le_bytes()); // signature
        zip.extend_from_slice(&0u16.to_le_bytes()); // disk number
        zip.extend_from_slice(&0u16.to_le_bytes()); // disk with start of CD
        zip.extend_from_slice(&1u16.to_le_bytes()); // entries on disk
        zip.extend_from_slice(&1u16.to_le_bytes()); // total entries
        zip.extend_from_slice(&cd_size.to_le_bytes()); // CD size
        zip.extend_from_slice(&cd_offset.to_le_bytes()); // CD offset
        zip.extend_from_slice(&0u16.to_le_bytes()); // comment length

        zip
    }

    // ── HapZip::parse ────────────────────────────────────────────────────

    #[test]
    fn test_parse_empty_slice_returns_error() {
        let result = HapZip::parse(&[]);
        assert!(result.is_err(), "parsing empty bytes must return Err");
    }

    #[test]
    fn test_parse_too_short_returns_error() {
        let result = HapZip::parse(&[0u8; 10]);
        assert!(result.is_err(), "parsing 10 bytes must return Err");
    }

    #[test]
    fn test_parse_random_bytes_returns_error() {
        let junk = b"this is not a zip file at all!!";
        let result = HapZip::parse(junk);
        assert!(result.is_err(), "parsing random bytes must return Err");
    }

    #[test]
    fn test_parse_minimal_zip_succeeds() {
        let bytes = make_zip_bytes(b"test.txt", b"hello");
        let result = HapZip::parse(&bytes);
        assert!(
            result.is_ok(),
            "valid minimal ZIP must parse successfully: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_returns_correct_entry_count() {
        let bytes = make_zip_bytes(b"data.json", b"{}");
        let hap = HapZip::parse(&bytes).expect("parse must succeed");
        assert_eq!(hap.entries.len(), 1, "should have exactly 1 entry");
    }

    #[test]
    fn test_parse_preserves_filename() {
        let bytes = make_zip_bytes(b"ets/modules.abc", b"PAND");
        let hap = HapZip::parse(&bytes).expect("parse must succeed");
        assert_eq!(hap.entries[0].name, "ets/modules.abc");
    }

    #[test]
    fn test_parse_recognises_abc_as_runnable() {
        let bytes = make_zip_bytes(b"ets/modules.abc", b"PAND\x00\x00");
        let hap = HapZip::parse(&bytes).expect("parse must succeed");
        assert!(
            hap.entries[0].is_runnable,
            ".abc file must be marked runnable"
        );
    }

    #[test]
    fn test_parse_recognises_so_as_runnable() {
        let bytes = make_zip_bytes(b"libs/libfoo.so", b"\x7fELF");
        let hap = HapZip::parse(&bytes).expect("parse must succeed");
        assert!(
            hap.entries[0].is_runnable,
            ".so file must be marked runnable"
        );
    }

    #[test]
    fn test_parse_json_file_not_runnable() {
        let bytes = make_zip_bytes(b"module.json", b"{}");
        let hap = HapZip::parse(&bytes).expect("parse must succeed");
        assert!(
            !hap.entries[0].is_runnable,
            ".json file must not be marked runnable"
        );
    }

    // ── sort_entries ────────────────────────────────────────────────────

    #[test]
    fn test_sort_puts_abc_before_json() {
        // Build a two-entry ZIP: json first, then abc. After sort, abc should be first.
        // We approximate by checking sort_by_key order directly on a constructed HapZip.

        let abc_bytes = make_zip_bytes(b"ets/modules.abc", b"PAND");
        let json_bytes = make_zip_bytes(b"module.json", b"{}");

        let mut hap_abc = HapZip::parse(&abc_bytes).expect("abc parse");
        let hap_json = HapZip::parse(&json_bytes).expect("json parse");

        // Manually combine entries: json first, abc second
        hap_abc.entries = vec![hap_json.entries[0].clone(), hap_abc.entries[0].clone()];
        hap_abc.sort_entries();

        assert!(
            hap_abc.entries[0].is_runnable,
            "after sort, first entry must be the runnable .abc file"
        );
        assert!(
            !hap_abc.entries[1].is_runnable,
            "after sort, second entry must be the non-runnable .json file"
        );
    }

    // ── align ────────────────────────────────────────────────────────────

    #[test]
    fn test_align_abc_pads_to_4096() {
        let bytes = make_zip_bytes(b"ets/modules.abc", b"PAND\x00\x00\x00\x00");
        let mut hap = HapZip::parse(&bytes).expect("parse must succeed");
        hap.align();

        // After alignment, data must start at a multiple of 4096.
        // Data offset = LOCAL_HEADER_FIXED(30) + filename.len() + extra.len()
        let e = &hap.entries[0];
        let data_offset = 30 + e.filename.len() + e.extra.len();
        assert_eq!(
            data_offset % 4096,
            0,
            "aligned .abc data offset ({data_offset}) must be a multiple of 4096"
        );
    }

    #[test]
    fn test_align_json_pads_to_4() {
        let bytes = make_zip_bytes(b"module.json", b"{}");
        let mut hap = HapZip::parse(&bytes).expect("parse must succeed");
        hap.align();

        let e = &hap.entries[0];
        let data_offset = 30 + e.filename.len() + e.extra.len();
        assert_eq!(
            data_offset % 4,
            0,
            "aligned .json data offset ({data_offset}) must be a multiple of 4"
        );
    }

    // ── sections_for_signing / with_signing_block ─────────────────────

    #[test]
    fn test_sections_for_signing_returns_nonempty_entries_section() {
        let bytes = make_zip_bytes(b"test.txt", b"payload");
        let mut hap = HapZip::parse(&bytes).expect("parse");
        hap.sort_entries();
        hap.align();
        let (entries_bytes, cd_bytes, eocd_zeroed) = hap.sections_for_signing();
        assert!(
            !entries_bytes.is_empty(),
            "entries section must not be empty"
        );
        assert!(!cd_bytes.is_empty(), "CD section must not be empty");
        assert!(!eocd_zeroed.is_empty(), "EOCD section must not be empty");
    }

    #[test]
    fn test_with_signing_block_increases_output_size() {
        let bytes = make_zip_bytes(b"test.txt", b"payload");
        let hap = HapZip::parse(&bytes).expect("parse");
        let signing_block = vec![0xFFu8; 128];
        let signed = hap.with_signing_block(&signing_block);
        assert!(
            signed.len() > bytes.len(),
            "signed output must be larger than the unsigned input"
        );
    }
}

/// Locate and parse the End of Central Directory record.
fn find_eocd(bytes: &[u8]) -> Result<EocdRecord, SignError> {
    // EOCD is at minimum 22 bytes, and comment can be up to 65535 bytes.
    // Scan backwards for the EOCD signature.
    if bytes.len() < 22 {
        return Err(SignError::InvalidZip("file too small to be a ZIP"));
    }

    let search_start = bytes.len().saturating_sub(22 + 65535);
    for i in (search_start..bytes.len() - 21).rev() {
        if bytes[i] == 0x50 && bytes[i + 1] == 0x4b && bytes[i + 2] == 0x05 && bytes[i + 3] == 0x06
        {
            // Found potential EOCD.
            let comment_len = u16::from_le_bytes([bytes[i + 20], bytes[i + 21]]) as usize;
            if i + 22 + comment_len == bytes.len() {
                let total_entries = u16::from_le_bytes([bytes[i + 8], bytes[i + 9]]);
                let cd_size = u32::from_le_bytes([
                    bytes[i + 12],
                    bytes[i + 13],
                    bytes[i + 14],
                    bytes[i + 15],
                ]);
                let cd_offset = u32::from_le_bytes([
                    bytes[i + 16],
                    bytes[i + 17],
                    bytes[i + 18],
                    bytes[i + 19],
                ]);
                let comment = bytes[i + 22..i + 22 + comment_len].to_vec();
                return Ok(EocdRecord {
                    total_entries,
                    cd_size,
                    cd_offset,
                    comment,
                });
            }
        }
    }
    Err(SignError::InvalidZip("EOCD record not found"))
}
