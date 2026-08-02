use std::io::{BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use flate2::write::DeflateEncoder;
use rayon::prelude::*;

use crate::error::PackError;

const PACK_WRITE_BUFFER_SIZE: usize = 1024 * 1024;
/// SDK `app_packing_tool.jar` defaults `ohos.Utility.compressLevel` to 1.
const HVIGOR_DEFAULT_COMPRESS_LEVEL: u32 = 1;

const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_DIRECTORY_HEADER_SIGNATURE: u32 = 0x0201_4b50;
const ZIP64_END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0606_4b50;
const ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0605_4b50;
const FLAG_UTF8_ENCODING: u16 = 0x800;
const METHOD_STORED: u16 = 0;
const METHOD_DEFLATED: u16 = 8;
const VERSION_MADE_BY: u16 = 20;
const STORED_VERSION_NEEDED: u16 = 10;
const DEFLATED_VERSION_NEEDED: u16 = 20;
const ZIP64_VERSION_NEEDED: u16 = 45;
const ZIP64_THRESHOLD_FILE_SIZE: u64 = u32::MAX as u64;
const ZIP64_THRESHOLD_OFFSET: u64 = u32::MAX as u64;
const ZIP64_THRESHOLD_ENTRIES: usize = u16::MAX as usize;

/// Create a ZIP archive writer backed by a buffered file.
pub fn create_zip(output_path: &Path) -> Result<RawZipWriter<BufWriter<fs_err::File>>, PackError> {
    let file = fs_err::File::create(output_path)?;
    Ok(RawZipWriter::new(BufWriter::with_capacity(
        PACK_WRITE_BUFFER_SIZE,
        file,
    )))
}

/// Add a file to the ZIP with deflate compression.
pub fn add_file_deflate<W: Write + Seek>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    file_path: &Path,
) -> Result<(), PackError> {
    writer.add_file_deflate(archive_path, file_path, flate2::Compression::default())
}

pub fn add_file_deflate_hvigor_default<W: Write + Seek>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    file_path: &Path,
) -> Result<(), PackError> {
    writer.add_file_deflate(
        archive_path,
        file_path,
        flate2::Compression::new(HVIGOR_DEFAULT_COMPRESS_LEVEL),
    )
}

/// Add a file to the ZIP with STORE (no compression).
pub fn add_file_stored<W: Write + Seek>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    file_path: &Path,
) -> Result<(), PackError> {
    writer.add_file_stored(archive_path, file_path)
}

/// Add raw bytes to the ZIP with deflate compression.
pub fn add_bytes_deflate<W: Write + Seek>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    content: &[u8],
) -> Result<(), PackError> {
    writer.add_bytes_deflate(archive_path, content, flate2::Compression::default())
}

/// Add raw bytes to the ZIP with STORE (no compression).
pub fn add_bytes_stored<W: Write + Seek>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    content: &[u8],
) -> Result<(), PackError> {
    writer.add_bytes_stored(archive_path, content)
}

#[derive(Debug)]
pub struct RawZipWriter<W> {
    writer: CountWriter<W>,
    files: Vec<FileHeader>,
    file_names: Vec<u8>,
    modification_time: u16,
    modification_date: u16,
}

impl<W: Write + Seek> RawZipWriter<W> {
    pub fn new(writer: W) -> Self {
        let timestamp = ZipTimestamp::now();
        Self {
            writer: CountWriter::new(writer),
            files: Vec::new(),
            file_names: Vec::new(),
            modification_time: timestamp.time,
            modification_date: timestamp.date,
        }
    }

    pub fn add_file_stored(
        &mut self,
        archive_path: &str,
        file_path: &Path,
    ) -> Result<(), PackError> {
        let file = fs_err::File::open(file_path)?;
        let uncompressed_size = file.metadata()?.len();
        let mut reader = BufReader::with_capacity(PACK_WRITE_BUFFER_SIZE, file);
        self.add_stored(archive_path, uncompressed_size, |entry| {
            std::io::copy(&mut reader, entry)?;
            Ok(())
        })
    }

    pub fn add_file_deflate(
        &mut self,
        archive_path: &str,
        file_path: &Path,
        level: flate2::Compression,
    ) -> Result<(), PackError> {
        let file = fs_err::File::open(file_path)?;
        let uncompressed_size = file.metadata()?.len();
        let mut reader = BufReader::with_capacity(PACK_WRITE_BUFFER_SIZE, file);
        self.add_deflated(archive_path, uncompressed_size, level, |entry| {
            std::io::copy(&mut reader, entry)?;
            Ok(())
        })
    }

    pub fn add_bytes_stored(
        &mut self,
        archive_path: &str,
        content: &[u8],
    ) -> Result<(), PackError> {
        self.add_stored(archive_path, content.len() as u64, |entry| {
            entry.write_all(content)?;
            Ok(())
        })
    }

    pub fn add_bytes_deflate(
        &mut self,
        archive_path: &str,
        content: &[u8],
        level: flate2::Compression,
    ) -> Result<(), PackError> {
        self.add_deflated(archive_path, content.len() as u64, level, |entry| {
            entry.write_all(content)?;
            Ok(())
        })
    }

    pub fn add_precompressed_deflated(
        &mut self,
        entry: &ParallelDeflatedEntry,
    ) -> Result<(), PackError> {
        let header = self.start_file(
            &entry.archive_path,
            METHOD_DEFLATED,
            entry.uncompressed_size,
            Some(entry.compressed_size),
        )?;
        let file = fs_err::File::open(&entry.compressed_path)?;
        let mut reader = BufReader::with_capacity(PACK_WRITE_BUFFER_SIZE, file);
        let copied = std::io::copy(&mut reader, &mut self.writer)?;
        debug_assert_eq!(copied, entry.compressed_size);
        self.finish_file(
            header,
            entry.crc32,
            entry.compressed_size,
            entry.uncompressed_size,
        )
    }

    pub fn finish(mut self) -> Result<W, PackError> {
        let central_directory_offset = self.writer.count();
        let total_entries = self.files.len();
        let needs_zip64 = total_entries >= ZIP64_THRESHOLD_ENTRIES
            || central_directory_offset >= ZIP64_THRESHOLD_OFFSET
            || self.files.iter().any(FileHeader::needs_zip64);

        let files = self.files.clone();
        let file_names = self.file_names.clone();
        let mut name_offset = 0usize;
        for file in &files {
            let name_end = name_offset + file.name_len as usize;
            let name = &file_names[name_offset..name_end];
            name_offset = name_end;
            self.write_central_file_header(file, name)?;
        }

        let central_directory_end = self.writer.count();
        let central_directory_size = central_directory_end - central_directory_offset;

        if needs_zip64 {
            let zip64_eocd_offset = self.writer.count();
            self.write_zip64_end_of_central_directory(
                total_entries as u64,
                central_directory_size,
                central_directory_offset,
            )?;
            self.write_zip64_end_of_central_directory_locator(zip64_eocd_offset)?;
        }

        self.write_end_of_central_directory(
            total_entries,
            central_directory_size,
            central_directory_offset,
        )?;
        self.writer.flush()?;
        Ok(self.writer.into_inner())
    }

    fn add_stored<F>(
        &mut self,
        archive_path: &str,
        uncompressed_size: u64,
        write_entry: F,
    ) -> Result<(), PackError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PackError>,
    {
        let header = self.start_file(
            archive_path,
            METHOD_STORED,
            uncompressed_size,
            Some(uncompressed_size),
        )?;
        let payload = EntryPayloadWriter::new(&mut self.writer);
        let mut data_writer = TrackingWriter::new(payload);
        write_entry(&mut data_writer)?;
        let (payload, stats) = data_writer.finish()?;
        let compressed_size = payload.bytes;
        self.finish_file(
            header,
            stats.crc32,
            compressed_size,
            stats.uncompressed_size,
        )
    }

    fn add_deflated<F>(
        &mut self,
        archive_path: &str,
        uncompressed_size: u64,
        level: flate2::Compression,
        write_entry: F,
    ) -> Result<(), PackError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PackError>,
    {
        let header = self.start_file(archive_path, METHOD_DEFLATED, uncompressed_size, None)?;
        let payload = EntryPayloadWriter::new(&mut self.writer);
        let encoder = DeflateEncoder::new(payload, level);
        let mut data_writer = TrackingWriter::new(encoder);
        write_entry(&mut data_writer)?;
        let (encoder, stats) = data_writer.finish()?;
        let payload = encoder.finish()?;
        let compressed_size = payload.bytes;
        self.finish_file(
            header,
            stats.crc32,
            compressed_size,
            stats.uncompressed_size,
        )
    }

    fn start_file(
        &mut self,
        archive_path: &str,
        method: u16,
        uncompressed_size: u64,
        compressed_size: Option<u64>,
    ) -> Result<PendingFile, PackError> {
        let name = archive_path.trim_end_matches('/');
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("ZIP entry name too long: {name}"),
            )
            .into());
        }

        let flags = utf8_flag(name);
        let local_header_offset = self.writer.count();
        let zip64 = local_header_offset >= ZIP64_THRESHOLD_OFFSET
            || uncompressed_size >= ZIP64_THRESHOLD_FILE_SIZE
            || compressed_size.is_some_and(|size| size >= ZIP64_THRESHOLD_FILE_SIZE)
            || (method == METHOD_DEFLATED
                && deflate_bound(uncompressed_size) >= ZIP64_THRESHOLD_FILE_SIZE);
        self.write_local_file_header(
            name_bytes,
            flags,
            method,
            zip64,
            uncompressed_size,
            compressed_size,
        )?;
        self.file_names.extend_from_slice(name_bytes);

        Ok(PendingFile {
            name_len: name_bytes.len() as u16,
            method,
            flags,
            local_header_offset,
            zip64,
        })
    }

    fn finish_file(
        &mut self,
        pending: PendingFile,
        crc32: u32,
        compressed_size: u64,
        uncompressed_size: u64,
    ) -> Result<(), PackError> {
        let payload_end = self.writer.count();
        self.patch_local_file_header(&pending, crc32, compressed_size, uncompressed_size)?;
        self.writer.seek(SeekFrom::Start(payload_end))?;
        self.files.push(FileHeader {
            name_len: pending.name_len,
            method: pending.method,
            local_header_offset: pending.local_header_offset,
            compressed_size,
            uncompressed_size,
            crc32,
            flags: pending.flags,
            modification_time: self.modification_time,
            modification_date: self.modification_date,
            zip64: pending.zip64,
        });
        Ok(())
    }

    fn write_local_file_header(
        &mut self,
        name: &[u8],
        flags: u16,
        method: u16,
        zip64: bool,
        uncompressed_size: u64,
        compressed_size: Option<u64>,
    ) -> Result<(), PackError> {
        self.write_u32(LOCAL_FILE_HEADER_SIGNATURE)?;
        self.write_u16(version_needed(method, zip64))?;
        self.write_u16(flags)?;
        self.write_u16(method)?;
        self.write_u16(self.modification_time)?;
        self.write_u16(self.modification_date)?;
        self.write_u32(0)?;
        self.write_u32(if zip64 { u32::MAX } else { 0 })?;
        self.write_u32(if zip64 { u32::MAX } else { 0 })?;
        self.write_u16(name.len() as u16)?;
        self.write_u16(if zip64 { 20 } else { 0 })?;
        self.writer.write_all(name)?;
        if zip64 {
            self.write_u16(0x0001)?;
            self.write_u16(16)?;
            self.write_u64(uncompressed_size)?;
            self.write_u64(compressed_size.unwrap_or(0))?;
        }
        Ok(())
    }

    fn patch_local_file_header(
        &mut self,
        pending: &PendingFile,
        crc32: u32,
        compressed_size: u64,
        uncompressed_size: u64,
    ) -> Result<(), PackError> {
        self.writer
            .seek(SeekFrom::Start(pending.local_header_offset + 14))?;
        self.write_u32(crc32)?;
        if pending.zip64 {
            self.writer.seek(SeekFrom::Start(
                pending.local_header_offset + 30 + u64::from(pending.name_len) + 4,
            ))?;
            self.write_u64(uncompressed_size)?;
            self.write_u64(compressed_size)?;
        } else {
            debug_assert!(compressed_size < ZIP64_THRESHOLD_FILE_SIZE);
            debug_assert!(uncompressed_size < ZIP64_THRESHOLD_FILE_SIZE);
            self.write_u32(compressed_size as u32)?;
            self.write_u32(uncompressed_size as u32)?;
        }
        Ok(())
    }

    fn write_central_file_header(
        &mut self,
        file: &FileHeader,
        name: &[u8],
    ) -> Result<(), PackError> {
        let version_needed = version_needed(file.method, file.needs_zip64());
        let zip64_extra = file.zip64_extra_field();

        self.write_u32(CENTRAL_DIRECTORY_HEADER_SIGNATURE)?;
        self.write_u16(VERSION_MADE_BY)?;
        self.write_u16(version_needed)?;
        self.write_u16(file.flags)?;
        self.write_u16(file.method)?;
        self.write_u16(file.modification_time)?;
        self.write_u16(file.modification_date)?;
        self.write_u32(file.crc32)?;
        self.write_u32(if file.zip64 {
            u32::MAX
        } else {
            file.compressed_size as u32
        })?;
        self.write_u32(if file.zip64 {
            u32::MAX
        } else {
            file.uncompressed_size as u32
        })?;
        self.write_u16(file.name_len)?;
        self.write_u16(zip64_extra.len() as u16)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        self.write_u32(0)?;
        self.write_u32(file.local_header_offset.min(ZIP64_THRESHOLD_OFFSET) as u32)?;
        self.writer.write_all(name)?;
        self.writer.write_all(&zip64_extra)?;
        Ok(())
    }

    fn write_zip64_end_of_central_directory(
        &mut self,
        total_entries: u64,
        central_directory_size: u64,
        central_directory_offset: u64,
    ) -> Result<(), PackError> {
        self.write_u32(ZIP64_END_OF_CENTRAL_DIRECTORY_SIGNATURE)?;
        self.write_u64(44)?;
        self.write_u16(ZIP64_VERSION_NEEDED)?;
        self.write_u16(ZIP64_VERSION_NEEDED)?;
        self.write_u32(0)?;
        self.write_u32(0)?;
        self.write_u64(total_entries)?;
        self.write_u64(total_entries)?;
        self.write_u64(central_directory_size)?;
        self.write_u64(central_directory_offset)?;
        Ok(())
    }

    fn write_zip64_end_of_central_directory_locator(
        &mut self,
        zip64_eocd_offset: u64,
    ) -> Result<(), PackError> {
        self.write_u32(ZIP64_END_OF_CENTRAL_DIRECTORY_LOCATOR_SIGNATURE)?;
        self.write_u32(0)?;
        self.write_u64(zip64_eocd_offset)?;
        self.write_u32(1)?;
        Ok(())
    }

    fn write_end_of_central_directory(
        &mut self,
        total_entries: usize,
        central_directory_size: u64,
        central_directory_offset: u64,
    ) -> Result<(), PackError> {
        self.write_u32(END_OF_CENTRAL_DIRECTORY_SIGNATURE)?;
        self.write_u16(0)?;
        self.write_u16(0)?;
        let entry_count = total_entries.min(ZIP64_THRESHOLD_ENTRIES) as u16;
        self.write_u16(entry_count)?;
        self.write_u16(entry_count)?;
        self.write_u32(central_directory_size.min(ZIP64_THRESHOLD_OFFSET) as u32)?;
        self.write_u32(central_directory_offset.min(ZIP64_THRESHOLD_OFFSET) as u32)?;
        self.write_u16(0)?;
        Ok(())
    }

    fn write_u16(&mut self, value: u16) -> Result<(), PackError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    fn write_u32(&mut self, value: u32) -> Result<(), PackError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }

    fn write_u64(&mut self, value: u64) -> Result<(), PackError> {
        self.writer.write_all(&value.to_le_bytes())?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ParallelDeflatedFiles {
    _temp_dir: tempfile::TempDir,
    entries: Vec<ParallelDeflatedEntry>,
}

impl ParallelDeflatedFiles {
    pub fn compress_hvigor_default(entries: Vec<(String, PathBuf)>) -> Result<Self, PackError> {
        Self::compress(entries, HVIGOR_DEFAULT_COMPRESS_LEVEL)
    }

    pub fn compress(
        entries: Vec<(String, PathBuf)>,
        compress_level: u32,
    ) -> Result<Self, PackError> {
        let temp_dir = tempfile::Builder::new()
            .prefix("parallelscatter")
            .tempdir()?;
        let temp_root = temp_dir.path().to_path_buf();
        let mut compressed = entries
            .into_par_iter()
            .enumerate()
            .map(|(index, (archive_path, source_path))| {
                ParallelDeflatedEntry::compress(
                    index,
                    archive_path,
                    source_path,
                    &temp_root,
                    compress_level,
                )
            })
            .collect::<Result<Vec<_>, PackError>>()?;
        compressed.sort_by_key(|entry| entry.index);
        Ok(Self {
            _temp_dir: temp_dir,
            entries: compressed,
        })
    }

    pub fn entries(&self) -> &[ParallelDeflatedEntry] {
        &self.entries
    }
}

#[derive(Debug)]
pub struct ParallelDeflatedEntry {
    index: usize,
    archive_path: String,
    compressed_path: PathBuf,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
}

impl ParallelDeflatedEntry {
    fn compress(
        index: usize,
        archive_path: String,
        source_path: PathBuf,
        temp_root: &Path,
        compress_level: u32,
    ) -> Result<Self, PackError> {
        let compressed_path = temp_root.join(format!("n{}", index + 1));
        let source = fs_err::File::open(&source_path)?;
        let mut reader = BufReader::with_capacity(PACK_WRITE_BUFFER_SIZE, source);
        let output = fs_err::File::create(&compressed_path)?;
        let output = BufWriter::with_capacity(PACK_WRITE_BUFFER_SIZE, output);
        let encoder = DeflateEncoder::new(output, flate2::Compression::new(compress_level));
        let mut data_writer = TrackingWriter::new(encoder);
        std::io::copy(&mut reader, &mut data_writer)?;
        let (encoder, stats) = data_writer.finish()?;
        let mut output = encoder.finish()?;
        output.flush()?;
        drop(output);
        let compressed_size = fs_err::metadata(&compressed_path)?.len();
        Ok(Self {
            index,
            archive_path,
            compressed_path,
            compressed_size,
            uncompressed_size: stats.uncompressed_size,
            crc32: stats.crc32,
        })
    }
}

#[derive(Debug)]
struct PendingFile {
    name_len: u16,
    method: u16,
    flags: u16,
    local_header_offset: u64,
    zip64: bool,
}

#[derive(Clone, Debug)]
struct FileHeader {
    name_len: u16,
    method: u16,
    local_header_offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    flags: u16,
    modification_time: u16,
    modification_date: u16,
    zip64: bool,
}

impl FileHeader {
    fn needs_zip64(&self) -> bool {
        self.zip64 || self.local_header_offset >= ZIP64_THRESHOLD_OFFSET
    }

    fn zip64_extra_field(&self) -> Vec<u8> {
        if !self.needs_zip64() {
            return Vec::new();
        }

        let mut data = Vec::with_capacity(28);
        if self.zip64 {
            data.extend_from_slice(&self.uncompressed_size.to_le_bytes());
            data.extend_from_slice(&self.compressed_size.to_le_bytes());
        }
        if self.local_header_offset >= ZIP64_THRESHOLD_OFFSET {
            data.extend_from_slice(&self.local_header_offset.to_le_bytes());
        }

        let mut extra = Vec::with_capacity(4 + data.len());
        extra.extend_from_slice(&0x0001u16.to_le_bytes());
        extra.extend_from_slice(&(data.len() as u16).to_le_bytes());
        extra.extend_from_slice(&data);
        extra
    }
}

#[derive(Debug)]
struct CountWriter<W> {
    writer: W,
    count: u64,
}

impl<W> CountWriter<W> {
    fn new(writer: W) -> Self {
        Self { writer, count: 0 }
    }

    fn count(&self) -> u64 {
        self.count
    }

    fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> Write for CountWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.writer.write(buf)?;
        self.count += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<W: Seek> Seek for CountWriter<W> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let position = self.writer.seek(position)?;
        self.count = position;
        Ok(position)
    }
}

#[derive(Debug)]
struct EntryPayloadWriter<'a, W> {
    writer: &'a mut CountWriter<W>,
    bytes: u64,
}

impl<'a, W> EntryPayloadWriter<'a, W> {
    fn new(writer: &'a mut CountWriter<W>) -> Self {
        Self { writer, bytes: 0 }
    }
}

impl<W: Write> Write for EntryPayloadWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.writer.write(buf)?;
        self.bytes += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Debug)]
struct TrackingWriter<W> {
    inner: W,
    hasher: crc32fast::Hasher,
    uncompressed_size: u64,
}

impl<W> TrackingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: crc32fast::Hasher::new(),
            uncompressed_size: 0,
        }
    }
}

impl<W: Write> TrackingWriter<W> {
    fn finish(mut self) -> Result<(W, EntryStats), PackError> {
        self.inner.flush()?;
        let stats = EntryStats {
            crc32: self.hasher.finalize(),
            uncompressed_size: self.uncompressed_size,
        };
        Ok((self.inner, stats))
    }
}

impl<W: Write> Write for TrackingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        self.uncompressed_size += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[derive(Debug)]
struct EntryStats {
    crc32: u32,
    uncompressed_size: u64,
}

fn utf8_flag(_name: &str) -> u16 {
    FLAG_UTF8_ENCODING
}

fn version_needed(method: u16, zip64: bool) -> u16 {
    if zip64 {
        ZIP64_VERSION_NEEDED
    } else if method == METHOD_STORED {
        STORED_VERSION_NEEDED
    } else {
        DEFLATED_VERSION_NEEDED
    }
}

fn deflate_bound(uncompressed_size: u64) -> u64 {
    uncompressed_size
        .saturating_add(
            uncompressed_size
                .saturating_div(16_383)
                .saturating_add(1)
                .saturating_mul(5),
        )
        .saturating_add(6)
}

struct ZipTimestamp {
    time: u16,
    date: u16,
}

impl ZipTimestamp {
    fn now() -> Self {
        let now =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        let now =
            zip::DateTime::try_from(now).unwrap_or_else(|_| zip::DateTime::default_for_write());
        Self {
            time: u16::from(now.second() / 2)
                | (u16::from(now.minute()) << 5)
                | (u16::from(now.hour()) << 11),
            date: u16::from(now.day())
                | (u16::from(now.month()) << 5)
                | ((now.year().saturating_sub(1980)) << 9),
        }
    }
}
