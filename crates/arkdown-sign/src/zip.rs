//! ZIP manipulation for HAP signing: parse, sort, align, insert signing block.
//!
//! Manual ZIP parsing — does not use the `zip` crate, since we need precise
//! control over the local-header extra field for alignment padding.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use bytes::Bytes;

use crate::{digest, error::SignError};

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
    pub data: ZipEntryData,
    /// Human-readable name.
    pub name: String,
    /// Compression method (0 = stored, 8 = deflate).
    pub method: u16,
    /// True if the file is runnable (.abc or .so extension).
    pub is_runnable: bool,
    /// Raw central directory entry bytes (verbatim from the source file).
    pub cd_entry: Vec<u8>,
}

/// Entry payload storage. File-backed entries let the signer stream large HAPs
/// without retaining the full archive in memory.
#[derive(Debug, Clone)]
pub enum ZipEntryData {
    Memory(Bytes),
    FileRange { offset: u64, len: usize },
}

impl ZipEntryData {
    pub fn len(&self) -> usize {
        match self {
            Self::Memory(bytes) => bytes.len(),
            Self::FileRange { len, .. } => *len,
        }
    }

    fn memory_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Memory(bytes) => Some(bytes),
            Self::FileRange { .. } => None,
        }
    }
}

struct ZipEntryName<'a> {
    raw: &'a str,
}

impl<'a> ZipEntryName<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_runnable(&self) -> bool {
        self.has_extension("abc") || self.has_extension("so")
    }

    fn has_extension(&self, expected: &str) -> bool {
        Path::new(self.raw)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
    }
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
    source_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

impl HapZip {
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

    /// Parse an unsigned HAP (raw bytes).
    pub fn parse(bytes: &[u8]) -> Result<Self, SignError> {
        Self::parse_bytes(Bytes::copy_from_slice(bytes))
    }

    /// Parse an unsigned HAP from an owned byte buffer without copying entry
    /// payloads. Each entry stores a cheap slice into the original HAP bytes.
    pub fn parse_owned(bytes: Vec<u8>) -> Result<Self, SignError> {
        Self::parse_bytes(Bytes::from(bytes))
    }

    /// Parse an unsigned HAP file without reading the whole archive into
    /// memory. Entry payloads are stored as byte ranges and streamed later.
    pub fn parse_file(path: &Path) -> Result<Self, SignError> {
        let mut file = fs_err::File::open(path)?;
        let file_len = file.metadata()?.len();
        if file_len < 22 {
            return Err(SignError::InvalidZip("file too small to be a ZIP"));
        }

        let tail_len = usize::try_from(file_len.min((22 + 65535) as u64))
            .map_err(|_| SignError::InvalidZip("ZIP file is too large"))?;
        let tail_start = file_len - tail_len as u64;
        file.seek(SeekFrom::Start(tail_start))?;
        let mut tail = vec![0u8; tail_len];
        file.read_exact(&mut tail)?;
        let eocd = find_eocd(&tail)?;

        let cd_start = eocd.cd_offset as u64;
        let cd_size = eocd.cd_size as usize;
        if cd_start + cd_size as u64 > file_len {
            return Err(SignError::InvalidZip(
                "central directory extends beyond file",
            ));
        }
        file.seek(SeekFrom::Start(cd_start))?;
        let mut cd = vec![0u8; cd_size];
        file.read_exact(&mut cd)?;

        let entries = Self::parse_file_entries(&mut file, &cd, &eocd, file_len)?;
        Ok(HapZip {
            entries,
            eocd,
            source_path: Some(path.to_path_buf()),
        })
    }

    fn parse_bytes(bytes: Bytes) -> Result<Self, SignError> {
        let raw = bytes.as_ref();
        let eocd = find_eocd(raw)?;

        // Parse central directory to get the list of entries + their offsets.
        let cd_start = eocd.cd_offset as usize;
        let cd_end = cd_start + eocd.cd_size as usize;
        if cd_end > raw.len() {
            return Err(SignError::InvalidZip(
                "central directory extends beyond file",
            ));
        }

        let mut entries = Vec::with_capacity(eocd.total_entries as usize);
        let mut pos = cd_start;

        for _ in 0..eocd.total_entries {
            if pos + 46 > raw.len() {
                return Err(SignError::InvalidZip("central directory entry too short"));
            }
            let sig = Self::read_u32(raw, pos)?;
            if sig != CENTRAL_DIR_SIG {
                return Err(SignError::InvalidZip(
                    "central directory signature mismatch",
                ));
            }

            let method = Self::read_u16(raw, pos + 10)?;
            let fn_len = Self::read_u16(raw, pos + 28)? as usize;
            let ef_len = Self::read_u16(raw, pos + 30)? as usize;
            let cm_len = Self::read_u16(raw, pos + 32)? as usize;
            let local_offset = Self::read_u32(raw, pos + 42)?;

            let cd_entry_len = 46 + fn_len + ef_len + cm_len;
            if pos + cd_entry_len > raw.len() {
                return Err(SignError::InvalidZip(
                    "central directory entry data out of bounds",
                ));
            }

            let cd_entry = raw[pos..pos + cd_entry_len].to_vec();
            let name_bytes = raw[pos + 46..pos + 46 + fn_len].to_vec();
            let name = String::from_utf8_lossy(&name_bytes).into_owned();

            pos += cd_entry_len;

            // Now parse the local file header at local_offset.
            let lh_pos = local_offset as usize;
            if lh_pos + LOCAL_HEADER_FIXED > raw.len() {
                return Err(SignError::InvalidZip("local header offset out of bounds"));
            }
            let lh_sig = Self::read_u32(raw, lh_pos)?;
            if lh_sig != LOCAL_FILE_HEADER_SIG {
                return Err(SignError::InvalidZip(
                    "local file header signature mismatch",
                ));
            }

            let lh_fn_len = Self::read_u16(raw, lh_pos + 26)? as usize;
            let lh_ef_len = Self::read_u16(raw, lh_pos + 28)? as usize;

            let mut local_header_fixed = [0u8; LOCAL_HEADER_FIXED];
            local_header_fixed.copy_from_slice(&raw[lh_pos..lh_pos + LOCAL_HEADER_FIXED]);

            let filename_bytes =
                raw[lh_pos + LOCAL_HEADER_FIXED..lh_pos + LOCAL_HEADER_FIXED + lh_fn_len].to_vec();
            let extra_bytes = raw[lh_pos + LOCAL_HEADER_FIXED + lh_fn_len
                ..lh_pos + LOCAL_HEADER_FIXED + lh_fn_len + lh_ef_len]
                .to_vec();

            let data_start = lh_pos + LOCAL_HEADER_FIXED + lh_fn_len + lh_ef_len;

            // Compressed size comes from CD. This also handles data descriptors,
            // and a zero value is valid for empty file entries.
            let cd_comp_size = Self::read_u32(&cd_entry, 20)? as usize;
            let data_size = cd_comp_size;

            if data_start + data_size > raw.len() {
                return Err(SignError::InvalidZip("file data extends beyond file"));
            }

            let data = ZipEntryData::Memory(bytes.slice(data_start..data_start + data_size));

            let is_runnable = ZipEntryName::new(&name).is_runnable();

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

        Ok(HapZip {
            entries,
            eocd,
            source_path: None,
        })
    }

    fn parse_file_entries(
        file: &mut fs_err::File,
        cd: &[u8],
        eocd: &EocdRecord,
        file_len: u64,
    ) -> Result<Vec<ZipEntry>, SignError> {
        let mut entries = Vec::with_capacity(eocd.total_entries as usize);
        let mut pos = 0usize;

        for _ in 0..eocd.total_entries {
            if pos + 46 > cd.len() {
                return Err(SignError::InvalidZip("central directory entry too short"));
            }
            let sig = Self::read_u32(cd, pos)?;
            if sig != CENTRAL_DIR_SIG {
                return Err(SignError::InvalidZip(
                    "central directory signature mismatch",
                ));
            }

            let method = Self::read_u16(cd, pos + 10)?;
            let fn_len = Self::read_u16(cd, pos + 28)? as usize;
            let ef_len = Self::read_u16(cd, pos + 30)? as usize;
            let cm_len = Self::read_u16(cd, pos + 32)? as usize;
            let local_offset = Self::read_u32(cd, pos + 42)?;

            let cd_entry_len = 46 + fn_len + ef_len + cm_len;
            if pos + cd_entry_len > cd.len() {
                return Err(SignError::InvalidZip(
                    "central directory entry data out of bounds",
                ));
            }

            let cd_entry = cd[pos..pos + cd_entry_len].to_vec();
            let name_bytes = cd[pos + 46..pos + 46 + fn_len].to_vec();
            let name = String::from_utf8_lossy(&name_bytes).into_owned();
            pos += cd_entry_len;

            let lh_pos = local_offset as u64;
            if lh_pos + LOCAL_HEADER_FIXED as u64 > file_len {
                return Err(SignError::InvalidZip("local header offset out of bounds"));
            }
            file.seek(SeekFrom::Start(lh_pos))?;
            let mut local_header_fixed = [0u8; LOCAL_HEADER_FIXED];
            file.read_exact(&mut local_header_fixed)?;
            if u32::from_le_bytes([
                local_header_fixed[0],
                local_header_fixed[1],
                local_header_fixed[2],
                local_header_fixed[3],
            ]) != LOCAL_FILE_HEADER_SIG
            {
                return Err(SignError::InvalidZip(
                    "local file header signature mismatch",
                ));
            }

            let lh_fn_len =
                u16::from_le_bytes([local_header_fixed[26], local_header_fixed[27]]) as usize;
            let lh_ef_len =
                u16::from_le_bytes([local_header_fixed[28], local_header_fixed[29]]) as usize;
            let mut filename_bytes = vec![0u8; lh_fn_len];
            file.read_exact(&mut filename_bytes)?;
            let mut extra_bytes = vec![0u8; lh_ef_len];
            file.read_exact(&mut extra_bytes)?;

            let data_start =
                lh_pos + LOCAL_HEADER_FIXED as u64 + lh_fn_len as u64 + lh_ef_len as u64;
            let data_size = Self::read_u32(&cd_entry, 20)? as usize;
            if data_start + data_size as u64 > file_len {
                return Err(SignError::InvalidZip("file data extends beyond file"));
            }

            let is_runnable = ZipEntryName::new(&name).is_runnable();
            entries.push(ZipEntry {
                original_offset: local_offset,
                local_header_fixed,
                filename: filename_bytes,
                extra: extra_bytes,
                data: ZipEntryData::FileRange {
                    offset: data_start,
                    len: data_size,
                },
                name,
                method,
                is_runnable,
                cd_entry,
            });
        }

        Ok(entries)
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
                entry.extra.extend(std::iter::repeat_n(0u8, needed_padding));
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
        let eocd_zeroed = self.build_eocd_zeroed(&cd_bytes);

        (entries_bytes, cd_bytes, eocd_zeroed)
    }

    /// Compute the content digest for initial signing without materializing the
    /// local-entry section as one contiguous buffer.
    pub fn content_digest_for_signing(&self) -> Result<[u8; 32], SignError> {
        let (entry_offsets, entries_len) = self.entry_offsets();
        let cd_bytes = self.build_cd(&entry_offsets);
        let eocd_zeroed = self.build_eocd_zeroed(&cd_bytes);

        let mut digest =
            digest::HapDigestComputer::new(&[entries_len, 0, cd_bytes.len(), eocd_zeroed.len()]);
        digest.try_update_section_pieces(entries_len, |push| {
            self.try_for_each_entry_section_piece(push)
        })?;
        digest.update_section(&[]);
        digest.update_section(&cd_bytes);
        digest.update_section(&eocd_zeroed);
        Ok(digest.finalize())
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

    /// Insert signing block between entries and CD, writing the signed HAP bytes
    /// directly to the supplied writer.
    pub fn write_with_signing_block<W: Write>(
        &self,
        writer: &mut W,
        signing_block: &[u8],
    ) -> Result<(), SignError> {
        let (entry_offsets, entries_len) = self.entry_offsets();
        let cd_bytes = self.build_cd(&entry_offsets);
        let cd_offset = entries_len + signing_block.len();
        let eocd_bytes = self.build_eocd(cd_offset as u32, &cd_bytes);

        for entry in &self.entries {
            writer.write_all(&entry.local_header_fixed)?;
            writer.write_all(&entry.filename)?;
            writer.write_all(&entry.extra)?;
            self.write_entry_data(writer, entry)?;
        }
        writer.write_all(signing_block)?;
        writer.write_all(&cd_bytes)?;
        writer.write_all(&eocd_bytes)?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Build the serialized entries and (updated) central directory.
    /// `signing_block_len` is added to the CD offset base (for final assembly).
    fn build_entries_and_cd(&self, signing_block_len: usize) -> (Vec<u8>, Vec<u8>, usize) {
        let mut entries_bytes = Vec::new();
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
            entries_bytes.extend_from_slice(
                entry
                    .data
                    .memory_bytes()
                    .expect("with_signing_block is only supported for memory-backed HAPs"),
            );
        }

        let cd_base = entries_bytes.len() + signing_block_len;
        let cd_bytes = self.build_cd(&entry_offsets);

        (entries_bytes, cd_bytes, cd_base)
    }

    fn entry_offsets(&self) -> (Vec<usize>, usize) {
        let mut offsets = Vec::with_capacity(self.entries.len());
        let mut current_offset = 0usize;
        for entry in &self.entries {
            offsets.push(current_offset);
            current_offset +=
                LOCAL_HEADER_FIXED + entry.filename.len() + entry.extra.len() + entry.data.len();
        }
        (offsets, current_offset)
    }

    fn try_for_each_entry_section_piece(
        &self,
        push: &mut dyn FnMut(&[u8]),
    ) -> Result<(), SignError> {
        for entry in &self.entries {
            push(&entry.local_header_fixed);
            push(&entry.filename);
            push(&entry.extra);
            self.push_entry_data(entry, push)?;
        }
        Ok(())
    }

    fn push_entry_data(
        &self,
        entry: &ZipEntry,
        push: &mut dyn FnMut(&[u8]),
    ) -> Result<(), SignError> {
        match &entry.data {
            ZipEntryData::Memory(bytes) => {
                push(bytes);
                Ok(())
            }
            ZipEntryData::FileRange { offset, len } => {
                let Some(source_path) = &self.source_path else {
                    return Err(SignError::InvalidZip(
                        "file-backed ZIP entry is missing a source path",
                    ));
                };
                let mut file = fs_err::File::open(source_path)?;
                file.seek(SeekFrom::Start(*offset))?;
                Self::read_range_pieces(&mut file, *len, push)
            }
        }
    }

    fn read_range_pieces(
        file: &mut fs_err::File,
        mut remaining: usize,
        push: &mut dyn FnMut(&[u8]),
    ) -> Result<(), SignError> {
        let mut buffer = vec![0u8; 256 * 1024];
        while remaining > 0 {
            let to_read = remaining.min(buffer.len());
            file.read_exact(&mut buffer[..to_read])?;
            push(&buffer[..to_read]);
            remaining -= to_read;
        }
        Ok(())
    }

    fn write_entry_data<W: Write>(&self, writer: &mut W, entry: &ZipEntry) -> Result<(), SignError>
    where
        W: Write,
    {
        match &entry.data {
            ZipEntryData::Memory(bytes) => {
                writer.write_all(bytes)?;
                Ok(())
            }
            ZipEntryData::FileRange { offset, len } => {
                let Some(source_path) = &self.source_path else {
                    return Err(SignError::InvalidZip(
                        "file-backed ZIP entry is missing a source path",
                    ));
                };
                let mut file = fs_err::File::open(source_path)?;
                file.seek(SeekFrom::Start(*offset))?;
                let mut remaining = *len as u64;
                let copied = std::io::copy(&mut file.take(remaining), writer)?;
                remaining = remaining.saturating_sub(copied);
                if remaining != 0 {
                    return Err(SignError::InvalidZip("file data ended before range length"));
                }
                Ok(())
            }
        }
    }

    fn build_cd(&self, entry_offsets: &[usize]) -> Vec<u8> {
        let mut cd_bytes = Vec::new();
        for (entry, local_offset) in self.entries.iter().zip(entry_offsets.iter().copied()) {
            let local_offset = local_offset as u32;
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
        cd_bytes
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

    fn build_eocd_zeroed(&self, cd_bytes: &[u8]) -> Vec<u8> {
        let mut eocd_zeroed = self.build_eocd(0, cd_bytes);
        // Bytes 16..20 of EOCD are the CD offset field included as zero in the
        // initial-signing digest, matching developtools_hapsigner.
        if eocd_zeroed.len() >= 20 {
            eocd_zeroed[16] = 0;
            eocd_zeroed[17] = 0;
            eocd_zeroed[18] = 0;
            eocd_zeroed[19] = 0;
        }
        eocd_zeroed
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
    fn test_parse_zero_length_file_uses_cd_compressed_size() {
        let bytes = make_zip_bytes(b"empty.txt", b"");
        let hap = HapZip::parse(&bytes).expect("zero-length file must parse successfully");
        assert_eq!(hap.entries[0].data.len(), 0);
    }

    #[test]
    fn test_parse_owned_matches_slice_parse_sections_and_output() {
        let bytes = make_zip_bytes(b"ets/modules.abc", &[0x7Au8; 8193]);
        let mut borrowed = HapZip::parse(&bytes).expect("borrowed parse");
        let mut owned = HapZip::parse_owned(bytes).expect("owned parse");
        borrowed.sort_entries();
        owned.sort_entries();
        borrowed.align();
        owned.align();

        assert_eq!(
            borrowed.sections_for_signing(),
            owned.sections_for_signing()
        );

        let signing_block = vec![0xCDu8; 128];
        let mut borrowed_out = Vec::new();
        let mut owned_out = Vec::new();
        borrowed
            .write_with_signing_block(&mut borrowed_out, &signing_block)
            .expect("borrowed output");
        owned
            .write_with_signing_block(&mut owned_out, &signing_block)
            .expect("owned output");
        assert_eq!(borrowed_out, owned_out);
    }

    #[test]
    fn test_parse_file_matches_memory_parse_digest_and_output() {
        let bytes = make_zip_bytes(b"ets/modules.abc", &[0x7Au8; 8193]);
        let temp = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(temp.path(), &bytes).expect("write temp zip");

        let mut memory = HapZip::parse(&bytes).expect("memory parse");
        let mut file = HapZip::parse_file(temp.path()).expect("file parse");
        memory.sort_entries();
        file.sort_entries();
        memory.align();
        file.align();

        assert_eq!(
            memory.content_digest_for_signing().expect("memory digest"),
            file.content_digest_for_signing().expect("file digest")
        );

        let signing_block = vec![0xCDu8; 128];
        let mut memory_out = Vec::new();
        let mut file_out = Vec::new();
        memory
            .write_with_signing_block(&mut memory_out, &signing_block)
            .expect("memory output");
        file.write_with_signing_block(&mut file_out, &signing_block)
            .expect("file output");
        assert_eq!(memory_out, file_out);
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

    #[test]
    fn test_streaming_signing_block_writer_matches_vec_output() {
        let bytes = make_zip_bytes(b"test.txt", b"payload");
        let hap = HapZip::parse(&bytes).expect("parse");
        let signing_block = vec![0xFFu8; 128];
        let signed = hap.with_signing_block(&signing_block);
        let mut streamed = Vec::new();

        hap.write_with_signing_block(&mut streamed, &signing_block)
            .expect("streaming signed output");

        assert_eq!(streamed, signed);
    }

    #[test]
    fn test_streaming_signing_digest_matches_materialized_sections() {
        let bytes = make_zip_bytes(b"ets/modules.abc", &[0xAB; 8193]);
        let mut hap = HapZip::parse(&bytes).expect("parse");
        hap.sort_entries();
        hap.align();

        let expected = {
            let (entries_bytes, cd_bytes, eocd_zeroed) = hap.sections_for_signing();
            digest::compute_hap_digest(&entries_bytes, &[], &cd_bytes, &eocd_zeroed)
        };

        assert_eq!(hap.content_digest_for_signing().expect("digest"), expected);
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
