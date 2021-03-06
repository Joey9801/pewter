// Allow over this entire module, as the conditional compilation causes a bunch of spurious unused
// warnings without spamming the #[cfg(...)] attributes everywhere.
#![allow(unused)]

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "bmi2"
))]
#[inline(always)]
fn pext_native(val: u64, mask: u64) -> u64 {
    todo!()
}

// Slower manual implementation of the PEXT behaviour for CPUs which don't support the instruction natively
#[inline(always)]
fn pext_polyfill(val: u64, mut mask: u64) -> u64 {
    let mut res = 0;
    let mut bb = 1;
    while mask != 0 {
        if val & mask & (mask.wrapping_neg()) != 0 {
            res |= bb;
        }
        mask &= mask - 1;
        bb += bb;
    }
    res
}

pub fn pext(val: u64, mask: u64) -> u64 {
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "bmi2"
    ))]
    let ans = pext_native(val, mask);

    #[cfg(not(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "bmi2"
    )))]
    let ans = pext_polyfill(val, mask);

    ans
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::proptest;

    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "bmi2"
    ))]
    proptest! {
        #[test]
        fn test_pext_polyfill(val: u64, mask: u64) {
            let native = pext_native(val, mask);
            let polyfilled = pext_polyfill(val, mask);

            assert_eq!(native, polyfilled);
        }
    }
}
