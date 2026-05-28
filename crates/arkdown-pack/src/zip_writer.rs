use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

/// Create a zip archive at `output_path`.
pub fn create_zip(output_path: &Path) -> Result<ZipWriter<std::fs::File>, crate::error::PackError> {
    let file = std::fs::File::create(output_path)?;
    Ok(ZipWriter::new(file))
}

/// Add a file to the zip with deflate compression (for JSON/text).
pub fn add_file_deflate<W: Write + std::io::Seek>(
    writer: &mut ZipWriter<W>,
    archive_path: &str,
    file_path: &Path,
) -> Result<(), crate::error::PackError> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(archive_path, options)?;
    let content = fs_err::read(file_path)?;
    writer.write_all(&content)?;
    Ok(())
}

/// Add a file to the zip with STORE (no compression) — required for .abc and .so files.
pub fn add_file_stored<W: Write + std::io::Seek>(
    writer: &mut ZipWriter<W>,
    archive_path: &str,
    file_path: &Path,
) -> Result<(), crate::error::PackError> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file(archive_path, options)?;
    let content = fs_err::read(file_path)?;
    writer.write_all(&content)?;
    Ok(())
}

/// Add raw bytes to the zip with deflate compression.
pub fn add_bytes_deflate<W: Write + std::io::Seek>(
    writer: &mut ZipWriter<W>,
    archive_path: &str,
    content: &[u8],
) -> Result<(), crate::error::PackError> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(archive_path, options)?;
    writer.write_all(content)?;
    Ok(())
}
