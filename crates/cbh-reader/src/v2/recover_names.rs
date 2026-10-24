//! Retain complete referenced name slots verbatim, remapping only their ids.
use crate::{CbhError, source::SourceFile};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

const SLOTS: [u64; 6] = [1024, 1120, 220, 1024, 314, 532];
const GROUP: u64 = 4234;
const HEADER: u64 = 184;
const MAX_NAMES: usize = 250_000;

pub(super) struct Names {
    source: Option<SourceFile>,
    header: Option<Vec<u8>>,
    output: File,
    map: HashMap<(usize, u64), u64>,
    counts: [u32; 6],
}

impl Names {
    pub fn new(source: Option<&Path>, output: &Path) -> Result<Self, CbhError> {
        let source = source.map(SourceFile::open).transpose()?;
        let header = source
            .as_ref()
            .and_then(|s| s.read_exact_range(0, HEADER as usize).ok())
            .filter(|h| {
                u32::from_be_bytes(h[0..4].try_into().unwrap()) == 184
                    && u32::from_be_bytes(h[4..8].try_into().unwrap()) == 6
                    && (0..6).all(|i| {
                        let at = 8 + i * 20;
                        let kind = 136 + i * 8;
                        u32::from_be_bytes(h[at..at + 4].try_into().unwrap()) as u64 == SLOTS[i]
                            && h[at + 4..at + 8] == [0; 4]
                            && u32::from_be_bytes(h[kind..kind + 4].try_into().unwrap()) == i as u32
                            && h[kind + 4..kind + 8] == [0, 0, 0, 1]
                    })
            });
        Ok(Self {
            source,
            header,
            output: OpenOptions::new().write(true).read(true).open(output)?,
            map: HashMap::new(),
            counts: [0; 6],
        })
    }

    fn slot(&self, kind: usize, id: u64) -> Result<Vec<u8>, CbhError> {
        let bad = || {
            CbhError::Decode(format!(
                "Name list {kind}, id {id} is missing, truncated or invalid"
            ))
        };
        let header = self.header.as_ref().ok_or_else(bad)?;
        let at = 8 + kind * 20 + 8;
        if id >= u32::from_be_bytes(header[at..at + 4].try_into().unwrap()) as u64 {
            return Err(bad());
        }
        let offset = HEADER
            .checked_add(id.checked_mul(GROUP).ok_or_else(bad)?)
            .and_then(|p| p.checked_add(SLOTS[..kind].iter().sum()))
            .ok_or_else(bad)?;
        let source = self.source.as_ref().ok_or_else(bad)?;
        let b = source.read_exact_range(offset, SLOTS[kind] as usize)?;
        let len = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
        if len > b.len() - 4 {
            return Err(bad());
        }
        let payload = &b[4..4 + len];
        let mut at = 0usize;
        let fields = if kind == 4 { 1 } else { 2 };
        for _ in 0..fields {
            let size = payload.get(at..at + 4).ok_or_else(bad)?;
            let n = u32::from_le_bytes(size.try_into().unwrap()) as usize;
            at += 4;
            let value = payload
                .get(at..at.checked_add(n).ok_or_else(bad)?)
                .ok_or_else(bad)?;
            std::str::from_utf8(value).map_err(|_| bad())?;
            at += n;
        }
        let tail = match kind {
            0 => 24,
            1 => 108,
            2 => 12,
            4 => 5,
            _ => 0,
        };
        if payload.len().saturating_sub(at) < tail {
            return Err(bad());
        }
        Ok(b)
    }

    /// Validate all names before writing any of an entry's references.
    pub fn remap(&mut self, record: &mut [u8]) -> Result<(), CbhError> {
        let fields = [
            (24, 0),
            (32, 0),
            (40, 1),
            (48, 0),
            (56, 2),
            (64, 4),
            (72, 4),
        ];
        let mut pending = vec![];
        for (at, kind) in fields {
            let id = u64::from_le_bytes(record[at..at + 8].try_into().unwrap());
            if id != u64::MAX && !self.map.contains_key(&(kind, id)) {
                pending.push((kind, id, self.slot(kind, id)?));
            }
        }
        for (kind, id, bytes) in pending {
            if self.map.contains_key(&(kind, id)) {
                continue;
            }
            if self.map.len() >= MAX_NAMES {
                return Err(CbhError::LimitExceeded);
            }
            let new = self.counts[kind] as u64;
            self.counts[kind] += 1;
            let groups = *self.counts.iter().max().unwrap() as u64;
            self.output.set_len(HEADER + groups * GROUP)?;
            self.output.seek(SeekFrom::Start(
                HEADER + new * GROUP + SLOTS[..kind].iter().sum::<u64>(),
            ))?;
            self.output.write_all(&bytes)?;
            self.map.insert((kind, id), new);
        }
        for (at, kind) in fields {
            let id = u64::from_le_bytes(record[at..at + 8].try_into().unwrap());
            if id != u64::MAX {
                record[at..at + 8].copy_from_slice(&self.map[&(kind, id)].to_le_bytes());
            }
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<(), CbhError> {
        let mut header = vec![0; HEADER as usize];
        self.output.seek(SeekFrom::Start(0))?;
        self.output.read_exact(&mut header)?;
        for (kind, count) in self.counts.iter().enumerate() {
            header[16 + kind * 20..20 + kind * 20].copy_from_slice(&count.to_be_bytes());
        }
        self.output.seek(SeekFrom::Start(0))?;
        self.output.write_all(&header)?;
        self.output.sync_all()?;
        Ok(())
    }
}
