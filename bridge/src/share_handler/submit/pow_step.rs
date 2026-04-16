//! Single-job PoW evaluation from a template header and nonce (no I/O, unit-tested).

use super::pow_math;
use crate::hasher::calculate_target;
use kaspa_consensus_core::header::Header;
use kaspa_pow::State as PowState;
use num_bigint::BigUint;

/// Result of [`evaluate_job_pow`] for one job template and nonce.
pub(super) struct JobPowSnapshot {
    pub pow_value: BigUint,
    pub check_passed: bool,
    pub network_target: BigUint,
    pub meets_network_target: bool,
    pub header_bits: u32,
}

/// Clone the header, set `nonce`, run `kaspa_pow`, and derive BigUint targets for logging / decisions.
///
/// Logic matches the former inline block in `pow_loop`: `check_passed` comes from `kaspa_pow`;
/// `meets_network_target` uses the same BigUint comparison helper as before.
pub(super) fn evaluate_job_pow(header: &Header, nonce_val: u64) -> JobPowSnapshot {
    let mut header_clone = header.clone();
    header_clone.nonce = nonce_val;
    let pow_state = PowState::new(&header_clone);
    let (check_passed, pow_value_uint256) = pow_state.check_pow(nonce_val);
    let pow_value = BigUint::from_bytes_be(&pow_value_uint256.to_be_bytes());
    let network_target = calculate_target(header_clone.bits as u64);
    let meets_network_target = pow_math::meets_network_target_biguint(&pow_value, &network_target);
    JobPowSnapshot { pow_value, check_passed, network_target, meets_network_target, header_bits: header_clone.bits }
}

#[cfg(test)]
mod tests {
    use super::super::pow_math;
    use super::*;
    use kaspa_hashes::Hash;

    fn test_header(bits: u32, template_nonce: u64) -> Header {
        let hash = Hash::from_bytes([1; 32]);
        let mut header = Header::from_precomputed_hash(hash, vec![]);
        header.timestamp = 1_700_000_000_000;
        header.bits = bits;
        header.nonce = template_nonce;
        header.daa_score = 0;
        header.blue_score = 0;
        header.version = 0;
        header
    }

    #[test]
    fn evaluate_job_pow_sets_fields_consistently() {
        let h = test_header(0x1d00ffff, 0u64);
        let s = evaluate_job_pow(&h, 0x1234);
        assert_eq!(s.header_bits, h.bits);
        assert_eq!(s.meets_network_target, pow_math::meets_network_target_biguint(&s.pow_value, &s.network_target));
        let s2 = evaluate_job_pow(&h, 0x1234);
        assert_eq!(s.pow_value, s2.pow_value);
        assert_eq!(s.check_passed, s2.check_passed);
    }

    #[test]
    fn different_nonce_changes_pow_value() {
        let h = test_header(0x207fffff, 0u64);
        let a = evaluate_job_pow(&h, 1);
        let b = evaluate_job_pow(&h, 2);
        assert_ne!(a.pow_value, b.pow_value);
    }
}
