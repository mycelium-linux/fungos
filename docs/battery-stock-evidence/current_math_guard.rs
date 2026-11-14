//! Offline evidence notebook; not linked to the HAL. Synthetic math is not a
//! complete emulator and deliberately excludes exceptional float/int conversion.
use std::{env, fs};

const OPS: &[(usize, &[u16])] = &[
    (0x3150, &[0xee81, 0x1a09]), // interval / 1000
    (0x315c, &[0xeec0, 0xfa0d]), // slope / 10000
    (0x3170, &[0xee82, 0x8a0e]), // sample / 1e6
    (0x3174, &[0xeec0, 0x9a09]), // resistance / 1000
    (0x3188, &[0xee83, 0xaa0e]), // OCV / 1e6
    (0x318c, &[0xee3a, 0x0a48]), // OCV - sample
    (0x3190, &[0xee81, 0x1a2d]), // dynamic term / capacity
    (0x3194, &[0xee30, 0x0a00]), // twice voltage difference
    (0x3198, &[0xee32, 0x2a01]), // denominator
    (0x319c, &[0xeec0, 0xea02]), // s29 current
    (0x31a0, &[0xee01, 0xaa6e]), // recursive OCV subtraction
    (0x3258, &[0xee2e, 0x0a8e]), // s29 * s28
    (0x325e, &[0xeebd, 0x0ac0]), // signed truncating conversion
    (0x3268, &[0xee10, 0x5a10]), // current integer -> r5
    (0x326e, &[0x2500]),         // no samples -> zero, not measurement
    (0x3272, &[0xb917]),         // OCV changed -> publish
    (0x3274, &[0x2800]),         // otherwise test samples-processed flag
    (0x3276, &[0xf000, 0x8085]), // no samples/no change -> skip write
    (0x3290, &[0xf7ff, 0xf8a0]), // write_int_to_file
    (0x3294, &[0xf1b0, 0x3fff]), // compare write return to -1
    (0x3298, &[0xdc07]),         // successful write bypasses error log
];
const POOL: &[(usize, f32)] = &[
    (0x2bf4, 1000.0),
    (0x2bf8, 3600.0),
    (0x2bfc, 10000.0),
    (0x2c00, 1000000.0),
    (0x2c04, 100000.0),
];
fn verify(b: &[u8]) -> Result<(), String> {
    for &(va, words) in OPS {
        for (i, word) in words.iter().enumerate() {
            let off = va - 0x1000 + i * 2;
            if b.get(off..off + 2) != Some(word.to_le_bytes().as_slice()) {
                return Err(format!("Thumb instruction mismatch at {:#x}", va + i * 2));
            }
        }
    }
    for &(va, value) in POOL {
        let off = va - 0x1000;
        if b.get(off..off + 4) != Some(value.to_le_bytes().as_slice()) {
            return Err(format!("float pool mismatch at {va:#x}"));
        }
    }
    Ok(())
}

// Ordinary finite synthetic values only; model inputs are already adjusted.
// Preserve the recovered binary32 operation ordering, not an algebraic rewrite.
#[cfg(test)]
fn step(
    ocv_uv: i32,
    sample_uv: i32,
    r_mohm: f32,
    fcc_mah: f32,
    slope: i32,
    interval_ms: i32,
    count: i32,
) -> (f32, f32) {
    let capacity = (fcc_mah / 1000.0) * 3600.0;
    let t = interval_ms as f32 / 1000.0;
    let l = slope as f32 / 10000.0;
    let d = ((t * l) * count as f32) / capacity;
    let r = r_mohm / 1000.0;
    let e = ocv_uv as f32 / 1000000.0;
    let v = sample_uv as f32 / 1000000.0;
    let delta = e - v;
    let i = (delta + delta) / ((r + r) + d);
    (i * 1000000.0, (e - d * i) * 1000000.0)
}
fn main() {
    let path = env::args()
        .nth(1)
        .expect("supply private vm_bms executable path");
    verify(&fs::read(path).expect("read retained executable"))
        .expect("stock current arithmetic evidence mismatch");
    println!("PASS: selected Thumb current arithmetic + float pool (not full binary identity)");
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 0x2500];
        for &(va, words) in OPS {
            for (i, w) in words.iter().enumerate() {
                b[va - 0x1000 + i * 2..va - 0x1000 + i * 2 + 2].copy_from_slice(&w.to_le_bytes());
            }
        }
        for &(va, v) in POOL {
            b[va - 0x1000..va - 0x1000 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    #[test]
    fn accepts_consistent_selected_bytes() {
        assert!(verify(&fixture()).is_ok());
    }
    #[test]
    fn rejects_each_instruction_mutation() {
        for &(va, words) in OPS {
            for i in 0..words.len() * 2 {
                let mut b = fixture();
                b[va - 0x1000 + i] ^= 1;
                assert!(verify(&b).is_err(), "{va:#x}+{i}");
            }
        }
    }
    #[test]
    fn rejects_scale_mutations_and_truncation() {
        for &(va, _) in POOL {
            let mut b = fixture();
            b[va - 0x1000] ^= 1;
            assert!(verify(&b).is_err());
        }
        assert!(verify(&[]).is_err());
        assert!(verify(&fixture()[..0x2000]).is_err());
    }
    #[test]
    fn synthetic_sign_units_and_zero() {
        let (positive, _) = step(4000000, 3900000, 200.0, 2000.0, 10000, 1000, 1);
        let (negative, _) = step(3900000, 4000000, 200.0, 2000.0, 10000, 1000, 1);
        let (zero, _) = step(4000000, 4000000, 200.0, 2000.0, 10000, 1000, 1);
        assert!((positive - 499826.3).abs() < 2.0);
        assert_eq!(positive, -negative);
        assert_eq!(zero, 0.0);
        assert_eq!(positive.trunc(), 499825.0);
        assert_eq!(negative.trunc(), -499825.0);
    }
    #[test]
    fn invalid_math_is_not_a_valid_zero_current() {
        let (nan, _) = step(4000000, 4000000, 0.0, 2000.0, 0, 1000, 1);
        assert!(nan.is_nan()); // 0/0, not measured zero
        let (infinite, _) = step(4000000, 3900000, 0.0, 2000.0, 0, 1000, 1);
        assert!(infinite.is_infinite());
        let (_, next) = step(4000000, 3900000, 200.0, 0.0, 10000, 1000, 1);
        assert!(!next.is_finite()); // zero FCC invalidates recursion
                                    // Do NOT cast these to integers: host casts do not model ARM exceptions.
    }
    #[test]
    fn skipped_failed_and_zero_writes_cannot_prove_fresh_measurement() {
        // Abstract cache consequences, not a vendor/init emulator. Kernel setter
        // stores only a value; neither timestamps nor validity accompany it.
        let initial_cache = 123456;
        for (samples, ocv_changed, write_ok, attempted) in [
            (false, false, true, 0),      // no write attempted
            (true, false, false, 200000), // failed sysfs write
            (false, true, true, 0),       // no-sample zero is published
        ] {
            let publish = samples || ocv_changed;
            let cache = if publish && write_ok {
                attempted
            } else {
                initial_cache
            };
            if !publish || !write_ok {
                assert_eq!(cache, initial_cache);
            } else {
                assert_eq!(cache, 0);
                assert!(!samples);
            }
        }
        // A repeated value has no embedded generation or observation time.
        assert_eq!(initial_cache, initial_cache);
    }
    #[test]
    fn synthetic_recursive_ocv_and_last_entry_not_mean() {
        let (first, next) = step(4000000, 3900000, 200.0, 2000.0, 10000, 1000, 1);
        assert!(next < 4000000.0 && next > 3900000.0);
        let (last, _) = step(next.trunc() as i32, 3950000, 200.0, 2000.0, 10000, 1000, 1);
        assert!(last > 0.0 && last < first);
        assert_ne!(last.trunc(), ((first + last) / 2.0).trunc());
    }
}
