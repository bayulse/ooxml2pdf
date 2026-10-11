//! Narrow ZIP adapter: single-disk ZIP32, stored/DEFLATE, no extraction.
use crate::{PackageError, Result, SecurityLimits, limit, normalize_part_path};
use flate2::{Decompress, FlushDecompress, Status};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: &str) -> PackageError {
    PackageError::InvalidZip(message.into())
}
fn slice(bytes: &[u8], start: usize, len: usize) -> Result<&[u8]> {
    bytes
        .get(
            start
                ..start
                    .checked_add(len)
                    .ok_or_else(|| invalid("Offset overflow"))?,
        )
        .ok_or_else(|| invalid("Truncated ZIP record"))
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    let b = slice(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    let b = slice(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn extra(bytes: &[u8]) -> Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let kind = u16_at(bytes, offset)?;
        let size = usize::from(u16_at(bytes, offset + 2)?);
        slice(bytes, offset + 4, size)?;
        if kind == 1 {
            return Err(PackageError::Unsupported("ZIP64".into()));
        }
        // Alternate Unicode names would introduce two identities for one part.
        if kind == 0x7075 {
            return Err(PackageError::Unsupported(
                "ZIP alternate Unicode path field".into(),
            ));
        }
        offset += 4 + size;
    }
    Ok(())
}

struct Entry {
    name: String,
    directory: bool,
    data_start: usize,
    compressed: usize,
    size: u64,
    crc: u32,
    method: u16,
}

pub(crate) fn read(bytes: &[u8], limits: SecurityLimits) -> Result<BTreeMap<String, Vec<u8>>> {
    if bytes.len() as u64 > limits.max_input_size {
        return Err(limit("max_input_size"));
    }
    if bytes.len() < 22 {
        return Err(invalid("Missing end-of-central-directory"));
    }
    let end = (bytes.len().saturating_sub(22 + 65535)..=bytes.len() - 22)
        .rev()
        .find(|&offset| {
            bytes.get(offset..offset + 4) == Some(b"PK\x05\x06")
                && u16_at(bytes, offset + 20)
                    .is_ok_and(|len| offset + 22 + usize::from(len) == bytes.len())
        })
        .ok_or_else(|| invalid("Missing end-of-central-directory"))?;
    if u16_at(bytes, end + 4)? != 0
        || u16_at(bytes, end + 6)? != 0
        || u16_at(bytes, end + 8)? != u16_at(bytes, end + 10)?
    {
        return Err(PackageError::Unsupported("Multi-disk ZIP".into()));
    }
    let count = usize::from(u16_at(bytes, end + 10)?);
    let directory_size = u32_at(bytes, end + 12)? as usize;
    let directory_start = u32_at(bytes, end + 16)? as usize;
    if count == 65535 || directory_size == u32::MAX as usize || directory_start == u32::MAX as usize
    {
        return Err(PackageError::Unsupported("ZIP64".into()));
    }
    if count > limits.max_zip_entries {
        return Err(limit("max_zip_entries"));
    }
    if directory_start.checked_add(directory_size) != Some(end) {
        return Err(invalid("Invalid central directory bounds"));
    }
    let directory = slice(bytes, directory_start, directory_size)?;
    let mut entries = Vec::new();
    let mut names = BTreeSet::new();
    let mut ranges = Vec::new();
    let mut declared_total = 0u64;
    let mut offset = 0;
    for _ in 0..count {
        slice(directory, offset, 46)?;
        if slice(directory, offset, 4)? != b"PK\x01\x02" {
            return Err(invalid("Invalid central header"));
        }
        let flags = u16_at(directory, offset + 8)?;
        let method = u16_at(directory, offset + 10)?;
        if flags & !0x080e != 0 {
            return Err(PackageError::Unsupported(
                "Encrypted or unsupported ZIP flags".into(),
            ));
        }
        if method != 0 && method != 8 {
            return Err(PackageError::Unsupported(format!(
                "ZIP compression method {method}"
            )));
        }
        let crc = u32_at(directory, offset + 16)?;
        let compressed = u32_at(directory, offset + 20)? as usize;
        let size = u64::from(u32_at(directory, offset + 24)?);
        let name_len = usize::from(u16_at(directory, offset + 28)?);
        if name_len > limits.max_path_length {
            return Err(limit("max_path_length"));
        }
        let extra_len = usize::from(u16_at(directory, offset + 30)?);
        let comment_len = usize::from(u16_at(directory, offset + 32)?);
        let disk = u16_at(directory, offset + 34)?;
        let attributes = u32_at(directory, offset + 38)?;
        let local = u32_at(directory, offset + 42)? as usize;
        if compressed == u32::MAX as usize
            || size == u64::from(u32::MAX)
            || local == u32::MAX as usize
        {
            return Err(PackageError::Unsupported("ZIP64".into()));
        }
        if disk != 0 {
            return Err(PackageError::Unsupported("Multi-disk ZIP".into()));
        }
        if attributes >> 16 & 0xf000 == 0xa000 {
            return Err(PackageError::Unsupported("ZIP symlink".into()));
        }
        let raw_name = slice(directory, offset + 46, name_len)?;
        let name = std::str::from_utf8(raw_name)
            .map_err(|_| PackageError::Unsupported("Non-UTF-8 ZIP name".into()))?;
        let is_directory = name.ends_with('/');
        let name = normalize_part_path(if is_directory {
            &name[..name.len() - 1]
        } else {
            name
        })?;
        if !names.insert(name.clone()) {
            return Err(PackageError::DuplicatePart(name));
        }
        extra(slice(directory, offset + 46 + name_len, extra_len)?)?;
        slice(directory, offset + 46 + name_len + extra_len, comment_len)?;
        offset += 46 + name_len + extra_len + comment_len;
        if size > limits.max_part_size {
            return Err(limit("max_part_size"));
        }
        if (name == "[Content_Types].xml" || name.ends_with(".rels")) && size > limits.max_xml_size
        {
            return Err(limit("max_xml_size"));
        }
        if size > (compressed as u64).saturating_mul(limits.max_compression_ratio) {
            return Err(limit("max_compression_ratio"));
        }
        declared_total = declared_total
            .checked_add(size)
            .ok_or_else(|| limit("max_total_uncompressed_size"))?;
        if declared_total > limits.max_total_uncompressed_size {
            return Err(limit("max_total_uncompressed_size"));
        }
        if is_directory && size != 0 {
            return Err(invalid("Nonempty directory member"));
        }
        slice(bytes, local, 30)?;
        if slice(bytes, local, 4)? != b"PK\x03\x04" {
            return Err(invalid("Invalid local header"));
        }
        if u16_at(bytes, local + 6)? != flags || u16_at(bytes, local + 8)? != method {
            return Err(invalid("Local/central header mismatch"));
        }
        let local_name = usize::from(u16_at(bytes, local + 26)?);
        let local_extra = usize::from(u16_at(bytes, local + 28)?);
        if slice(bytes, local + 30, local_name)? != raw_name {
            return Err(invalid("Local/central name mismatch"));
        }
        extra(slice(bytes, local + 30 + local_name, local_extra)?)?;
        let data_start = local + 30 + local_name + local_extra;
        let data_end = data_start
            .checked_add(compressed)
            .ok_or_else(|| invalid("Offset overflow"))?;
        let mut record_end = data_end;
        for (field, expected) in [(14, crc), (18, compressed as u32), (22, size as u32)] {
            let value = u32_at(bytes, local + field)?;
            if value != expected && !(flags & 8 != 0 && value == 0) {
                return Err(invalid("Local/central size or CRC mismatch"));
            }
        }
        if flags & 8 != 0 {
            let descriptor = if slice(bytes, data_end, 4)? == b"PK\x07\x08" {
                data_end + 4
            } else {
                data_end
            };
            slice(bytes, descriptor, 12)?;
            if u32_at(bytes, descriptor)? != crc
                || u32_at(bytes, descriptor + 4)? != compressed as u32
                || u32_at(bytes, descriptor + 8)? != size as u32
            {
                return Err(invalid("Invalid data descriptor"));
            }
            record_end = descriptor + 12;
        }
        if record_end > directory_start {
            return Err(invalid("Member overlaps central directory"));
        }
        ranges.push((local, record_end));
        entries.push(Entry {
            name,
            directory: is_directory,
            data_start,
            compressed,
            size,
            crc,
            method,
        });
    }
    if offset != directory.len() {
        return Err(invalid("Central directory count mismatch"));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(invalid("Overlapping ZIP members"));
    }
    let mut parts = BTreeMap::new();
    let mut actual_total = 0u64;
    for entry in entries {
        let compressed = slice(bytes, entry.data_start, entry.compressed)?;
        let output = if entry.method == 0 {
            if entry.size != compressed.len() as u64 {
                return Err(invalid("Stored size mismatch"));
            }
            compressed.to_vec()
        } else {
            let mut decoder = Decompress::new(false);
            let mut output = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let before_in = decoder.total_in();
                let before_out = decoder.total_out();
                let available = (entry.size + 1 - before_out).min(buffer.len() as u64) as usize;
                if available == 0 {
                    return Err(invalid("DEFLATE exceeds declared size"));
                }
                let status = decoder
                    .decompress(
                        &compressed[before_in as usize..],
                        &mut buffer[..available],
                        FlushDecompress::None,
                    )
                    .map_err(|_| invalid("Malformed DEFLATE stream"))?;
                let produced = (decoder.total_out() - before_out) as usize;
                output.extend_from_slice(&buffer[..produced]);
                if decoder.total_out() > entry.size {
                    return Err(invalid("DEFLATE exceeds declared size"));
                }
                if status == Status::StreamEnd {
                    if decoder.total_in() != compressed.len() as u64 {
                        return Err(invalid("Trailing DEFLATE data"));
                    }
                    break;
                }
                if decoder.total_in() == before_in && decoder.total_out() == before_out {
                    return Err(invalid("Incomplete DEFLATE stream"));
                }
            }
            output
        };
        if output.len() as u64 != entry.size {
            return Err(invalid("Uncompressed size mismatch"));
        }
        if crc32fast::hash(&output) != entry.crc {
            return Err(invalid("CRC mismatch"));
        }
        actual_total = actual_total
            .checked_add(output.len() as u64)
            .ok_or_else(|| limit("max_total_uncompressed_size"))?;
        if actual_total > limits.max_total_uncompressed_size {
            return Err(limit("max_total_uncompressed_size"));
        }
        if !entry.directory {
            parts.insert(entry.name, output);
        }
    }
    Ok(parts)
}
