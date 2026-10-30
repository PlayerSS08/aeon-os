//! ACPI RSDP validation on byte slices (pure, host-testable).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rsdp {
    /// Signature or checksum is wrong; do not trust the pointer.
    Invalid,
    /// ACPI 1.0 (revision 0): only the RSDT address is meaningful.
    V1 { rsdt: u32 },
    /// ACPI 2.0+: the extended checksum covers the length stated in the table.
    V2 { xsdt: u64, revision: u8 },
}

pub const V1_LEN: usize = 20;
pub const V2_MIN_LEN: usize = 36;
/// Upper bound on the `length` field we accept before summing, so a corrupt table cannot make us
/// read an arbitrary amount of memory.
pub const V2_MAX_LEN: usize = 4096;

fn checksum(b: &[u8]) -> u8 {
    b.iter().fold(0u8, |a, &x| a.wrapping_add(x))
}

/// Header fields needed to decide how many bytes may be read: `(revision, length)`.
/// `bytes` must hold at least the first 24 bytes of the RSDP.
pub fn peek_header(bytes: &[u8]) -> Option<(u8, usize)> {
    if bytes.len() < 24 || &bytes[0..8] != b"RSD PTR " {
        return None;
    }
    let revision = bytes[15];
    let len = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
    Some((revision, len))
}

/// Validates the RSDP in `bytes` (which must cover the whole table).
pub fn validate(bytes: &[u8]) -> Rsdp {
    let Some((revision, len)) = peek_header(bytes) else {
        return Rsdp::Invalid;
    };
    if bytes.len() < V1_LEN || checksum(&bytes[..V1_LEN]) != 0 {
        return Rsdp::Invalid;
    }
    if revision == 0 {
        let rsdt = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        return Rsdp::V1 { rsdt };
    }
    if !(V2_MIN_LEN..=V2_MAX_LEN).contains(&len) || bytes.len() < len || checksum(&bytes[..len]) != 0 {
        return Rsdp::Invalid;
    }
    let mut x = [0u8; 8];
    x.copy_from_slice(&bytes[24..32]);
    Rsdp::V2 { xsdt: u64::from_le_bytes(x), revision }
}

/// ACPI system description table header size.
pub const SDT_HEADER: usize = 36;
/// Largest table we will read (a DSDT can be a few hundred KiB).
pub const SDT_MAX_LEN: usize = 1 << 20;

/// Table length from an SDT header, if it is plausible.
pub fn sdt_len(header: &[u8]) -> Option<usize> {
    let len = u32::from_le_bytes(header.get(4..8)?.try_into().ok()?) as usize;
    (SDT_HEADER..=SDT_MAX_LEN).contains(&len).then_some(len)
}

pub fn sdt_checksum_ok(table: &[u8]) -> bool {
    checksum(table) == 0
}

/// Physical addresses of the tables listed by an RSDT (4-byte entries) or XSDT (8-byte entries).
pub fn sdt_entries(table: &[u8], xsdt: bool) -> impl Iterator<Item = u64> + '_ {
    let w = if xsdt { 8 } else { 4 };
    table.get(SDT_HEADER..).unwrap_or(&[]).chunks_exact(w).map(move |c| {
        let mut b = [0u8; 8];
        b[..w].copy_from_slice(c);
        u64::from_le_bytes(b)
    })
}

/// What the FADT tells us about powering off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fadt {
    pub dsdt: u64,
    pub pm1a_cnt: u16,
    pub pm1b_cnt: u16,
}

/// Parses a FADT ("FACP"). Port numbers above 0xFFFF (memory-mapped registers) are not supported
/// and yield `None`.
pub fn parse_fadt(t: &[u8]) -> Option<Fadt> {
    if t.get(0..4)? != b"FACP" || t.len() < 72 {
        return None;
    }
    let rd32 = |o: usize| -> Option<u32> { Some(u32::from_le_bytes(t.get(o..o + 4)?.try_into().ok()?)) };
    let mut dsdt = rd32(40)? as u64;
    if t.len() >= 148 {
        let x = u64::from_le_bytes(t.get(140..148)?.try_into().ok()?);
        if x != 0 {
            dsdt = x;
        }
    }
    let pm1a = rd32(64)?;
    let pm1b = rd32(68)?;
    if pm1a == 0 || pm1a > 0xFFFF || pm1b > 0xFFFF {
        return None;
    }
    Some(Fadt { dsdt, pm1a_cnt: pm1a as u16, pm1b_cnt: pm1b as u16 })
}

/// Finds the `_S5_` sleep-type values `(SLP_TYPa, SLP_TYPb)` in a DSDT by locating the
/// `Name(_S5_, Package(){...})` object, the same shortcut other small kernels use instead of a
/// full AML interpreter.
pub fn find_s5(dsdt: &[u8]) -> Option<(u8, u8)> {
    let pos = dsdt.windows(4).position(|w| w == b"_S5_")?;
    // Name(_S5_ ...) is `08 5F 53 35 5F 12 <PkgLength> <NumElements> <elements>`; the name may also
    // be prefixed with `\` (0x5C).
    if pos == 0 || !matches!(dsdt[pos - 1], 0x08 | 0x5C) {
        return None;
    }
    let mut i = pos + 4;
    if *dsdt.get(i)? != 0x12 {
        return None;
    }
    i += 1;
    let extra = (*dsdt.get(i)? >> 6) as usize; // PkgLength: top two bits count the extra bytes
    i += 1 + extra;
    i += 1; // NumElements
    let mut vals = [0u8; 2];
    for v in vals.iter_mut() {
        if *dsdt.get(i)? == 0x0A {
            i += 1; // BytePrefix
        }
        *v = *dsdt.get(i)?;
        i += 1;
    }
    Some((vals[0], vals[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(buf: &mut [u8], at: usize, upto: usize) {
        buf[at] = 0;
        let s = checksum(&buf[..upto]);
        buf[at] = 0u8.wrapping_sub(s);
    }

    fn v2(len: u32) -> std::vec::Vec<u8> {
        let mut b = std::vec![0u8; len as usize];
        b[0..8].copy_from_slice(b"RSD PTR ");
        b[15] = 2;
        b[20..24].copy_from_slice(&len.to_le_bytes());
        b[24..32].copy_from_slice(&0x3ED7_D0E8u64.to_le_bytes());
        fix(&mut b, 8, 20);
        fix(&mut b, 32, len as usize);
        b
    }

    #[test]
    fn valid_v2_is_accepted() {
        assert_eq!(validate(&v2(36)), Rsdp::V2 { xsdt: 0x3ED7_D0E8, revision: 2 });
    }

    #[test]
    fn extended_checksum_covers_declared_length() {
        let mut b = v2(48);
        b[44] ^= 1; // beyond byte 36: the old 36-byte check would have missed this
        assert_eq!(validate(&b), Rsdp::Invalid);
    }

    #[test]
    fn bad_first_checksum_is_rejected() {
        let mut b = v2(36);
        b[9] ^= 0x10;
        assert_eq!(validate(&b), Rsdp::Invalid);
    }

    #[test]
    fn wrong_signature_is_rejected() {
        let mut b = v2(36);
        b[0] = b'X';
        assert_eq!(validate(&b), Rsdp::Invalid);
    }

    #[test]
    fn absurd_length_is_rejected_without_reading() {
        let mut b = v2(36);
        b[20..24].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
        assert_eq!(validate(&b), Rsdp::Invalid);
    }

    #[test]
    fn v1_table() {
        let mut b = std::vec![0u8; 20];
        b[0..8].copy_from_slice(b"RSD PTR ");
        b[16..20].copy_from_slice(&0x1234_5000u32.to_le_bytes());
        // peek_header wants 24 bytes
        b.resize(24, 0);
        fix(&mut b, 8, 20);
        assert_eq!(validate(&b), Rsdp::V1 { rsdt: 0x1234_5000 });
    }

    #[test]
    fn fadt_fields() {
        let mut t = std::vec![0u8; 244];
        t[0..4].copy_from_slice(b"FACP");
        t[4..8].copy_from_slice(&244u32.to_le_bytes());
        t[40..44].copy_from_slice(&0x3EDF_0000u32.to_le_bytes());
        t[64..68].copy_from_slice(&0x4004u32.to_le_bytes());
        assert_eq!(parse_fadt(&t), Some(Fadt { dsdt: 0x3EDF_0000, pm1a_cnt: 0x4004, pm1b_cnt: 0 }));
        t[140..148].copy_from_slice(&0x1_0000_0000u64.to_le_bytes());
        assert_eq!(parse_fadt(&t).unwrap().dsdt, 0x1_0000_0000);
        t[64..68].copy_from_slice(&0x1_0000u32.to_le_bytes()); // memory-mapped: unsupported
        assert_eq!(parse_fadt(&t), None);
    }

    #[test]
    fn s5_object_is_found() {
        // ...Name(_S5_, Package(4){ 0x05, 0x05, 0, 0 })...
        let aml = [0x10, 0x20, 0x08, b'_', b'S', b'5', b'_', 0x12, 0x06, 0x04, 0x0A, 0x05, 0x0A, 0x05, 0x00, 0x00];
        assert_eq!(find_s5(&aml), Some((5, 5)));
        // Zero/One ops instead of BytePrefix constants.
        let aml2 = [0x08, b'_', b'S', b'5', b'_', 0x12, 0x04, 0x02, 0x00, 0x01];
        assert_eq!(find_s5(&aml2), Some((0, 1)));
        assert_eq!(find_s5(b"no sleep object here"), None);
        assert_eq!(find_s5(&[0x08, b'_', b'S', b'5', b'_', 0x12]), None);
    }

    #[test]
    fn sdt_length_is_bounded() {
        let mut h = [0u8; 36];
        h[4..8].copy_from_slice(&100u32.to_le_bytes());
        assert_eq!(sdt_len(&h), Some(100));
        h[4..8].copy_from_slice(&10u32.to_le_bytes());
        assert_eq!(sdt_len(&h), None);
        h[4..8].copy_from_slice(&0x4000_0000u32.to_le_bytes());
        assert_eq!(sdt_len(&h), None);
    }

    #[test]
    fn sdt_entry_widths() {
        let mut t = std::vec![0u8; 36];
        t.extend_from_slice(&0x1111u64.to_le_bytes());
        t.extend_from_slice(&0x2222u64.to_le_bytes());
        assert_eq!(sdt_entries(&t, true).collect::<std::vec::Vec<_>>(), [0x1111, 0x2222]);
        assert_eq!(sdt_entries(&t, false).count(), 4);
    }
}
