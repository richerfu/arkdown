//! Executable-page bitmap generation for HAP code signing.
//!
//! This follows `developtools_hapsigner`'s
//! `PageInfoGenerator`/`SignProvider.copyFileAndAlignment` contract. Each HAP
//! page has four bitmap bits; bit 0 marks executable ELF data and bit 1 marks
//! Ark bytecode. The bitmap capacity ends at the first non-runnable entry.

use object::{Object, ObjectSegment, SegmentFlags};

use crate::{
    error::SignError,
    zip::{HapZip, ALIGNMENT_RUNNABLE},
};

const BITS_PER_PAGE: usize = 4;
const ELF_EXECUTABLE_BIT: usize = 0;
const ABC_EXECUTABLE_BIT: usize = 1;

#[derive(Debug, Clone, Copy)]
enum ExecutableKind {
    Elf,
    Abc,
}

#[derive(Debug, Clone, Copy)]
struct ExecutableSegment {
    kind: ExecutableKind,
    start: usize,
    end: usize,
}

pub(crate) struct PageInfoGenerator {
    max_entry_data_offset: usize,
    segments: Vec<ExecutableSegment>,
}

impl PageInfoGenerator {
    pub(crate) fn new(hap_zip: &HapZip) -> Result<Self, SignError> {
        let mut generator = Self {
            max_entry_data_offset: 0,
            segments: Vec::new(),
        };

        for (entry_index, entry) in hap_zip.entries.iter().enumerate() {
            let data_offset = hap_zip.entry_data_offset(entry_index)?;
            if data_offset % ALIGNMENT_RUNNABLE != 0 {
                return Err(SignError::InvalidZip(
                    "code-signing entry data is not 4096-byte aligned",
                ));
            }
            if !entry.is_runnable || entry.method != 0 {
                generator.max_entry_data_offset = data_offset;
                break;
            }

            let data = hap_zip.read_uncompressed_entry(&entry.name)?;
            if entry.name.ends_with(".abc") {
                generator.push_segment(ExecutableKind::Abc, data_offset, 0, data.len())?;
            } else {
                generator.collect_elf_segments(&entry.name, data_offset, &data)?;
            }
        }

        Ok(generator)
    }

    pub(crate) fn generate_bitmap(&self) -> Result<Vec<u8>, SignError> {
        if self.segments.is_empty() {
            return Ok(Vec::new());
        }
        if !self
            .max_entry_data_offset
            .is_multiple_of(ALIGNMENT_RUNNABLE)
        {
            return Err(SignError::InvalidZip(
                "page-info boundary is not 4096-byte aligned",
            ));
        }

        let initial_bits = self
            .max_entry_data_offset
            .checked_div(ALIGNMENT_RUNNABLE)
            .and_then(|pages| pages.checked_mul(BITS_PER_PAGE))
            .ok_or_else(|| SignError::SigningFailed("page-info bitmap size overflow".into()))?;
        let highest_bit = self
            .segments
            .iter()
            .filter_map(|segment| {
                let page = segment.end.saturating_sub(1) / ALIGNMENT_RUNNABLE;
                page.checked_mul(BITS_PER_PAGE).and_then(|bit| {
                    bit.checked_add(match segment.kind {
                        ExecutableKind::Elf => ELF_EXECUTABLE_BIT,
                        ExecutableKind::Abc => ABC_EXECUTABLE_BIT,
                    })
                })
            })
            .max()
            .and_then(|bit| bit.checked_add(1))
            .unwrap_or(0);
        let bit_capacity = initial_bits.max(highest_bit);
        let word_count = bit_capacity
            .checked_add(63)
            .ok_or_else(|| SignError::SigningFailed("page-info bitmap size overflow".into()))?
            / 64;
        let mut bitmap = vec![0u8; word_count * 8];

        for segment in &self.segments {
            let start_page = segment.start / ALIGNMENT_RUNNABLE;
            let end_page = segment
                .end
                .checked_add(ALIGNMENT_RUNNABLE - 1)
                .ok_or_else(|| SignError::SigningFailed("page-info segment overflow".into()))?
                / ALIGNMENT_RUNNABLE;
            let kind_bit = match segment.kind {
                ExecutableKind::Elf => ELF_EXECUTABLE_BIT,
                ExecutableKind::Abc => ABC_EXECUTABLE_BIT,
            };
            for page in start_page..end_page {
                let bit = page
                    .checked_mul(BITS_PER_PAGE)
                    .and_then(|bit| bit.checked_add(kind_bit))
                    .ok_or_else(|| SignError::SigningFailed("page-info bit overflow".into()))?;
                bitmap[bit / 8] |= 1 << (bit % 8);
            }
        }

        Ok(bitmap)
    }

    fn collect_elf_segments(
        &mut self,
        entry_name: &str,
        data_offset: usize,
        data: &[u8],
    ) -> Result<(), SignError> {
        if !data.starts_with(b"\x7fELF") {
            return Ok(());
        }
        let file = object::File::parse(data).map_err(|error| {
            SignError::SigningFailed(format!(
                "failed to parse ELF runnable entry '{entry_name}': {error}"
            ))
        })?;
        for segment in file.segments() {
            let SegmentFlags::Elf { p_flags } = segment.flags() else {
                continue;
            };
            if p_flags & object::elf::PF_X == 0 {
                continue;
            }
            let (segment_offset, segment_size) = segment.file_range();
            let segment_offset = usize::try_from(segment_offset).map_err(|_| {
                SignError::SigningFailed(format!(
                    "ELF segment offset exceeds platform limits in '{entry_name}'"
                ))
            })?;
            let segment_size = usize::try_from(segment_size).map_err(|_| {
                SignError::SigningFailed(format!(
                    "ELF segment size exceeds platform limits in '{entry_name}'"
                ))
            })?;
            self.push_segment(
                ExecutableKind::Elf,
                data_offset,
                segment_offset,
                segment_size,
            )?;
        }
        Ok(())
    }

    fn push_segment(
        &mut self,
        kind: ExecutableKind,
        data_offset: usize,
        relative_offset: usize,
        size: usize,
    ) -> Result<(), SignError> {
        let start = data_offset
            .checked_add(relative_offset)
            .ok_or_else(|| SignError::SigningFailed("executable segment offset overflow".into()))?;
        let end = start
            .checked_add(size)
            .ok_or_else(|| SignError::SigningFailed("executable segment size overflow".into()))?;
        self.segments.push(ExecutableSegment { kind, start, end });
        Ok(())
    }
}
