//! Fingerprints: TLC's FP64 (ADR 0011).
//!
//! [`Fp64`] ports `tlc2.util.FP64` (Rabin fingerprints over GF(2^64), "Written by Allan
//! Heydon and Marc Najork"; tlaplus `94d0c50`) and [`TlcFingerprint`] ports each
//! value's `fingerPrint(long)` method. With polynomial index `n` (TLC's `-fp n`), a
//! value's fingerprint equals TLC's, bit for bit, for every value whose names were
//! interned in TLC's order (model values fingerprint their intern token, as TLC's
//! `UniqueString.fingerPrint` does).
//!
//! The [`Fingerprinter`] trait leaves room for another fingerprint function.

use crate::ctx::Ctx;
use crate::error::{ErrorKind, Result, ValueError};
use crate::print::{ppr, to_tlc_string_lenient};
use crate::value::{FcnRepr, SetRepr, Value};

/// TLC's 131 irreducible polynomials (`FP64.Polys`); `-fp n` selects `POLYS[n]`.
pub const POLYS: [u64; 131] = [
    0x9114_98ae_0e66_bad6,
    0xda8a_0ba6_6dae_0181,
    0xc02f_176b_8f26_8d9f,
    0xd617_bb12_20fc_7812,
    0xc6fd_951a_d34f_9f74,
    0xdd18_97bd_9917_04d4,
    0xf539_4c54_1cbf_d343,
    0xb1dd_ed37_b5c7_b8f7,
    0xb713_ff61_039d_c632,
    0xdfb3_40cb_2fb0_3d43,
    0xbc3e_7e4c_5ecb_76a3,
    0xdbb4_b134_9cd7_058a,
    0xf53e_9dcb_9e91_5cdf,
    0xca5f_58e9_0dd0_1848,
    0x80e7_ff44_0689_1aa1,
    0xab54_1bf8_81fa_8571,
    0xbf27_4e07_ac54_99d5,
    0x939b_1ea9_3304_0a4e,
    0xb791_a595_448d_75b1,
    0x8bf8_8d6e_f855_63a2,
    0xecb3_3ec3_3951_3a53,
    0xfa2d_3e72_2db5_208a,
    0xb4e2_058a_ac47_9d24,
    0xafbd_6474_e721_3b82,
    0x98c1_694d_14ff_aeef,
    0xe188_fb5c_0a12_5e24,
    0xfa71_cc38_6548_7d80,
    0x8911_35f7_c1c9_4569,
    0xcf77_cdd1_6d22_e3e6,
    0xeb5e_3a1d_2e2b_b4b5,
    0x92f7_f5b6_9cd0_0c55,
    0xa9fb_be40_ca3b_9ae9,
    0x84a7_b33d_8529_5bde,
    0xebd4_680d_bb6f_dee2,
    0xa31f_c46a_0583_b4d0,
    0xa792_c94f_15de_3e49,
    0xd9d6_0a9f_eff4_521a,
    0x9227_ba31_dfdd_a04b,
    0xfb4c_89c6_07ce_162d,
    0xa89b_3b2e_0147_9cc0,
    0xb35a_0c2a_28b8_9f7d,
    0x91d0_b700_b99d_9ec2,
    0xf064_6bbd_a050_20b0,
    0xcb5d_5f63_ce04_3056,
    0xd276_b6a0_4f42_a1b8,
    0xbc1a_7a7d_bfeb_47a3,
    0xe138_acff_7a96_3036,
    0xed86_0223_c155_7ee7,
    0x9b24_91e9_8015_0ab6,
    0xe7e0_3dd8_a5b4_e59e,
    0xaaa3_f5ea_c516_783c,
    0xfe78_cc26_7a72_4180,
    0xc225_19a2_1edf_ac64,
    0xcdb2_9419_33fe_c60f,
    0xc5f4_8555_1ef3_8aa1,
    0xa192_93d2_50bd_3335,
    0xa4d4_c215_a50b_7afe,
    0xc115_5176_406a_5070,
    0xecad_aa82_00e1_23ec,
    0xbacf_e629_d58b_2f08,
    0xded9_9108_2148_cb42,
    0x9a0c_cbe5_deaa_88db,
    0x9a83_246f_3420_61bb,
    0xb714_8284_2297_ab05,
    0xf240_7eeb_9975_92bd,
    0xb7b4_3d4b_5c4c_4bcc,
    0xb339_a256_8221_ffe7,
    0xdb4b_6b37_9446_ef9e,
    0xe43c_205b_b5c0_b2b6,
    0xe8e1_d141_f19d_6db0,
    0xd19e_8710_a4ea_1c86,
    0x9704_cecf_a8d6_d07b,
    0xb0a3_5716_162c_3f26,
    0xa2a6_8c0c_f563_90ac,
    0xe4a7_4bc6_01c9_5b46,
    0xe668_fff6_7559_5e56,
    0xe0ea_77eb_e06f_abbb,
    0xa8bb_94f5_8527_9523,
    0xbeb6_67b4_2b68_4f21,
    0xd7b6_5410_189e_28af,
    0x8572_2037_beff_c5b9,
    0xe7e7_c5f7_7342_6204,
    0xa5fd_0cc8_e060_c6f4,
    0xb8f9_1ee9_065d_cf95,
    0xb404_7008_040f_8b50,
    0xecb9_ab62_91c8_cfcc,
    0xe08b_b9b7_0caa_d6df,
    0xd6e0_86a3_01d9_5d56,
    0xf6a8_08f5_f3fb_9da3,
    0xfa74_b8a8_ef86_fbc5,
    0xa0b6_b33b_a9e6_381c,
    0x8c78_7034_2787_3dbb,
    0xc551_6ea4_2301_1021,
    0xb075_cce8_528a_e7e2,
    0x92e4_a379_79e2_b13b,
    0xafc9_ab00_0ed8_1026,
    0xf187_3f2a_861a_518a,
    0xf885_d6b3_5770_192c,
    0xd2a8_2f27_f71f_5f7e,
    0xd5e7_a4dd_8beb_2d9f,
    0xbab9_e7e6_5dc2_3e0f,
    0xd5fc_877c_0bdf_5b85,
    0xab42_8169_b9f3_1c02,
    0xb7b1_351f_9266_d3ea,
    0xfad5_6491_4328_f635,
    0x9623_ecb1_000d_b9bd,
    0x88d3_71ec_6644_c892,
    0xd0a7_1270_573e_271c,
    0xbd20_0d76_3f9d_81b4,
    0x8484_fd96_374b_f2ea,
    0xe0d0_8107_4943_2294,
    0x8ebe_9dad_c886_58ba,
    0xf626_8c58_993a_e542,
    0xd6cc_88fe_d0d3_59c9,
    0xbcad_db8d_40a1_6690,
    0x9281_7c6d_b421_cbf6,
    0xac63_7211_20a3_71b5,
    0xd6cc_137f_dced_0820,
    0xf1c5_fbae_bb61_7bd7,
    0xac35_d78c_7652_37a5,
    0xbd0a_471f_a9a2_3116,
    0x943a_7031_b946_a5ae,
    0xe4e8_3520_cb1a_aebb,
    0xe92a_4b73_246d_d124,
    0xafc1_e070_787c_4c86,
    0xdfe8_4d42_cf06_286b,
    0x8b29_ec96_2e4b_964b,
    0x807e_b5de_812e_de0f,
    0xa3cd_7129_9c8b_3bfd,
    0x845b_8031_ef88_6f35,
    0x91f5_a5fa_9c55_15a5,
];

/// TLC's value kind codes (`tlc2.value.ValueConstants`) that appear in fingerprints.
mod kind {
    pub const BOOL: u8 = 0;
    pub const INT: u8 = 1;
    pub const STRING: u8 = 3;
    pub const SETENUM: u8 = 5;
    pub const FCNRCD: u8 = 9;
    pub const MODEL: u8 = 21;
    /// dive-only: integers outside 32 bits (big-integer mode). TLC has no such value,
    /// so the code is outside TLC's range (0..=26).
    pub const DIVE_BIGINT: u8 = 0x80;
}

/// The FP64 fingerprint function for one irreducible polynomial.
#[derive(Clone)]
pub struct Fp64 {
    irred: u64,
    table: [u64; 256],
}

impl std::fmt::Debug for Fp64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fp64({:#018x})", self.irred)
    }
}

impl Fp64 {
    /// The fingerprint function TLC uses with `-fp index` (`FP64.Init(int)`).
    #[must_use]
    pub fn new(index: usize) -> Option<Self> {
        POLYS.get(index).map(|&p| Self::with_poly(p))
    }

    /// `FP64.Init(long)`: builds the byte table for an irreducible polynomial.
    #[must_use]
    pub fn with_poly(poly: u64) -> Self {
        let mut power = [0u64; 72];
        let mut t: u64 = 0x8000_0000_0000_0000;
        for p in &mut power {
            *p = t;
            let mask = if t & 1 != 0 { poly } else { 0 };
            t = (t >> 1) ^ mask;
        }
        let mut table = [0u64; 256];
        for (j, slot) in table.iter_mut().enumerate() {
            let mut v = 0u64;
            for k in 0..8 {
                if j & (1 << k) != 0 {
                    v ^= power[71 - k];
                }
            }
            *slot = v;
        }
        Self { irred: poly, table }
    }

    /// `FP64.New()`: the fingerprint of the empty string (the polynomial itself).
    #[must_use]
    pub const fn empty(&self) -> u64 {
        self.irred
    }

    /// `FP64.Extend(long, byte)`.
    #[must_use]
    #[inline]
    pub fn extend_byte(&self, fp: u64, b: u8) -> u64 {
        (fp >> 8) ^ self.table[usize::from(b ^ fp.to_le_bytes()[0])]
    }

    /// `FP64.Extend(long, char)`: only the low byte of the UTF-16 unit is used.
    #[must_use]
    #[inline]
    pub fn extend_char(&self, fp: u64, c: u16) -> u64 {
        self.extend_byte(fp, c.to_le_bytes()[0])
    }

    /// `FP64.Extend(long, int)`: four bytes, least significant first.
    #[must_use]
    #[inline]
    pub fn extend_i32(&self, fp: u64, x: i32) -> u64 {
        x.to_le_bytes()
            .iter()
            .fold(fp, |fp, &b| self.extend_byte(fp, b))
    }

    /// `FP64.Extend(long, long)`: eight bytes, least significant first.
    #[must_use]
    pub fn extend_i64(&self, fp: u64, x: i64) -> u64 {
        x.to_le_bytes()
            .iter()
            .fold(fp, |fp, &b| self.extend_byte(fp, b))
    }

    /// `FP64.Extend(long, String)`: one step per UTF-16 unit, low byte only.
    #[must_use]
    pub fn extend_str(&self, fp: u64, s: &str) -> u64 {
        s.encode_utf16().fold(fp, |fp, u| self.extend_char(fp, u))
    }
}

/// A fingerprint function over values.
pub trait Fingerprinter: Send + Sync {
    /// Extends `fp` by the value `v`.
    ///
    /// # Errors
    /// For values that cannot be fingerprinted (infinite sets) and enumeration errors.
    fn extend(&self, fp: u64, v: &Value, ctx: Ctx) -> Result<u64>;

    /// The fingerprint of `v` alone (TLC: `v.fingerPrint(FP64.New())`).
    ///
    /// # Errors
    /// As [`Fingerprinter::extend`].
    fn fingerprint(&self, v: &Value, ctx: Ctx) -> Result<u64>;

    /// The fingerprint of a state: the values of its variables in declaration order
    /// (TLC: `TLCStateMut.fingerPrint` without symmetry or `VIEW`).
    ///
    /// # Errors
    /// As [`Fingerprinter::extend`].
    fn fingerprint_state(&self, vars: &[Value], ctx: Ctx) -> Result<u64>;
}

/// TLC's value fingerprints (`fingerPrint(long)` of each `tlc2.value.impl` class).
#[derive(Clone, Debug)]
pub struct TlcFingerprint {
    fp: Fp64,
}

impl TlcFingerprint {
    /// Fingerprints with TLC's polynomial `index` (`-fp index`; the goldens use 0).
    #[must_use]
    pub fn new(index: usize) -> Option<Self> {
        Fp64::new(index).map(|fp| Self { fp })
    }

    /// The underlying FP64 function.
    #[must_use]
    pub const fn fp64(&self) -> &Fp64 {
        &self.fp
    }
}

impl Fingerprinter for TlcFingerprint {
    fn extend(&self, fp: u64, v: &Value, ctx: Ctx) -> Result<u64> {
        let f = &self.fp;
        Ok(match v {
            Value::Bool(b) => {
                f.extend_char(f.extend_byte(fp, kind::BOOL), if *b { 0x74 } else { 0x66 })
            }
            Value::Int(i) => match i32::try_from(*i) {
                Ok(i) => f.extend_i32(f.extend_byte(fp, kind::INT), i),
                Err(_) => big_int(f, fp, &num_bigint::BigInt::from(*i)),
            },
            Value::BigInt(b) => big_int(f, fp, b),
            Value::Str(a) => {
                let fp = f.extend_byte(fp, kind::STRING);
                let fp = f.extend_i32(fp, len_i32(a.utf16_len() as usize));
                f.extend_str(fp, a.as_str())
            }
            Value::Model(a) => {
                let fp = f.extend_byte(fp, kind::MODEL);
                f.extend_i32(fp, i32::try_from(a.tok()).unwrap_or(i32::MAX))
            }
            Value::Set(s) => {
                if matches!(s.repr(), SetRepr::Nat | SetRepr::Int | SetRepr::Strings)
                    || (matches!(s.repr(), SetRepr::Seq(_)) && s.is_infinite())
                {
                    return Err(ValueError::new(
                        ErrorKind::NotFingerprintable,
                        format!(
                            "TLC has found a state in which the value of a variable contains {}",
                            ppr(&to_tlc_string_lenient(v, ctx))
                        ),
                    ));
                }
                let elems = s.elements(ctx)?;
                let mut fp = f.extend_byte(fp, kind::SETENUM);
                fp = f.extend_i32(fp, len_i32(elems.len()));
                for e in elems.iter() {
                    fp = self.extend(fp, e, ctx)?;
                }
                fp
            }
            Value::Fcn(fun) => {
                let mut fp = f.extend_byte(fp, kind::FCNRCD);
                fp = f.extend_i32(fp, len_i32(fun.len()));
                match &fun.0.repr {
                    FcnRepr::Tuple(e) => {
                        for (i, x) in e.iter().enumerate() {
                            fp = f.extend_byte(fp, kind::INT);
                            fp = f.extend_i32(fp, len_i32(i + 1));
                            fp = self.extend(fp, x, ctx)?;
                        }
                    }
                    FcnRepr::Record(fs) => {
                        for (a, x) in fs {
                            fp = f.extend_byte(fp, kind::STRING);
                            fp = f.extend_i32(fp, len_i32(a.utf16_len() as usize));
                            fp = f.extend_str(fp, a.as_str());
                            fp = self.extend(fp, x, ctx)?;
                        }
                    }
                    FcnRepr::General(ps) => {
                        for (d, x) in ps {
                            fp = self.extend(fp, d, ctx)?;
                            fp = self.extend(fp, x, ctx)?;
                        }
                    }
                }
                fp
            }
        })
    }

    fn fingerprint(&self, v: &Value, ctx: Ctx) -> Result<u64> {
        self.extend(self.fp.empty(), v, ctx)
    }

    fn fingerprint_state(&self, vars: &[Value], ctx: Ctx) -> Result<u64> {
        vars.iter()
            .try_fold(self.fp.empty(), |fp, v| self.extend(fp, v, ctx))
    }
}

fn len_i32(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// dive's extension for integers outside 32 bits: a kind byte TLC never uses, the
/// sign, the byte count and the magnitude.
fn big_int(f: &Fp64, fp: u64, b: &num_bigint::BigInt) -> u64 {
    let (sign, mag) = b.to_bytes_le();
    let mut fp = f.extend_byte(fp, kind::DIVE_BIGINT);
    fp = f.extend_byte(fp, u8::from(sign == num_bigint::Sign::Minus));
    fp = f.extend_i32(fp, len_i32(mag.len()));
    mag.iter().fold(fp, |fp, &x| f.extend_byte(fp, x))
}
