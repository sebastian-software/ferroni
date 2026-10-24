//! Test-only wire checksum repair, so mutations reach structural validators.

pub fn word(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize)
}

pub fn repair_checksums(bytes: &mut [u8]) {
    let Some(count) = word(bytes, 13).filter(|&count| count <= 100_000) else {
        return;
    };
    let mut at = 17usize;
    for _ in 0..count {
        let Some(keys) = word(bytes, at).filter(|&count| count <= 65_535) else {
            return;
        };
        let Some(checksum_at) = keys
            .checked_mul(4)
            .and_then(|n| at.checked_add(4)?.checked_add(n))
        else {
            return;
        };
        let Some(len) = word(bytes, checksum_at + 4) else {
            return;
        };
        let Some(end) = (checksum_at + 8).checked_add(len) else {
            return;
        };
        let Some(record) = bytes.get(checksum_at + 8..end) else {
            return;
        };
        let checksum = checksum(record);
        bytes[checksum_at..checksum_at + 4].copy_from_slice(&checksum.to_le_bytes());
        at = end;
    }
}

// FPRT002/FPRT003 four-lane checksum. Keep independent of the loader so
// decoder fuzzing can repair checksums without exporting implementation APIs.
fn checksum(bytes: &[u8]) -> u32 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut lanes = [
        0xcbf2_9ce4_8422_2325u64,
        0x9e37_79b1_85eb_ca87,
        0xc2b2_ae3d_27d4_eb4f,
        0x1656_67b1_9e37_79f9,
    ];
    let (blocks, tail) = bytes.as_chunks::<32>();
    for block in blocks {
        for (lane, word) in lanes.iter_mut().zip(block.as_chunks::<8>().0) {
            *lane = (*lane ^ u64::from_le_bytes(*word)).wrapping_mul(PRIME);
            *lane ^= *lane >> 29;
        }
    }
    let mut hash = lanes[0]
        ^ lanes[1].rotate_left(13)
        ^ lanes[2].rotate_left(31)
        ^ lanes[3].rotate_left(47)
        ^ bytes.len() as u64;
    let (words, tail) = tail.as_chunks::<8>();
    for word in words {
        hash = (hash ^ u64::from_le_bytes(*word)).wrapping_mul(PRIME);
        hash ^= hash >> 29;
    }
    for &byte in tail {
        hash = (hash ^ u64::from(byte)).wrapping_mul(PRIME);
    }
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 33;
    (hash ^ (hash >> 32)) as u32
}
