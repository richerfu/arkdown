use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use flate2::write::DeflateEncoder;
use rawzip::{CompressionMethod, ZipArchiveWriter};

use crate::error::PackError;

const PACK_WRITE_BUFFER_SIZE: usize = 1024 * 1024;

pub type RawZipWriter<W> = ZipArchiveWriter<W>;

/// Create a ZIP archive writer backed by rawzip.
pub fn create_zip(output_path: &Path) -> Result<RawZipWriter<BufWriter<fs_err::File>>, PackError> {
    let file = fs_err::File::create(output_path)?;
    Ok(ZipArchiveWriter::new(BufWriter::with_capacity(
        PACK_WRITE_BUFFER_SIZE,
        file,
    )))
}

/// Add a file to the ZIP with deflate compression.
pub fn add_file_deflate<W: Write>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    file_path: &Path,
) -> Result<(), PackError> {
    let file = fs_err::File::open(file_path)?;
    let mut reader = BufReader::with_capacity(PACK_WRITE_BUFFER_SIZE, file);
    writer.add_reader(archive_path, &mut reader, ZipCompression::Deflate)
}

/// Add a file to the ZIP with STORE (no compression).
pub fn add_file_stored<W: Write>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    file_path: &Path,
) -> Result<(), PackError> {
    let file = fs_err::File::open(file_path)?;
    let mut reader = BufReader::with_capacity(PACK_WRITE_BUFFER_SIZE, file);
    writer.add_reader(archive_path, &mut reader, ZipCompression::Stored)
}

/// Add raw bytes to the ZIP with deflate compression.
pub fn add_bytes_deflate<W: Write>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    content: &[u8],
) -> Result<(), PackError> {
    writer.add_bytes(archive_path, content, ZipCompression::Deflate)
}

/// Add raw bytes to the ZIP with STORE (no compression).
pub fn add_bytes_stored<W: Write>(
    writer: &mut RawZipWriter<W>,
    archive_path: &str,
    content: &[u8],
) -> Result<(), PackError> {
    writer.add_bytes(archive_path, content, ZipCompression::Stored)
}

enum ZipCompression {
    Stored,
    Deflate,
}

trait RawZipWriterExt {
    fn add_reader<R: std::io::Read>(
        &mut self,
        archive_path: &str,
        reader: &mut R,
        compression: ZipCompression,
    ) -> Result<(), PackError>;

    fn add_bytes(
        &mut self,
        archive_path: &str,
        content: &[u8],
        compression: ZipCompression,
    ) -> Result<(), PackError>;
}

impl<W: Write> RawZipWriterExt for RawZipWriter<W> {
    fn add_reader<R: std::io::Read>(
        &mut self,
        archive_path: &str,
        reader: &mut R,
        compression: ZipCompression,
    ) -> Result<(), PackError> {
        match compression {
            ZipCompression::Stored => self.add_stored(archive_path, |entry| {
                std::io::copy(reader, entry)?;
                Ok(())
            }),
            ZipCompression::Deflate => self.add_deflated(archive_path, |entry| {
                std::io::copy(reader, entry)?;
                Ok(())
            }),
        }
    }

    fn add_bytes(
        &mut self,
        archive_path: &str,
        content: &[u8],
        compression: ZipCompression,
    ) -> Result<(), PackError> {
        match compression {
            ZipCompression::Stored => self.add_stored(archive_path, |entry| {
                entry.write_all(content)?;
                Ok(())
            }),
            ZipCompression::Deflate => self.add_deflated(archive_path, |entry| {
                entry.write_all(content)?;
                Ok(())
            }),
        }
    }
}

trait RawZipEntryExt {
    fn add_stored<F>(&mut self, archive_path: &str, write_entry: F) -> Result<(), PackError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PackError>;

    fn add_deflated<F>(&mut self, archive_path: &str, write_entry: F) -> Result<(), PackError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PackError>;
}

impl<W: Write> RawZipEntryExt for RawZipWriter<W> {
    fn add_stored<F>(&mut self, archive_path: &str, write_entry: F) -> Result<(), PackError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PackError>,
    {
        let (mut entry, config) = self
            .new_file(archive_path)
            .compression_method(CompressionMethod::Store)
            .start()?;
        let mut data_writer = config.wrap(&mut entry);
        write_entry(&mut data_writer)?;
        let (_, descriptor) = data_writer.finish()?;
        entry.finish(descriptor)?;
        Ok(())
    }

    fn add_deflated<F>(&mut self, archive_path: &str, write_entry: F) -> Result<(), PackError>
    where
        F: FnOnce(&mut dyn Write) -> Result<(), PackError>,
    {
        let (mut entry, config) = self
            .new_file(archive_path)
            .compression_method(CompressionMethod::Deflate)
            .start()?;
        let encoder = DeflateEncoder::new(&mut entry, flate2::Compression::default());
        let mut data_writer = config.wrap(encoder);
        write_entry(&mut data_writer)?;
        let (encoder, descriptor) = data_writer.finish()?;
        encoder.finish()?;
        entry.finish(descriptor)?;
        Ok(())
    }
}
