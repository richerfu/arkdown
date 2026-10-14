use super::*;
use std::io::Cursor;

fn fixed_writer() -> RawZipWriter<Cursor<Vec<u8>>> {
    let mut writer = RawZipWriter::new(Cursor::new(Vec::new()));
    writer.modification_time = 1234;
    writer.modification_date = 5678;
    writer
}

#[test]
fn parallel_memory_and_spill_entries_match_sequential_zip_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let mut random = 17u32;
    let large: Vec<_> = (0..3 * COMPRESSED_BLOCK_SIZE)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            random as u8
        })
        .collect();
    let inputs = [
        large.as_slice(),
        b"small repeated data small repeated data",
        b"",
        &large[..123],
    ];
    let entries: Vec<_> = (0..24)
        .map(|index| {
            let name = format!("libs/{index:02}.so");
            let path = directory.path().join(format!("{index}.so"));
            std::fs::write(&path, inputs[index % inputs.len()]).unwrap();
            (name, path)
        })
        .collect();
    let mut sequential = fixed_writer();
    for (name, path) in &entries {
        sequential
            .add_file_deflate(name, path, flate2::Compression::new(1))
            .unwrap();
    }
    let expected = sequential.finish().unwrap().into_inner();
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for budget in [0, COMPRESSED_BLOCK_SIZE, COMPRESSED_MEMORY_BUDGET] {
            let batch = ParallelDeflatedFiles {
                available: AtomicUsize::new(budget),
            };
            let mut writer = fixed_writer();
            pool.install(|| batch.write(&mut writer, entries.clone(), 1))
                .unwrap();
            assert_eq!(writer.finish().unwrap().into_inner(), expected);
            assert_eq!(batch.available.load(Ordering::Relaxed), budget);
        }
    }
}

#[test]
fn compressed_payload_budget_covers_running_and_waiting_entries() {
    let budget = AtomicUsize::new(2 * COMPRESSED_BLOCK_SIZE);
    let mut first = CompressedOutput::new(&budget);
    first
        .write_all(&vec![1; COMPRESSED_BLOCK_SIZE + 1])
        .unwrap();
    assert_eq!(budget.load(Ordering::Relaxed), 0);
    let mut second = CompressedOutput::new(&budget);
    second.write_all(b"waiting entry").unwrap();
    let spill = second.spill.as_ref().unwrap().path().to_owned();
    assert!(spill.exists());
    assert!(second.blocks.is_empty());
    first.write_all(&vec![2; COMPRESSED_BLOCK_SIZE]).unwrap();
    assert!(first.blocks.is_empty());
    assert!(first.spill.is_some());
    assert_eq!(budget.load(Ordering::Relaxed), 2 * COMPRESSED_BLOCK_SIZE);
    let mut second = second.finish();
    assert!(matches!(second, CompressedPayload::File { .. }));
    let mut actual = Vec::new();
    second.copy_to(&mut actual).unwrap();
    assert_eq!(actual, b"waiting entry");
    drop(second);
    assert!(!spill.exists());
}

#[test]
fn worker_errors_are_reported_in_entry_order_without_stranding_writer() {
    let directory = tempfile::tempdir().unwrap();
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let entries = ["first-missing", "second-missing"]
            .map(|name| (name.into(), directory.path().join(name)))
            .to_vec();
        let mut writer = fixed_writer();
        let error = pool
            .install(|| writer.add_files_deflated(entries, 1))
            .unwrap_err();
        assert!(error.to_string().contains("first-missing"), "{error}");
    }
}
