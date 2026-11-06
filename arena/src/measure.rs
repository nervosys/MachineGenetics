//! Cheap measures of how much structure a byte sequence has.
//!
//! The arena's real criterion is what a learner can fit, and that takes a
//! training run. This is the fast proxy for comparing *program vocabularies*:
//! the Lempel–Ziv (1976) complexity of an output, normalised to an entropy
//! rate. Pseudo-random bytes sit near the top of the range and constant runs
//! near zero; the regularities self-play pretraining wants — copying, periodic
//! templates, nesting — land in between. It was added when the transformer
//! learner showed the generated data to be mostly noise (`ARENA.md`), so that a
//! vocabulary change could be judged before spending a GPU run on it.

/// Number of phrases in the Lempel–Ziv (1976) exhaustive parse of `s`
/// (Kaspar & Schuster's algorithm).
pub fn lz76_phrases(s: &[u8]) -> usize {
    let n = s.len();
    if n < 2 {
        return n;
    }
    let (mut i, mut k, mut l, mut c, mut k_max) = (0usize, 1usize, 1usize, 1usize, 1usize);
    loop {
        if s[i + k - 1] == s[l + k - 1] {
            k += 1;
            if l + k > n {
                c += 1;
                break;
            }
        } else {
            k_max = k_max.max(k);
            i += 1;
            if i == l {
                c += 1;
                l += k_max;
                if l + 1 > n {
                    break;
                }
                i = 0;
                k = 1;
                k_max = 1;
            } else {
                k = 1;
            }
        }
    }
    c
}

/// LZ76 entropy-rate estimate in bits per byte: `c · log₂(n) / n`, capped at
/// 8. Biased on short inputs, so compare like with like — the same lengths,
/// different vocabularies.
pub fn lz_bits_per_byte(s: &[u8]) -> f64 {
    let n = s.len();
    if n < 2 {
        return 0.0;
    }
    (lz76_phrases(s) as f64 * (n as f64).log2() / n as f64).min(8.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_simple_noise_is_not_and_structure_is_between() {
        let constant = vec![7u8; 256];
        let periodic: Vec<u8> = b"abcde".iter().cycle().take(256).cloned().collect();
        let mut rng = crate::grammar::Rng(11);
        let noise: Vec<u8> = (0..256).map(|_| rng.next_u64() as u8).collect();
        let (c, p, r) = (lz_bits_per_byte(&constant), lz_bits_per_byte(&periodic), lz_bits_per_byte(&noise));
        assert!(c < p && p < r, "constant {c}, periodic {p}, noise {r}");
        assert!(r > 4.0, "noise should look near-incompressible: {r}");
    }

    #[test]
    fn the_known_parse_of_a_textbook_example() {
        // Kaspar & Schuster: 0·001·10·100·1000·101 → 6 phrases.
        let s: Vec<u8> = "0001101001000101".bytes().collect();
        assert_eq!(lz76_phrases(&s), 6);
    }
}
