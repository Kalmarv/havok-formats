#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]
use crate::types::{Cursor, read_types, required};
use crate::{Compendium, Result, Type, invalid, sections};

/// SDK versions whose tagfile layout this reader knows: 2016.2 and 2018.1. Both store the same section tree and the
/// same item and patch tables; class layouts come from each file's own type metadata.
pub const SDK_VERSIONS: [[u8; 8]; 2] = [*b"20160200", *b"20180100"];

/// One entry of the tagfile's item table: `count` consecutive elements of one type at `offset` in DATA. Item 0 is
/// the null item; item 1 holds the root object.
#[derive(Debug, Clone, Copy)]
pub struct Item {
    /// Index into [`Tagfile::types`] (one-based; never 0 for an item after the null item).
    pub type_index: usize,
    /// The flags byte stored beside the type index, as stored.
    pub flags: u8,
    /// Byte offset of the first element in DATA.
    pub offset: usize,
    /// Number of elements.
    pub count: usize,
}

/// A parsed `TAG0` tagfile: its types, its item table and a borrow of its DATA section.
///
/// Objects are read lazily through [`Tagfile::root`], [`Tagfile::item`] and the [`crate::Value`] and [`crate::Array`] views they
/// return; nothing is decoded until asked for.
#[derive(Debug)]
pub struct Tagfile<'a> {
    /// The SDK version string as stored, e.g. `20160200`; one of [`SDK_VERSIONS`].
    pub version: [u8; 8],
    /// The type table. Index 0 is the null type; the file's own, or the compendium's.
    pub types: Vec<Type>,
    /// The item table, from the null item at index 0.
    pub items: Vec<Item>,
    /// The PTCH section: per type index, the DATA offsets of the stored item references of that type. Empty when the
    /// file has no PTCH section.
    pub patches: Vec<(usize, Vec<usize>)>,
    data: &'a [u8],
    data_offset: usize,
}

impl<'a> Tagfile<'a> {
    /// Parse and check the file's sections, types, item table and patch table. A file whose types are stored in an
    /// external compendium (`TCRF`) needs `compendium`; one that carries its own (`TYPE`) ignores it. Item extents
    /// and patch references are checked against DATA, so later reads cannot leave it.
    pub fn read(bytes: &'a [u8], compendium: Option<&Compendium>) -> Result<Self> {
        let roots = sections(bytes)?;
        let &[root] = roots.as_slice() else {
            return Err(invalid(0, "expected one TAG0 root"));
        };
        if root.signature != *b"TAG0" {
            return Err(invalid(0, "expected one TAG0 root"));
        }
        let children = root.children()?;
        for s in &children {
            if ![*b"SDKV", *b"DATA", *b"TYPE", *b"TCRF", *b"INDX"].contains(&s.signature) {
                return Err(invalid(s.offset, "unknown tagfile section"));
            }
        }
        let sdk = required(&children, &[*b"SDKV"])?;
        let version: [u8; 8] = sdk
            .payload
            .try_into()
            .map_err(|_| invalid(sdk.offset, "invalid SDK version"))?;
        if !SDK_VERSIONS.contains(&version) {
            return Err(invalid(sdk.offset, "unsupported SDK version"));
        }
        let ts = required(&children, &[*b"TYPE", *b"TCRF"])?;
        let types = if ts.signature == *b"TYPE" {
            read_types(ts)?
        } else {
            let mut c = Cursor::new(ts);
            let id: [u8; 8] = c.array()?;
            c.padding()?;
            let cmp = compendium.ok_or_else(|| invalid(ts.offset, "external compendium required"))?;
            if !cmp.ids.contains(&id) {
                return Err(invalid(ts.offset, "compendium ID mismatch"));
            }
            cmp.types.clone()
        };
        let data = required(&children, &[*b"DATA"])?;
        let indices = required(&children, &[*b"INDX"])?.children()?;
        for s in &indices {
            if ![*b"ITEM", *b"PTCH"].contains(&s.signature) {
                return Err(invalid(s.offset, "unknown index section"));
            }
        }
        let table = required(&indices, &[*b"ITEM"])?;
        if table.payload.len() % 12 != 0 {
            return Err(invalid(table.offset, "partial item record"));
        }
        let mut items = Vec::new();
        for &[t0, t1, t2, flags, o0, o1, o2, o3, n0, n1, n2, n3] in table.payload.as_chunks::<12>().0 {
            // a little-endian word: the type index in the low 24 bits, the flags in the top byte
            let type_index = u32::from_le_bytes([t0, t1, t2, 0]) as usize;
            if type_index >= types.len() {
                return Err(invalid(table.offset, "item type out of bounds"));
            }
            items.push(Item {
                type_index,
                flags,
                offset: u32::from_le_bytes([o0, o1, o2, o3]) as usize,
                count: u32::from_le_bytes([n0, n1, n2, n3]) as usize,
            });
        }
        // the null item first (type, count and offset 0), and at least the root after it
        let null = items.first().map(|i| (i.type_index, i.count, i.offset));
        if items.len() < 2 || null != Some((0, 0, 0)) {
            return Err(invalid(table.offset, "missing null/root item"));
        }
        let mut patches = Vec::new();
        let patch_sections = indices
            .iter()
            .filter(|s| s.signature == *b"PTCH")
            .collect::<Vec<_>>();
        if patch_sections.len() > 1 {
            return Err(invalid(table.offset, "duplicate patch section"));
        }
        if let Some(&ps) = patch_sections.first() {
            let mut c = Cursor::new(*ps);
            while c.position < c.bytes.len() {
                let ti = c.u32_le()? as usize;
                let count = c.u32_le()? as usize;
                if ti == 0 || ti >= types.len() {
                    return Err(c.error("patch type out of bounds"));
                }
                let mut offsets = Vec::new();
                for _ in 0..count {
                    let offset = c.u32_le()? as usize;
                    let index = data
                        .payload
                        .get(offset..)
                        .and_then(<[u8]>::first_chunk::<4>)
                        .map(|&b| u32::from_le_bytes(b) as usize)
                        .ok_or_else(|| c.error("patch exceeds DATA"))?;
                    if index >= items.len() {
                        return Err(c.error("patched item reference out of bounds"));
                    }
                    offsets.push(offset);
                }
                patches.push((ti, offsets));
            }
        }
        let result = Self {
            version,
            types,
            items,
            patches,
            data: data.payload,
            data_offset: data.offset.saturating_add(8),
        };
        for (i, item) in result.items.iter().enumerate().skip(1) {
            if item.type_index == 0 {
                return Err(invalid(table.offset, format!("null type in item {i}")));
            }
            let stride = result.stride(item.type_index)?;
            let size = stride
                .checked_mul(item.count)
                .ok_or_else(|| invalid(table.offset, "item length overflow"))?;
            result.bytes(item.offset, size)?;
        }
        Ok(result)
    }
    /// The first value `f` finds on type `index` or the types it derives from; the default (0) at the null type.
    pub fn inherited<T: Default>(&self, mut index: usize, f: impl Fn(&Type) -> Option<T>) -> Result<T> {
        for _ in 0..self.types.len() {
            if index == 0 {
                return Ok(T::default());
            }
            let t = self
                .types
                .get(index)
                .ok_or_else(|| invalid(0, "type index out of bounds"))?;
            if let Some(value) = f(t) {
                return Ok(value);
            }
            index = t.parent;
        }
        Err(invalid(0, "cyclic inherited property"))
    }
    /// The `format` word of a type or the nearest type it derives from (low four bits: the kind, e.g. 7 class, 6
    /// pointer, 8 array, 3 string; the rest per kind).
    pub fn format(&self, index: usize) -> Result<usize> {
        let format: u64 = self.inherited(index, |t| t.format)?;
        usize::try_from(format).map_err(|_| invalid(0, "type format exceeds the address space"))
    }
    /// The stored size in bytes of one element of the type (its own or the nearest parent's).
    pub fn stride(&self, index: usize) -> Result<usize> {
        // DATA retains the reflected element width, including references: an eight-byte
        // pointer stores its ITEM index in the low word and a zero high word. Advancing
        // four bytes makes the second array element read that padding as a null pointer.
        // HKLib's PointerFormatHandler and IndexItem read UInt64 per element (2018.1).
        self.inherited(index, |t| t.size)
    }
    /// `size` bytes of DATA at `offset`; an error naming the offset when they run past it.
    pub fn bytes(&self, offset: usize, size: usize) -> Result<&'a [u8]> {
        let end = offset
            .checked_add(size)
            .ok_or_else(|| invalid(self.data_offset.saturating_add(offset), "DATA range overflow"))?;
        self.data
            .get(offset..end)
            .ok_or_else(|| invalid(self.data_offset.saturating_add(offset), "object exceeds DATA"))
    }
    /// The `N` bytes at `offset` in DATA.
    pub(crate) fn array<const N: usize>(&self, offset: usize) -> Result<[u8; N]> {
        self.bytes(offset, N)?
            .try_into()
            .map_err(|_| invalid(self.data_offset.saturating_add(offset), "object exceeds DATA"))
    }
}
