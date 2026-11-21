//! Where each random stream starts: TLC's seeding, and dive's replayable seeding.

use super::source::{JavaRandom, RandomSource};

/// Which seeding a run uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RandomMode {
    /// TLC's seeding with `java.util.Random`, bit-exact (`--compat=tlc`). Requires
    /// TLC's own state fingerprints for state-level draws.
    Tlc,
    /// dive's seeding: every stream is a pure function of the seed, a
    /// stream key (the state's fingerprint, or a simulation trace number) and the call
    /// index, so a run replays at any worker count. Any 64-bit state fingerprint works.
    Dive,
}

/// The run's seed and seeding: hands out the random stream for each evaluation.
///
/// **Model checking** (TLC's `RandomEnumerableValues`, `TLCStateRandom`):
/// - [`Streams::startup`] is one stream, used in order by the constant definitions
///   TLC evaluates at start-up, the `ASSUME`s, and the initial predicate, including
///   invariants checked on initial states. In TLC this is the main thread's
///   generator: `new Random(seed)`, never reseeded.
/// - [`Streams::state`] is the stream for everything evaluated while expanding one
///   state: its next-state relation (all actions, in order), and the checks on the
///   successors. TLC reseeds the worker's generator to `fingerprint(state) ^ seed`
///   on the first draw for each newly expanded state, and keeps drawing from it for
///   the rest of that expansion.
/// - Trace reconstruction replays with the same streams: [`Streams::startup`] for
///   the initial states, then [`Streams::state`] per predecessor (TLC resets the
///   generator, `ConcurrentTLCTrace.getTrace`). In TLC the reset stream does *not*
///   replay draws made by start-up definitions and `ASSUME`s before `Init`, so a
///   violation that depends on them can lose its counterexample.
///
/// **Simulation** (TLC's `Simulator`, `SimulationWorker`, `DefaultRandom`):
/// - [`Streams::startup`] serves constants, `ASSUME`s and the initial states, as above.
/// - Each worker chooses initial states, actions and successors with its own
///   generator, seeded from [`Streams::tlc_simulation_worker_seeds`].
/// - Each worker evaluates `RandomElement` and friends with
///   [`Streams::tlc_simulation_values`], `new Random(seed)` per worker thread,
///   never reseeded; so in TLC every worker draws the same value sequence.
///
/// dive's simulation uses [`Streams::simulation_trace`] in both modes: it replaces
/// both per-worker generators with one stream per trace number, so traces are
/// reproducible whatever worker runs them. The TLC worker seeds are kept for
/// reference and tests only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Streams {
    seed: i64,
    mode: RandomMode,
}

impl Streams {
    /// The streams of a run with `seed` (always reported) in `mode`.
    #[must_use]
    pub const fn new(seed: i64, mode: RandomMode) -> Self {
        Self { seed, mode }
    }

    /// The run's seed, as reported (`TLCGet("config").seed`).
    #[must_use]
    pub const fn seed(&self) -> i64 {
        self.seed
    }

    /// The seeding mode.
    #[must_use]
    pub const fn mode(&self) -> RandomMode {
        self.mode
    }

    /// The start-up stream: constant definitions, `ASSUME`s and `Init`, in TLC's
    /// evaluation order.
    #[must_use]
    pub const fn startup(&self) -> Stream {
        match self.mode {
            RandomMode::Tlc => Stream::Java(JavaRandom::new(self.seed)),
            RandomMode::Dive => Stream::Dive(DiveStream::new(self.seed, Domain::Startup, 0)),
        }
    }

    /// The stream for expanding the state with fingerprint `fp`: under
    /// [`RandomMode::Tlc`] TLC's state fingerprint (`TLCState.fingerPrint`), seeding
    /// `new Random(fp ^ seed)` (`RandomEnumerableValues.TLCStateRandom`).
    #[must_use]
    pub const fn state(&self, fp: u64) -> Stream {
        match self.mode {
            #[allow(clippy::cast_possible_wrap, reason = "Java's long: same bits")]
            RandomMode::Tlc => Stream::Java(JavaRandom::new(fp as i64 ^ self.seed)),
            RandomMode::Dive => Stream::Dive(DiveStream::new(self.seed, Domain::State, fp)),
        }
    }

    /// TLC simulation: the seeds of the `workers` simulation workers' generators,
    /// `rng.nextLong()` each in worker order after `rng.setSeed(seed, aril)`
    /// (`TLC.process`, `Simulator` constructor). With TLC's default (no `-seed`) the
    /// seed itself came from an unseeded generator and `aril` is ignored; pass 0.
    #[must_use]
    pub fn tlc_simulation_worker_seeds(&self, aril: u64, workers: usize) -> Vec<i64> {
        let mut rng = JavaRandom::new(self.seed);
        rng.set_seed_aril(self.seed, aril);
        (0..workers).map(|_| rng.next_long()).collect()
    }

    /// TLC simulation: a worker thread's generator for `RandomElement` and the
    /// `Randomization` operators (`RandomEnumerableValues.DefaultRandom`).
    #[must_use]
    pub const fn tlc_simulation_values(&self) -> JavaRandom {
        JavaRandom::new(self.seed)
    }

    /// dive simulation: the single stream of trace number `trace` (0-based, in the
    /// order traces are numbered, not the order workers finish them). It drives the
    /// choice of initial state, actions and successors, and the operators' draws.
    #[must_use]
    pub const fn simulation_trace(&self, trace: u64) -> Stream {
        Stream::Dive(DiveStream::new(self.seed, Domain::Simulation, trace))
    }

    /// dive simulation: the stream that draws members of unbounded sets (`Nat`,
    /// `Int`, `Seq(S)`) at one position of trace `trace`, keyed by the trace,
    /// the step, the action tried and the attempt (ADR 0162 §8, §9). It is
    /// separate from [`Self::simulation_trace`], so a trace's choices of
    /// initial state, action and successor do not depend on how many members
    /// were drawn, and the draws at a step do not depend on what earlier
    /// actions or attempts generated.
    #[must_use]
    pub const fn simulation_draws(
        &self,
        trace: u64,
        step: u64,
        action: u64,
        attempt: u64,
    ) -> Stream {
        let key = mix64(mix64(mix64(trace) ^ step) ^ action) ^ attempt;
        Stream::Dive(DiveStream::new(self.seed, Domain::SimulationDraw, key))
    }
}

/// A random stream: TLC's generator or dive's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stream {
    /// `java.util.Random`.
    Java(JavaRandom),
    /// dive's counter-based stream.
    Dive(DiveStream),
}

impl RandomSource for Stream {
    fn next_bits(&mut self, bits: u32) -> i32 {
        match self {
            Self::Java(r) => r.next_bits(bits),
            Self::Dive(r) => r.next_bits(bits),
        }
    }
}

/// What a dive stream is keyed by, so that streams of different kinds never
/// coincide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Domain {
    Startup,
    State,
    Simulation,
    SimulationDraw,
}

impl Domain {
    const fn tag(self) -> u64 {
        match self {
            Self::Startup => 0x5354_4152_5455_5021,        // "STARTUP!"
            Self::State => 0x5354_4154_4521_2121,          // "STATE!!!"
            Self::Simulation => 0x5349_4d55_4c41_5445,     // "SIMULATE"
            Self::SimulationDraw => 0x5349_4d44_5241_5753, // "SIMDRAWS"
        }
    }
}

/// dive's random stream: draw `i` of the stream keyed by `(seed, domain, key)` is
/// `SplitMix64(k + (i + 1) * γ)`, with `k` a mix of the seed, domain and key. It is
/// counter based, so draw `i` depends only on `(seed, key, i)`: never on which
/// worker evaluates it or what it evaluated before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DiveStream {
    key: u64,
    counter: u64,
}

const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// `SplitMix64`'s finalizer (Steele, Lea and Flood, "Fast splittable pseudorandom number
/// generators", OOPSLA 2014), a bijection on 64-bit words.
const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl DiveStream {
    #[allow(clippy::cast_sign_loss, reason = "the seed's bits")]
    const fn new(seed: i64, domain: Domain, key: u64) -> Self {
        let k = mix64(mix64(seed as u64 ^ domain.tag()) ^ key);
        Self { key: k, counter: 0 }
    }

    /// How many 64-bit words the stream has produced (the call index).
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.counter
    }
}

impl RandomSource for DiveStream {
    fn next_bits(&mut self, bits: u32) -> i32 {
        debug_assert!((1..=32).contains(&bits));
        self.counter = self.counter.wrapping_add(1);
        let word = mix64(
            self.key
                .wrapping_add(self.counter.wrapping_mul(GOLDEN_GAMMA)),
        );
        #[allow(clippy::cast_possible_truncation, reason = "the top `bits` bits")]
        let v = (word >> (64 - bits)) as u32;
        v.cast_signed()
    }
}

/// Parses a seed as TLC's `-seed` does (`Long.parseLong`): an optional `+` or `-`
/// and decimal digits, within the range of a 64-bit signed integer.
///
/// Java also accepts non-ASCII decimal digits (`Character.digit`); dive refuses
/// them, returning `None` like any other malformed seed.
#[must_use]
pub fn parse_seed(text: &str) -> Option<i64> {
    let digits = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // `i64::from_str` accepts exactly this grammar (sign, ASCII digits) and range.
    text.parse().ok()
}

/// A fresh seed for a run without `--seed`, as TLC draws one without `-seed`
/// (`new RandomGenerator().nextLong()`, seeded from the clock). dive always
/// reports the seed it used, so the run can be replayed.
#[must_use]
pub fn fresh_seed() -> i64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        h.write_u128(d.as_nanos());
    }
    h.write_u32(std::process::id());
    h.finish().cast_signed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_draw_streams_are_keyed_by_every_position_component() {
        let streams = Streams::new(7, RandomMode::Dive);
        let first = |mut stream: Stream| stream.next_long();
        let base = first(streams.simulation_draws(3, 4, 5, 6));
        assert_eq!(base, first(streams.simulation_draws(3, 4, 5, 6)));
        for other in [
            streams.simulation_draws(2, 4, 5, 6),
            streams.simulation_draws(3, 3, 5, 6),
            streams.simulation_draws(3, 4, 4, 6),
            streams.simulation_draws(3, 4, 5, 5),
            Streams::new(8, RandomMode::Dive).simulation_draws(3, 4, 5, 6),
            streams.simulation_trace(3),
        ] {
            assert_ne!(base, first(other));
        }
    }

    #[test]
    fn seeds_parse_like_long_parse_long() {
        assert_eq!(parse_seed("7"), Some(7));
        assert_eq!(parse_seed("+7"), Some(7));
        assert_eq!(parse_seed("-0"), Some(0));
        assert_eq!(parse_seed("-9223372036854775808"), Some(i64::MIN));
        assert_eq!(parse_seed("9223372036854775808"), None);
        for bad in ["", "+", "-", " 7", "7 ", "0x10", "1e3", "--1", "+-1", "٧"] {
            assert_eq!(parse_seed(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn dive_streams_are_keyed() {
        let s = Streams::new(42, RandomMode::Dive);
        let draws = |mut st: Stream| (0..4).map(|_| st.next_long()).collect::<Vec<_>>();
        assert_eq!(draws(s.state(5)), draws(s.state(5)));
        assert_ne!(draws(s.state(5)), draws(s.state(6)));
        assert_ne!(draws(s.state(0)), draws(s.startup()));
        assert_ne!(draws(s.state(0)), draws(s.simulation_trace(0)));
        assert_ne!(
            draws(s.state(5)),
            draws(Streams::new(43, RandomMode::Dive).state(5))
        );
    }

    #[test]
    fn tlc_state_stream_is_fp_xor_seed() {
        let s = Streams::new(7, RandomMode::Tlc);
        assert_eq!(s.state(0x10), Stream::Java(JavaRandom::new(0x17)));
        assert_eq!(s.startup(), Stream::Java(JavaRandom::new(7)));
    }
}
