use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownError, ChunkGraph};

/// Write every chunk in `chunk_graph` to `output_dir` as `<name>.js`.
///
/// Creates the output directory (and any missing parents) if it does not exist.
/// Returns a list of `(chunk_name, absolute_output_path)` pairs.
pub fn write_chunks(
    chunk_graph: &ChunkGraph,
    output_dir: &Path,
) -> Result<Vec<(String, PathBuf)>, ArkdownError> {
    fs_err::create_dir_all(output_dir).map_err(|e| ArkdownError::Io(e.into()))?;

    let mut files = Vec::new();

    for chunk in chunk_graph.chunks.iter() {
        let filename = format!("{}.js", chunk.name);
        let path = output_dir.join(&filename);
        fs_err::write(&path, &chunk.code).map_err(|e| ArkdownError::Io(e.into()))?;
        files.push((chunk.name.clone(), path));

        // Write source map alongside the chunk if one was produced.
        if let Some(ref map) = chunk.source_map {
            let map_path = output_dir.join(format!("{}.js.map", chunk.name));
            fs_err::write(&map_path, map.as_str()).map_err(|e| ArkdownError::Io(e.into()))?;
        }
    }

    Ok(files)
}
