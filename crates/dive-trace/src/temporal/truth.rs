//! Position-indexed truth on a single nonempty, ultimately periodic word.
#[derive(Clone)]
pub(super) struct Truth {
    bits: Vec<u64>,
}

impl Truth {
    pub(super) fn constant(len: usize, value: bool) -> Self {
        Self {
            bits: vec![if value { u64::MAX } else { 0 }; len.div_ceil(64)],
        }
    }

    pub(super) fn pack(values: &[bool]) -> Self {
        let bits = values
            .chunks(64)
            .map(|part| {
                part.iter()
                    .enumerate()
                    .fold(0, |word, (i, value)| word | (u64::from(*value) << i))
            })
            .collect();
        Self { bits }
    }

    pub(super) fn unpack(&self, len: usize) -> Vec<bool> {
        (0..len).map(|i| self.at(i)).collect()
    }

    pub(super) fn at(&self, i: usize) -> bool {
        self.bits[i / 64] >> (i % 64) & 1 != 0
    }

    pub(super) fn set(&mut self, i: usize) {
        self.bits[i / 64] |= 1 << (i % 64);
    }

    pub(super) fn invert(mut self) -> Self {
        for word in &mut self.bits {
            *word = !*word;
        }
        self
    }

    pub(super) fn combine(mut self, rhs: &Self, op: impl Fn(u64, u64) -> u64) -> Self {
        for (left, right) in self.bits.iter_mut().zip(&rhs.bits) {
            *left = op(*left, *right);
        }
        self
    }

    pub(super) fn select(mut self, yes: &Self, no: &Self) -> Self {
        for ((guard, yes), no) in self.bits.iter_mut().zip(&yes.bits).zip(&no.bits) {
            *guard = (*guard & *yes) | (!*guard & *no);
        }
        self
    }

    // A decisive position (false for Always, true for Eventually) on the cycle
    // is visible from every suffix. Before the cycle it affects exactly the
    // suffix starts up to and including its final occurrence.
    pub(super) fn future(mut self, len: usize, back: usize, always: bool) -> Self {
        let default = if always { u64::MAX } else { 0 };
        let mut boundary = 0;
        for (i, &word) in self.bits.iter().enumerate().rev() {
            let valid = u64::MAX >> (64 - (len - i * 64).min(64));
            let decisive = (word ^ default) & valid;
            if decisive != 0 {
                boundary = i * 64 + 64 - decisive.leading_zeros() as usize;
                break;
            }
        }
        if boundary > back {
            self.bits.fill(!default);
        } else {
            for (i, word) in self.bits.iter_mut().enumerate() {
                let n = boundary.saturating_sub(i * 64).min(64);
                *word = default ^ if n == 0 { 0 } else { u64::MAX >> (64 - n) };
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::Truth;

    #[test]
    fn future_uses_the_real_closing_edge_and_ignores_padding_bits() {
        for len in [1, 63, 64, 65, 127, 128, 129] {
            for back in [0, len / 2, len - 1] {
                for position in [0, back, len - 1] {
                    for always in [false, true] {
                        let mut decisive = Truth::constant(len, false);
                        decisive.set(position);
                        let input = if always { decisive.invert() } else { decisive };
                        let got = input.future(len, back, always).unpack(len);
                        let expected = (0..len)
                            .map(|i| {
                                if position >= back || i <= position {
                                    !always
                                } else {
                                    always
                                }
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(got, expected);
                        let all = Truth::constant(len, always);
                        assert_eq!(all.future(len, back, always).unpack(len), vec![always; len]);
                    }
                }
            }
        }
    }
}
