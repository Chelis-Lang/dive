# cfg acceptance against TLC

`dive-config` must read every `.cfg` file exactly as the pinned TLC does: it must accept
and reject the same files, build the same tables, and fail with the same `EC` code and
line. The oracle is TLC's own reader, `tlc2.tool.impl.ModelConfig`, in the pinned
`tla2tools.jar` (upstream `94d0c50`, SHA-256 `7beec0f0…ea78d`).

## Files

| File | What it holds |
|---|---|
| `CfgDump.java` | Calls `ModelConfig.parse()` on each file and prints what it built, or the error it threw, as one canonical JSON line. It does not reimplement anything. |
| `regenerate.sh` | Compiles `CfgDump` against the jar and regenerates the four `.jsonl` files below. |
| `make_specs.py` | Writes the mutant specs in `tests/fixtures/specs/` (a module and a cfg each). |
| `make_parse_cases.py` | Writes the parse-only cases in `tests/fixtures/modelconfig/`. |
| `../tests/corpus/examples.jsonl` | TLC's reading of every cfg in tlaplus/Examples at `6a7b26a`, with the cfg text. |
| `../tests/corpus/tlaplus.jsonl` | The same for every cfg in tlaplus/tlaplus at `94d0c50` (including `test-model/`). |
| `../tests/fixtures/specs.jsonl`, `modelconfig.jsonl` | The same for the mutants and parse cases. |
| `../tests/fixtures/tlc/<oracle>/` | Full TLC runs of the mutant specs, captured with `dive-conformance dev capture`. |

`tests/tlc_oracle.rs` renders dive's result in `CfgDump`'s format and compares strings.
It needs no Java. It also checks:
- the captured runs: the same `EC` code, exit status, line and message parameter;
- the 141 small-tier goldens: every cfg is accepted, and each violated invariant or
  property that TLC names is one that dive read.

## Regenerating

The pinned JRE is Temurin 17 and has no compiler, so `regenerate.sh` needs a JDK 17 or
later `javac`; it compiles with `javac --release 17`, because the pinned JRE cannot
load class files built for a newer release (a JDK 21 `javac` without `--release 17`
fails with `UnsupportedClassVersionError`). Use, for example, a Temurin 17 JDK
checked against Adoptium's published SHA-256. Check the jar's SHA-256 first.

```sh
crates/dive-config/acceptance/regenerate.sh \
  "$DIVE_CACHE_DIR/sha256/7beec0f0…/tla2tools.jar" <jdk>/bin/javac <pinned-jre>/bin/java \
  <examples checkout at 6a7b26a> <tlaplus checkout at 94d0c50>
```

To recapture the TLC runs:

```sh
cargo run -p dive-conformance --release -- dev capture --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-config/tests/fixtures/specs --out crates/dive-config/tests/fixtures/tlc <Name>...
```

Never edit the `.jsonl` files or transcripts by hand.

## Results (2026-10-08)

| Corpus | cfg files | Both accept | Both reject | dive only accepts | TLC only accepts | Tables or error differ |
|---|---|---|---|---|---|---|
| tlaplus/Examples `6a7b26a` | 235 | 233 | 2 | 0 | 0 | 0 |
| tlaplus `test-model/` `94d0c50` | 530 | 528 | 2 | 0 | 0 | 0 |
| tlaplus, all other cfg files | 121 | 121 | 0 | 0 | 0 | 0 |
| Mutant specs (full TLC runs) | 30 | 9 | 21 | 0 | 0 | 0 |
| Parse-only cases | 69 | 28 | 41 | 0 | 0 | 0 |

"Tables or error differ" compares everything:
- every keyword's value;
- every list;
- constant entries in order, including module-scoped entries and substitutions;
- model values in creation order;
- for rejected files, the `EC` code, line and message parameter.

The only thing not compared is the column of a lexical error.

The rejected corpus files:
- Examples `Disruptor/APDisruptor_MPMC.cfg` and `APDisruptor_SPMC.cfg`, which are
  Apalache-only configs with `NULL = -1`;
- tlaplus `test-model/Github1455.cfg`, an unterminated string;
- tlaplus `test-model/Github432.cfg`, which contains `{%1%}`.

## Surprising TLC cfg behaviours

Each one has a minimal case. Names in the first column are under `tests/fixtures/specs/`,
with a full TLC run. Names in `code` font are under `tests/fixtures/modelconfig/`.

| Case | TLC behaviour |
|---|---|
| `CfgNegativeInt` | `N = -1` is not a negative integer. `-` becomes a model value, and `1` then starts a new entry with no `=`, so TLC fails with 5006 "expecting = or <-" on the *next* line. Negative integers cannot be written in a cfg at all. |
| `CfgTuple` | `N = <<1, 2>>`: `<<` becomes a model value, then 5006. There are no tuples, records or functions in cfg values. |
| `CfgBigInt`, `CfgHexInt`, `octal_number` | An integer above 2^31−1, or one written as `\h1F`, `\o17` or `\b101` (all valid TLA+ number tokens), throws `NumberFormatException`. That is reported as `GENERAL` (1000) with exit 255, not as a config error with exit 151. |
| `CfgStringRaw` | String values keep their escapes verbatim: `"a\nb"` is four characters, and `ASSUME N = "a\\nb"` holds. |
| `CfgCommaList` | `INVARIANT Inv, Inv` is accepted. The comma is an invariant name, so TLC later fails with "The invariant , … is not defined" (2229, exit 151). |
| `CfgKeywordAsValue` | `INIT NEXT` sets the init predicate to `NEXT`. A single-valued keyword takes the next token, whatever it is. |
| `CfgLowercaseKeyword` | `init Init` after a `CONSTANT` section is read as a constant entry `init`, so the error is 5006 "expecting = or <-", not "a keyword". |
| `CfgSetTrailingComma` | `{1,}}` is accepted as `{1, }}`: after a comma the next token is always a value, here the model value `}`. |
| `CfgDuplicateConstant` | Two assignments to `N` are both kept, and the later one wins. Two `INIT` lines are an error (5004). |
| `CfgRepeatedSection` | Repeated list sections (`INVARIANT` twice) append. |
| `CfgUnicodeArrow` | `←` is not `<-` (TLC compares token text), so the result is 5006. |
| `CfgLexBom` | A UTF-8 byte-order mark is a lexical error (5007). |
| `CfgMissingOverride`, `CfgMissingValue` | At end of file, TLC names the line of the newline it appends to the input, so the line is one past the last line when the file ends with a newline. |
| `CfgUnclosedArgs`, `args_section_line` | `CFG_GENERAL` (5002) for unclosed `F(...` names the line of the `CONSTANT` keyword, not of the error. |
| `args_empty` | `F() = 2` is 5002: `)` is read as an argument value. |
| `unterminated_nested_comment` | An unterminated `(*` is 5007 at the line after the end of file. Before `d17f1d35e` this, and every other lexical error, silently ended the file. |
| `private_keywords` | `_PERIODIC`, `_RL_REWARD` and `_POSSIBLE` are cfg keywords. They are undocumented, and a name equal to one of them ends a section. |
| `value_keyword` | `X = INVARIANT` makes `INVARIANT` a model value, because keywords end a section only in name position. |

## Where dive refuses and TLC does not

dive refuses sets nested more than 1024 deep (`DIVE-C0030`). TLC recurses and stops
with a stack overflow at a depth that depends on `-Xss`. No corpus file comes close.
