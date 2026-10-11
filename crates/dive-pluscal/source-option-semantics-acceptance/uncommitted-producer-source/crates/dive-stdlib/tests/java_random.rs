//! `java.util.Random` and `tlc2.util.RandomGenerator`, bit for bit.
//!
//! The expected values were printed by a small Java program run on the pinned JRE
//! (Temurin 17.0.20.1) against the pinned `tla2tools.jar` (TLC at `94d0c50`):
//! `new Random(seed)` followed by `nextInt()`, `nextInt(bound)` for each bound in
//! [`BOUNDS`], `nextLong()`, `Double.doubleToRawLongBits(nextDouble())`, `nextLong()`;
//! `new RandomGenerator(seed).nextPrime()` five times; and, for the simulation worker
//! seeds, `RandomGenerator.setSeed(seed, 3)` then `nextLong()` three times.

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    reason = "test code; short names mirror the TLA+ specs"
)]

use dive_stdlib::random::{JavaRandom, RandomMode, RandomSource, Streams};

const BOUNDS: [i32; 14] = [
    1,
    2,
    3,
    7,
    10,
    16,
    100,
    1000,
    1 << 20,
    1 << 30,
    i32::MAX,
    1_000_000_007,
    1_431_655_765,
    1_500_000_000,
];

struct Vector {
    seed: i64,
    next_int: i32,
    bounded: [i32; 14],
    long1: i64,
    double_bits: u64,
    long2: i64,
    primes: [i32; 5],
    workers: [i64; 3],
}

const VECTORS: [Vector; 6] = [
    Vector {
        seed: 0,
        next_int: -1_155_484_576,
        bounded: [
            0,
            0,
            2,
            4,
            3,
            8,
            61,
            719,
            819_498,
            357_790_538,
            542_832_677,
            827_187_473,
            1_316_484_262,
            49_567_875,
        ],
        long1: -1_083_761_183_081_836_303,
        double_bits: 4_598_624_750_233_831_718,
        long2: 2_377_732_757_510_138_102,
        primes: [1_290_637, 1_281_667, 1_288_871, 1_287_371, 1_288_117],
        workers: [
            -8_292_973_307_042_192_125,
            -7_423_979_211_207_825_555,
            6_146_794_652_083_548_235,
        ],
    },
    Vector {
        seed: 7,
        next_int: -1_156_638_823,
        bounded: [
            0,
            1,
            1,
            5,
            4,
            14,
            49,
            850,
            596_796,
            377_865_600,
            2_045_813_712,
            259_278_708,
            542_588_911,
            1_495_889_662,
        ],
        long1: 1_535_132_644_386_981_093,
        double_bits: 4_606_540_638_494_645_542,
        long2: -6_611_035_329_062_026_007,
        primes: [1_290_631, 1_291_063, 1_283_573, 1_293_967, 1_290_199],
        workers: [
            -1_894_902_459_288_369_262,
            -5_383_181_422_176_253_347,
            6_491_681_576_930_330_529,
        ],
    },
    Vector {
        seed: -3,
        next_int: 1_154_330_329,
        bounded: [
            0,
            0,
            0,
            5,
            0,
            15,
            20,
            636,
            959_391,
            163_031_395,
            235_552_103,
            459_176_357,
            349_417_845,
            1_456_753_449,
        ],
        long1: -8_283_155_822_535_553_102,
        double_bits: 4_605_774_313_040_974_533,
        long2: 6_778_946_066_838_938_191,
        primes: [1_282_117, 1_283_683, 1_292_149, 1_295_563, 1_287_661],
        workers: [
            -350_517_750_740_384_063,
            -7_995_055_950_849_770_773,
            2_800_858_052_341_037_775,
        ],
    },
    Vector {
        seed: 1_234_567_890_123,
        next_int: -8_722_476,
        bounded: [
            0,
            1,
            2,
            2,
            5,
            12,
            65,
            80,
            229_683,
            971_258_015,
            1_234_316_534,
            570_267_634,
            694_613_688,
            92_999_599,
        ],
        long1: 5_027_400_435_344_464_924,
        double_bits: 4_600_715_993_501_221_902,
        long2: 5_862_476_205_997_460_119,
        primes: [1_295_881, 1_291_379, 1_291_861, 1_291_249, 1_281_149],
        workers: [
            -4_427_497_540_126_799_758,
            3_936_690_402_237_823_603,
            -1_760_658_433_924_376_084,
        ],
    },
    Vector {
        seed: i64::MAX,
        next_int: 1_155_099_827,
        bounded: [
            0,
            0,
            2,
            6,
            8,
            6,
            78,
            765,
            59_283,
            865_475_904,
            665_220_512,
            121_954_317,
            149_520_196,
            176_890_342,
        ],
        long1: -5_716_699_375_945_509_478,
        double_bits: 4_606_305_265_354_634_241,
        long2: -4_952_432_241_328_847_918,
        primes: [1_282_117, 1_277_233, 1_289_329, 1_284_967, 1_292_591],
        workers: [
            7_681_931_065_131_779_340,
            -3_206_673_117_535_979_274,
            -3_577_981_259_754_968_255,
        ],
    },
    // Only the low 48 bits of the seed matter: Long.MIN_VALUE behaves as 0.
    Vector {
        seed: i64::MIN,
        next_int: -1_155_484_576,
        bounded: [
            0,
            0,
            2,
            4,
            3,
            8,
            61,
            719,
            819_498,
            357_790_538,
            542_832_677,
            827_187_473,
            1_316_484_262,
            49_567_875,
        ],
        long1: -1_083_761_183_081_836_303,
        double_bits: 4_598_624_750_233_831_718,
        long2: 2_377_732_757_510_138_102,
        primes: [1_290_637, 1_281_667, 1_288_871, 1_287_371, 1_288_117],
        workers: [
            -8_292_973_307_042_192_125,
            -7_423_979_211_207_825_555,
            6_146_794_652_083_548_235,
        ],
    },
];

#[test]
fn java_util_random_is_bit_exact() {
    for v in &VECTORS {
        let mut r = JavaRandom::new(v.seed);
        assert_eq!(r.next_bits(32), v.next_int, "seed {}", v.seed);
        for (b, want) in BOUNDS.iter().zip(v.bounded) {
            assert_eq!(
                r.next_int_bounded(*b),
                Some(want),
                "seed {} bound {b}",
                v.seed
            );
        }
        assert_eq!(r.next_long(), v.long1, "seed {}", v.seed);
        assert_eq!(r.next_double().to_bits(), v.double_bits, "seed {}", v.seed);
        assert_eq!(r.next_long(), v.long2, "seed {}", v.seed);
    }
}

#[test]
fn random_generator_next_prime_is_bit_exact() {
    for v in &VECTORS {
        let mut r = JavaRandom::new(v.seed);
        let got: Vec<i32> = (0..5).map(|_| r.next_prime()).collect();
        assert_eq!(got, v.primes, "seed {}", v.seed);
    }
}

#[test]
fn simulation_worker_seeds_are_bit_exact() {
    for v in &VECTORS {
        let streams = Streams::new(v.seed, RandomMode::Tlc);
        assert_eq!(
            streams.tlc_simulation_worker_seeds(3, 3),
            v.workers,
            "seed {}",
            v.seed
        );
    }
}

#[test]
fn set_seed_resets_the_generator() {
    let mut r = JavaRandom::new(1);
    let _ = r.next_long();
    r.set_seed(7);
    assert_eq!(r, JavaRandom::new(7));
}
