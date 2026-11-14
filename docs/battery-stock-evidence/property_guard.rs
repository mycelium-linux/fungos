//! Offline evidence guard; not linked to the HAL or executed on the phone.
//! rustc --edition=2021 --test docs/battery-stock-evidence/property_guard.rs -o /tmp/property-guard-tests
//! rustc --edition=2021 docs/battery-stock-evidence/property_guard.rs -o /tmp/property-guard
//! /tmp/property-guard /path/to/retained/vmlinux.bin
use std::{env, fs};
const BASE: u32 = 0xc0008000;
const ATTRS: u32 = 0xc15e3874;
const STRIDE: u32 = 16;
fn word(bytes: &[u8], address: u32) -> Result<u32, String> {
    let offset = address.checked_sub(BASE).ok_or("address below base")? as usize;
    let value = bytes.get(offset..offset + 4).ok_or("word outside image")?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}
fn verify(bytes: &[u8]) -> Result<(), String> {
    // Read names from the fitted table, never from a reference header's enum.
    for (index, expected, target, load) in [
        (14, "voltage_ocv", 0xc0a7f134, 0xe5950068),
        (17, "current_now", 0xc0a7f13c, 0xe5950030),
        (44, "capacity", 0xc0a7f194, 0xe595049c),
    ] {
        let pointer = word(bytes, ATTRS + STRIDE * index)?;
        let offset = pointer.checked_sub(BASE).ok_or("name below base")? as usize;
        let name = bytes
            .get(offset..offset + expected.len() + 1)
            .ok_or("name outside image")?;
        if name != [expected.as_bytes(), &[0]].concat() {
            return Err(format!("fitted attribute {index}: expected {expected}"));
        }
        if word(bytes, 0xc0a7ef98 + 4 * index)? != target || word(bytes, target)? != load {
            return Err(format!("getter dispatch/load mismatch for {expected}"));
        }
    }
    // Exact ARM instructions at the setter dispatch and stores, not guessed labels.
    for (address, expected) in [
        (0xc0a7f398, 0xe357000e), // compare property14
        (0xc0a7f39c, 0x0a00000f), // property14 branch -> OCV setter
        (0xc0a7f3a0, 0xe3570011), // compare property17
        (0xc0a7f3a4, 0x1a000032), // non-property17 -> unsupported
        (0xc0a7f3ac, 0xe5850030), // CURRENT_NOW store
        (0xc0a7f3fc, 0xe5850068), // VOLTAGE_OCV store
    ] {
        if word(bytes, address)? != expected {
            return Err(format!("setter instruction mismatch at {address:#x}"));
        }
    }
    Ok(())
}
fn main() {
    let path = env::args()
        .nth(1)
        .expect("usage: property-guard vmlinux.bin");
    let bytes = fs::read(path).expect("read retained image");
    verify(&bytes).expect("stock property evidence mismatch");
    println!("PASS: fitted 16-byte attributes + getter dispatch/loads + setter stores");
}
#[cfg(test)]
mod tests {
    use super::*;
    fn put(b: &mut [u8], a: u32, v: u32) {
        let o = (a - BASE) as usize;
        b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; (ATTRS - BASE + 16 * 50) as usize];
        for (i, name, target, load) in [
            (14, "voltage_ocv", 0xc0a7f134, 0xe5950068),
            (17, "current_now", 0xc0a7f13c, 0xe5950030),
            (44, "capacity", 0xc0a7f194, 0xe595049c),
        ] {
            let p = BASE + 0x100 + 32 * i;
            put(&mut b, ATTRS + STRIDE * i, p);
            let o = (p - BASE) as usize;
            b[o..o + name.len()].copy_from_slice(name.as_bytes());
            put(&mut b, 0xc0a7ef98 + 4 * i, target);
            put(&mut b, target, load);
        }
        for (a, v) in [
            (0xc0a7f398, 0xe357000e),
            (0xc0a7f39c, 0x0a00000f),
            (0xc0a7f3a0, 0xe3570011),
            (0xc0a7f3a4, 0x1a000032),
            (0xc0a7f3ac, 0xe5850030),
            (0xc0a7f3fc, 0xe5850068),
        ] {
            put(&mut b, a, v);
        }
        b
    }
    #[test]
    fn accepts_consistent_evidence() {
        assert!(verify(&fixture()).is_ok());
    }
    #[test]
    fn rejects_swapped_property_names() {
        let mut b = fixture();
        let a = word(&b, ATTRS + 16 * 14).unwrap();
        let c = word(&b, ATTRS + 16 * 17).unwrap();
        put(&mut b, ATTRS + 16 * 14, c);
        put(&mut b, ATTRS + 16 * 17, a);
        assert!(verify(&b).is_err());
    }
    #[test]
    fn rejects_wrong_dispatch_load_store_and_truncation() {
        for (a, v) in [
            (0xc0a7efdc, 0xc0a7f134),
            (0xc0a7f13c, 0xe5950068),
            (0xc0a7f3ac, 0xe5850068),
        ] {
            let mut b = fixture();
            put(&mut b, a, v);
            assert!(verify(&b).is_err());
        }
        assert!(verify(&[0; 20]).is_err());
    }
}
