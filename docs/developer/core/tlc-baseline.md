# TLC per-state baseline

This page checks the claims the core plan makes about what TLC does for each
state, against the pinned TLC source (corpus pin `tlaplus-tests` at commit
`94d0c5070852b37b801bb8998c436383ab7ee37b`, tree
`tlatools/org.lamport.tlatools/src/`). Paths are relative to that tree. Line
numbers refer to that commit. Excerpts are trimmed to six lines or fewer.

Notation: V is the number of state variables, |v| is the size of a value tree
(nodes plus string characters), n is the length of a function, record, tuple or
set, and k is the number of EXCEPT pairs or operator arguments.

The main exploration path is `Worker.run` → `Tool.getNextStates(INextStateFunctor, TLCState)`
→ `Worker.addElement(curState, action, succState)` for each generated successor
(`tlc2/tool/Worker.java:116`, `tlc2/tool/impl/Tool.java:912-922`,
`tlc2/tool/Worker.java:418`). `ModelChecker.doNext` (`tlc2/tool/ModelChecker.java:414`),
which builds a whole `StateVec` first, runs only to replay an error with a call
stack (`tlc2/tool/ModelChecker.java:300`).

## 1. Successor construction

**Verdict: PARTLY.** TLC does not copy the state on each `bind`. It keeps one
mutable working state per (state, action) pair and binds and unbinds variables
in place. It makes one shallow copy (a new `TLCStateMut` and a new `IValue[V]`,
with values shared by reference) at each leaf, that is, once per generated
successor.

- A fresh empty working state is created for each action of each dequeued
  state: `TLCState.Empty.createEmpty().setPredecessor(state).setAction(action)`
  (`tlc2/tool/impl/Tool.java:920-921`). `createEmpty` allocates
  `new IValue[vars.length]` (`tlc2/tool/TLCStateMut.java:79-82`).
- `x' = e` evaluates `e`, then binds, recurses and unbinds
  (`tlc2/tool/impl/Tool.java:1353-1371`):

  ```java
  Value rval = this.eval(args[1], c, s0, s1, EvalControl.Clear, cm);
  if (lval == null) {
    resState.bind(varName, rval);
    resState = this.getNextStates(action, acts, s0, resState, nss, cm);
    resState.unbind(varName);
  ```
- `bind` and `unbind` are a single array store (`tlc2/tool/TLCStateMut.java:103-118`):
  `this.values[loc] = value;` and `this.values[loc] = null;`.
- At the leaf, the working state itself goes to the functor, and a shallow copy
  becomes the new working state (`tlc2/tool/impl/Tool.java:1004-1005`, and the
  same at `:1081-1082` on the all-assigned fast path):

  ```java
  if (acts.isEmpty()) {
    nss.addElement(s0, action, s1);
    return s1.copy();
  ```
- `copy()` copies the pointer array only (`tlc2/tool/TLCStateMut.java:130-135`):

  ```java
  IValue[] vals = new IValue[len];
  System.arraycopy(this.values, 0, vals, 0, len);
  return copy(new TLCStateMut(vals));
  ```
- `UNCHANGED v` binds the predecessor's value object by reference
  (`tlc2/tool/impl/Tool.java:1657-1660`).

**Cost per generated successor:** O(V) for the pointer-array copy, plus
evaluation of every primed right-hand side. Bind and unbind are O(1). The copy
is shallow, so value trees are not duplicated. The copy happens for every
generated successor, whether it turns out to be new or a duplicate.

## 2. State fingerprinting

**Verdict: TRUE.** TLC fingerprints a state by walking every variable's whole
value tree on every call. No value class caches a fingerprint, and the scheme
cannot cache one per subtree: each value extends a running fingerprint that it
receives from its left context, so a subtree's contribution depends on
everything hashed before it.

- State level (`tlc2/tool/TLCStateMut.java:250-254`):

  ```java
  long fp = FP64.New();
  if (viewMap == null) {
    for (int i = 0; i < sz; i++) {
      fp = minVals[i].fingerPrint(fp);
    }
  ```
- `FcnRcdValue.fingerPrint` normalizes, then walks the domain and values
  (`tlc2/value/impl/FcnRcdValue.java:814-823`):

  ```java
  this.normalize();
  fp = FP64.Extend(fp, FCNRCDVALUE);
  fp = FP64.Extend(fp, flen);
  for (int i = 0; i < flen; i++) {
    fp = this.domain[i].fingerPrint(fp);
    fp = this.values[i].fingerPrint(fp);
  ```
- `RecordValue.fingerPrint` re-hashes each field name character by character on
  every call (`tlc2/value/impl/RecordValue.java:691-702`):

  ```java
  String str = this.names[i].toString();
  fp = FP64.Extend(fp, STRINGVALUE);
  fp = FP64.Extend(fp, str.length());
  fp = FP64.Extend(fp, str);
  fp = this.values[i].fingerPrint(fp);
  ```
- `TupleValue` (`tlc2/value/impl/TupleValue.java:326-336`), `SetEnumValue`
  (`tlc2/value/impl/SetEnumValue.java:348-358`, which normalizes first) and
  `StringValue` (`tlc2/value/impl/StringValue.java:192-196`) walk their contents
  in the same way. `IntervalValue` hashes every integer in `low..high`
  (`tlc2/value/impl/IntervalValue.java:266-275`), so an interval fingerprints in
  O(size), not O(1).
- `FP64.Extend` is a byte-at-a-time table update: four steps for an `int`, eight
  for a `long`, one for each `char` of a string (`tlc2/util/FP64.java:61-71`,
  `:144-163`).
- Cache check: the instance fields of the five classes are only data and
  `isNorm` (`FcnRcdValue.java:42-45`, `RecordValue.java:55-57`,
  `TupleValue.java:31`, `SetEnumValue.java:30-31`, `IntervalValue.java:23`).
  There is no fingerprint field. `TLCStateMut` has no cached fingerprint either,
  so `curState.fingerPrint()` is recomputed when liveness is on
  (`tlc2/tool/Worker.java:169`).
- `Value.hashCode()` is a full fingerprint walk
  (`tlc2/value/impl/Value.java:298-303`), so any Java hash container keyed by a
  `Value` pays O(|v|) for each hash.
- Unchanged variables: the value object is shared (claim 1), but the state loop
  above still walks it in full. Yes, the fingerprint is recomputed for unchanged
  variables.
- With symmetry, each permutation is applied to every variable and compared
  lexicographically, in O(|perms| · Σ|v|) (`tlc2/tool/TLCStateMut.java:205-246`).
  With a VIEW, the view expression is evaluated for each state
  (`tlc2/tool/TLCStateMut.java:260-269`).

**Cost per generated successor:** Θ(Σ over variables of |v|), with string
characters counted individually. This is paid for duplicates too (claim 3).

## 3. Duplicates are fully materialized

**Verdict: TRUE.** Every generated successor is fully built and passes through
constraint evaluation and a full fingerprint before the seen-set lookup. Part of
the work after the lookup also runs for duplicates.

Order in `Worker.addElement` (`tlc2/tool/Worker.java:418-476`):

1. The whole action is evaluated and every variable is bound (claim 1) before
   `addElement` is called.
2. `statesGenerated++` (`:420`).
3. `isGoodState`, an O(V) all-assigned scan (`:423`, `tlc2/tool/impl/Tool.java:2867-2868`).
4. State constraints and action constraints are evaluated (`:434-435`):

   ```java
   final boolean inModel = (this.tool.isInModel(succState.setPredecessor(curState).setAction(action))
       && this.tool.isInActions(curState, succState));
   ```
5. The fingerprint and the seen-set insert, both in `isSeenState`
   (`tlc2/tool/Worker.java:524-525`):

   ```java
   final long fp = succState.fingerPrint(tool);
   final boolean seen = this.theFPSet.put(fp);
   ```
6. Only if the state is new: a trace write (`:537`), invariants (`:459-463`) and
   the enqueue (`:475`).
7. For every successor, including duplicates: implied-action (action property)
   checks (`:465-469`: "We need to do it even if succState is not new"), and
   after `addElement` returns, the `s1.copy()` from claim 1.

So steps 1 to 5 and the copy run for every duplicate, and so do implied actions
when any are configured. `theFPSet.put` is a combined test-and-insert, so there
is one seen-set operation for each successor.

**Cost per duplicate:** action evaluation, plus O(V), plus constraint
evaluation, plus Θ(Σ|v|) for the fingerprint, plus one FPSet probe, plus O(V)
for the copy, plus implied-action evaluation.

## 4. EXCEPT

**Verdict: TRUE.** Every EXCEPT update copies the whole backing array of each
function, record or tuple on its path: O(n) per level, per pair.

- `FcnRcdValue.takeExcept` (`tlc2/value/impl/FcnRcdValue.java:507-511`):

  ```java
  int flen = this.values.length;
  Value[] newValues = new Value[flen];
  for (int i = 0; i < flen; i++) {
    newValues[i] = this.values[i];
  }
  ```
  With an interval domain the index is direct (`:514-530`). Otherwise TLC finds
  the key by a linear scan with deep `equals` (`:535-536`,
  `if (arg.equals(this.domain[i]))`), so a non-interval update costs
  O(n · |key|). If the function is not normalized, the domain array is copied
  too (`:540-545`).
- `RecordValue.takeExcept` allocates and fills a new `values` array of every
  field (`tlc2/value/impl/RecordValue.java:389-402`), and copies `names` if not
  normalized (`:403-409`).
- `TupleValue.takeExcept` copies all elements (`tlc2/value/impl/TupleValue.java:188-200`).
- A multi-pair EXCEPT applies the pairs one after another, and each pair copies
  again (`tlc2/value/impl/FcnRcdValue.java:559-566`, and the evaluator loop at
  `tlc2/tool/impl/Tool.java:2305-2324`).
- Each pair also evaluates `@` through `result.select(lhs)` and allocates a
  `Value[]` for the path, a `Context` cell for `@` and a `ValueExcept`
  (`tlc2/tool/impl/Tool.java:2310-2322`).

**Cost:** Σ over path levels of O(n_level), per pair. For `[f EXCEPT ![a]=x, ![b]=y]`
over n entries, that is 2·O(n) work and two arrays of n entries.

## 5. Set membership

**Verdict: PARTLY.** `SetEnumValue.member` does not normalize. It uses a binary
search with `compareTo` only if the set is already normalized, and otherwise a
linear scan with `equals`. There is no hashing anywhere. Interval, function-set
and powerset membership are symbolic.

- `SetEnumValue.member` (`tlc2/value/impl/SetEnumValue.java:146-148`) calls
  `this.elems.search(elem, this.isNorm)`. `ValueVec.search`
  (`tlc2/value/impl/ValueVec.java:144-168`):

  ```java
  if (sorted) {
    ... cmp = elem.compareTo(this.elementData[mid]); ...
  } else {
    for (int i = 0; i < this.elementCount; i++) {
      if (this.elementData[i].equals(elem)) {
  ```
  This costs O(log n · cmp) if normalized and O(n · eq) if not. A set literal
  `{a, b, c}` is built anew, unnormalized, on every evaluation
  (`tlc2/tool/impl/Tool.java:2428-2435`, `new SetEnumValue(vals, false, cm)`),
  unless it is a constant-level definition precomputed at startup (claim 7).
  Normalizing is a binary insertion sort with O(n²) element moves in the worst
  case (`tlc2/value/impl/ValueVec.java:172-197`).
- `IntervalValue.member` is O(1) (`tlc2/value/impl/IntervalValue.java:77-79`):
  `return (x >= low) && (x <= high);`.
- `SetOfFcnsValue.member` (`[D -> R]`) is symbolic: it compares domains, then
  tests each value against R (`tlc2/value/impl/SetOfFcnsValue.java:124-141`). For
  a non-interval domain it normalizes the candidate and allocates a
  `SetEnumValue` over its domain to compare (`:125-127`). Cost:
  O(|D| · cost(R.member)), with no enumeration of `[D -> R]`.
- `SubsetValue.member` (`SUBSET S`) is symbolic: it enumerates the candidate's
  elements, not the powerset, and tests each against S
  (`tlc2/value/impl/SubsetValue.java:94-101`). Cost: O(|x| · cost(S.member)).
- `SetPredValue.member` checks the base set, then evaluates the predicate with a
  new `Context` binding (`tlc2/value/impl/SetPredValue.java:112-124`).
- `e \in S` in an expression evaluates S to a value, then calls
  `arg2.member(arg1)` (`tlc2/tool/impl/Tool.java:2670-2674`).

## 6. Equality

**Verdict: TRUE.** `equals` and `compareTo` are deep structural comparisons that
normalize both sides first. There is no identity short-circuit in the compound
classes and no interning of compound values. Only strings (and identifiers) are
interned.

- `FcnRcdValue.equals` converts the other side with `toFcnRcd()`, normalizes
  both, then compares element by element
  (`tlc2/value/impl/FcnRcdValue.java:262-278`). `compareTo` does the same
  (`:151-171`).
- `SetEnumValue.equals` (`tlc2/value/impl/SetEnumValue.java:119-136`) and
  `RecordValue.equals` (`tlc2/value/impl/RecordValue.java:338-352`) follow the
  same pattern.
- No `this == obj` fast path: searching `tlc2/value/impl/*.java` for
  `this == obj`, `obj == this` or `== other` finds only `KSubsetValue`'s scalar
  field comparison (`KSubsetValue.java:140`).
- Strings are interned: `StringValue` holds a `UniqueString`, and its equality
  and order compare the intern token (`util/UniqueString.java:227-240`):

  ```java
  public int compareTo(UniqueString uniqueString) {
      return this.tok - uniqueString.tok;
  }
  public boolean equals(UniqueString t) {
      return this.tok == t.tok;
  ```
  String order is therefore intern order, not lexicographic order.
- Small integers: only 0..9 are cached (`tlc2/value/impl/IntValue.java:20-26`,
  `:66-70`). Every other `IntValue` result is a new object.
- No hash-consing: no value class or factory looks up structurally equal
  compound values. Records, functions, tuples and sets are always built with
  `new` (for example `tlc2/tool/impl/Tool.java:2405`, `:2435`).

**Cost:** O(min(|a|, |b|)) per comparison in the common case, plus a
normalization that is O(n log n) to O(n²) the first time a value is compared,
because normalization mutates the value and sets `isNorm`.

## 7. Invariants

**Verdict: TRUE.** Each configured invariant is evaluated in full, from its root
expression, on every new distinct state. Nothing is memoized across states, and
TLC does not split an invariant into separately cached conjuncts.

- Configuration: each `INVARIANT` name becomes one `Action` over the whole
  definition body (`tlc2/tool/impl/SpecProcessor.java:1029`):
  `this.invVec.addElement(new Action(def.getBody(), Context.Empty, def));`.
- Checking runs only for new states (`tlc2/tool/Worker.java:459-463`) and loops
  over all invariants (`:552-554`):

  ```java
  for (k = 0; k < this.tool.getInvariants().length; k++)
  {
      if (!tool.isValid(this.tool.getInvariants()[k], succState))
  ```
- `isValid` is a plain `eval` of the predicate (`tlc2/tool/impl/Tool.java:3605-3616`).
- The only reuse is at startup: zero-arity, constant-level definitions are
  evaluated once and stored as tool objects
  (`tlc2/tool/impl/SpecProcessor.java:349-361`). Any state-dependent
  subexpression, such as `TypeOK`'s `x \in [D -> R]`, is evaluated again for each
  state.
- Implied actions (action properties) are evaluated for every successor,
  duplicates included (claim 3).

**Cost per distinct state:** Σ over invariants of a full evaluation.

## 8. Lazy values and INSTANCE substitution

**Verdict: TRUE.** Each `LazyValue` is a per-call thunk. Its cache lives only as
long as that object, and each cache hit is guarded by a check that is linear in
the state. INSTANCE substitutions are re-bound by consing onto the `Context`
list each time an instantiated expression is evaluated, and identifier lookup
walks that linked list.

- Creation: a new `LazyValue` is made for every expression argument of every
  user-defined operator call (`getOpContext` → `getVal`,
  `tlc2/tool/impl/SymbolNodeValueLookupProvider.java:124-146`):

  ```java
  if (expr instanceof ExprNode) {
      return new LazyValue(expr, c, cachable, cm);
  }
  ...
  c1 = c1.cons(formals[i], aval);
  ```
- Cache read: `getCachedValue` returns the cached value only when the tool id
  matches, both states are "subsets" of the states it was computed in, and the
  control is equivalent (`tlc2/value/impl/LazyValue.java:143-150`).
  `TLCState.isSubset` compares every variable with deep `equals`, unless the two
  are the same object (`tlc2/tool/TLCState.java:246-270`). A hit therefore costs
  up to O(Σ|v|) unless the state objects are identical.
- Cache write: `getValue` evaluates, then stores `s0.copy()` and `s1.copy()`,
  which are two shallow state copies for each cached value
  (`tlc2/value/impl/LazyValue.java:169-180`).
- Uncachable: thunks built with `cachable == false`, or with
  `-Dtlc2.value.LazyValue.off`, get `ValUndef` and are evaluated again on every
  use (`tlc2/value/impl/LazyValue.java:119-121`).
- INSTANCE in expressions: each evaluation of a `SubstInNode` conses one
  `Context` cell and one new `LazyValue` per substitution
  (`tlc2/tool/impl/Tool.java:1864-1872`):

  ```java
  for (int i = 0; i < slen; i++) {
    Subst sub = subs[i];
    c1 = c1.cons(sub.getOp(), this.getVal(sub.getExpr(), c, true, ...));
  }
  return this.eval(expr1.getBody(), c1, s0, s1, control, cm);
  ```
- INSTANCE in next-state generation uses `cachable = false`
  (`tlc2/tool/impl/Tool.java:965-974`), so substituted expressions inside actions
  are evaluated again at every reference.
- Lookup: `Context.lookup` walks the linked list by `SymbolNode` identity
  (`tlc2/util/Context.java:74-88`), and falls back to the node's tool object
  (`SymbolNodeValueLookupProvider.java:87-117`). The cost is O(depth of the
  context chain) per identifier reference.

## 9. Parallelism

**Verdict: PARTLY.** The FPSet is not globally locked. The default uses striped
read-write locks, and an opt-in variant is lock-free with CAS. The state queue
is one global monitor. The trace is written per worker, under each worker's own
monitor. The class `LongArrayFPSet` does not exist in this source. The
lock-free table is `LongArray`, used by `OffHeapDiskFPSet`.

**FPSet.**
- Default: the implementation defaults to `MSBDiskFPSet`
  (`tlc2/tool/fp/FPSetFactory.java:163-164`, `FPSetConfiguration.java:29-33`).
  A disk FPSet forces `fpBits = 1` (`FPSetConfiguration.java:54-58`), so the
  factory returns a `MultiFPSet` of two sets (`FPSetFactory.java:96-106`), and
  the top bit selects the subset (`tlc2/tool/fp/MultiFPSet.java:103-108`, `:118-119`).
- `MSBDiskFPSet` extends `HeapBasedDiskFPSet` (`MSBDiskFPSet.java:18`), which
  uses striped `ReentrantReadWriteLock`s (`tlc2/util/Striped.java:9-18`). There
  are 2^(⌊log2 workers⌋ + 8) stripes (`tlc2/tool/fp/HeapBasedDiskFPSet.java:25`, `:76-77`).
  `put` takes a read lock for the memory and disk lookup, then the stripe's
  write lock to insert into a bucket with a linear scan
  (`HeapBasedDiskFPSet.java:251-290`, `:212-225`):

  ```java
  final Lock readLock = rwLock.getAt(getLockIndex(fp0)).readLock();
  readLock.lock();
  if (this.memLookup(fp0)) { ... return true; }
  boolean diskHit = this.diskLookup(fp0);
  ...
  final Lock w = rwLock.getAt(getLockIndex(fp0)).writeLock();
  ```
  When the table fills, one thread acquires all stripes and flushes to disk.
  This stops every worker (`HeapBasedDiskFPSet.java:297-311`,
  `rwLock.acquireAllLocks(); flusher.flushTable();`). Disk lookups also take
  `synchronized (this.brafPool)` to borrow a file handle
  (`tlc2/tool/fp/DiskFPSet.java:506-523`).
- Opt-in (`-Dtlc2.tool.fp.FPSet.impl=...OffHeapDiskFPSet`): this variant is
  lock-free, using open addressing with CAS and up to `PROBE_LIMIT` = 1024
  probes (`tlc2/tool/fp/OffHeapDiskFPSet.java:116`, `:310-333`; CAS through
  `Unsafe.compareAndSwapLong` in `tlc2/tool/fp/LongArray.java:213-215`):

  ```java
  if (expected == EMPTY || (expected < 0 && fp0 != (expected & FLUSHED_MASK))) {
      if (array.trySet(position, expected, fp0)) {
          this.tblCnt.increment();
          return false;
  ```
  Eviction is a `Phaser` barrier that blocks every inserter
  (`OffHeapDiskFPSet.java:58-71`, `:236-241`, `:337-346`).
- The `MemFPSet*` variants are fully `synchronized` (`MemFPSet.java:109`,
  `MemFPSet1.java:48`, `MemFPSet2.java:84`), but they are not used by default.

**State queue.**
- Default `DiskStateQueue` (`tlc2/tool/queue/IStateQueue.java:21-39`;
  `ModelChecker.java:113`). `sEnqueue` and `sDequeue` are `synchronized` on the
  one queue object, and enqueue calls `notifyAll` when a worker is waiting
  (`tlc2/tool/queue/StateQueue.java:72-78`, `:134-145`):

  ```java
  public final synchronized void sEnqueue(final TLCState state) {
      this.enqueueInner(state);
      this.len++;
      if (this.numWaiting > 0 && !this.stop) {
          this.notifyAll();
  ```
- Workers dequeue one state at a time (`tlc2/tool/Worker.java:96`). Each new
  state costs one global monitor acquisition to enqueue and one to dequeue.
- A spill to disk happens inside the queue monitor: `enqueueInner` calls
  `StatePoolWriter.doWork` (`tlc2/tool/queue/DiskStateQueue.java:86-102`).
  `doWork` is `synchronized` and serializes the previous buffer on the calling
  thread if the background writer has not taken it yet
  (`tlc2/util/StatePoolWriter.java:47-63`).
- `DiskByteArrayQueue` (opt-in) is likewise built on `synchronized` methods in
  `ByteArrayQueue` (`tlc2/tool/queue/ByteArrayQueue.java:121-226`).

**Trace.**
- `ModelChecker` uses `ConcurrentTLCTrace` (`tlc2/tool/ModelChecker.java:116`).
  Each `Worker` has its own `BufferedRandomAccessFile`
  (`tlc2/tool/Worker.java:59`, `:84`), and `Worker.writeState` is `synchronized`
  on that worker. The lock is uncontended except during trace reconstruction.
  Each new state writes a predecessor pointer, a worker id and a fingerprint
  (`tlc2/tool/Worker.java:301-312`):

  ```java
  this.lastPtr = this.raf.getFilePointer();
  this.raf.writeLongNat(curState.uid);
  this.raf.writeShortNat(curState.workerId);
  this.raf.writeLong(sucStateFp);
  ```
- The single global `TLCTrace` (all methods `synchronized`,
  `tlc2/tool/TLCTrace.java:53-86`) is used only by the distributed `TLCServer`
  (`tlc2/tool/distributed/TLCServer.java:183`).

## 10. Allocation per generated successor

**Verdict: estimate from code paths.** These are not profiled counts.

| Source | Objects | When | Evidence |
|---|---|---|---|
| Working state for each (state, action) | 2 (`TLCStateMut` + `IValue[V]`) | per action per state, amortized over its successors | `Tool.java:920-921`, `TLCStateMut.java:79-82` |
| Leaf `s1.copy()` | 2 (`TLCStateMut` + `IValue[V]`) | every generated successor, duplicates included | `Tool.java:1005`, `TLCStateMut.java:130-135` |
| Conjunction continuation | 1 `ActionItemList` per conjunct after the first | each time a conjunction is entered | `Tool.java:1189-1196`, `tlc2/tool/impl/ActionItemList.java:52-54` |
| `x' = e` with scalar `e` | 0 to 1 (`IntValue` outside 0..9) | per primed assignment | `IntValue.java:66-70` |
| `UNCHANGED v` | 0 | value shared | `Tool.java:1657-1660` |
| User operator call with k arguments | 2k (`Context` + `LazyValue`) | per call | `SymbolNodeValueLookupProvider.java:124-146` |
| LazyValue cache store | 4 (2 × (`TLCStateMut` + array)) | the first time each thunk is forced | `LazyValue.java:176-177` |
| INSTANCE substitution | 2 per substitution (`Context` + `LazyValue`) | per evaluation of the instantiated expression | `Tool.java:965-974`, `:1864-1872` |
| `\E x \in S` | enumerator plus 1 `Context` per element | per enumeration | `Tool.java:1219-1240` |
| EXCEPT pair, depth d | about 3 + 2d (path `Value[]`, `@` `Context`, `ValueExcept`, plus a new array and value object per level) | per pair | `Tool.java:2310-2323`, `FcnRcdValue.java:508`, `:530` |
| Record or set literal | 2 to 3 (arrays + value) | per evaluation | `Tool.java:2394-2405`, `:2428-2435` |
| Fingerprint | 0 without symmetry or VIEW; with symmetry, permuted values plus `IValue[V]` per permutation | per generated successor | `TLCStateMut.java:205-246` |
| New state: queue and trace | 0 (stores a reference, writes primitives) | new states only | `DiskStateQueue.java:102`, `Worker.java:301-312` |

A typical action, a conjunction of a few primed assignments with one EXCEPT on a
function and one or two operator calls, allocates roughly 10 to 30 objects per
generated successor. A fixed floor of 2 (the leaf copy) applies even to
duplicates. Operator arguments, INSTANCE and EXCEPT dominate the variable part.

## Summary

| # | Claim | Verdict | Implication for a faster design |
|---|---|---|---|
| 1 | Full `TLCStateMut` copy per successor via copy/bind/unbind | PARTLY: in-place bind/unbind, one shallow O(V) copy per generated successor | A pointer-array copy is cheap. The waste is that it is paid for duplicates. Represent the successor as a delta over the parent until it is known to be new |
| 2 | Fingerprint walks every value tree; no caching | TRUE: prefix-chained FP64, no cached fingerprint, unchanged variables rehashed, field names rehashed by character | Use a compositional per-value hash, cached on interned values, and combine per-variable hashes so unchanged variables cost O(1) |
| 3 | Duplicates fully materialized before FPSet lookup | TRUE: evaluation, constraints, full fingerprint, then put; copy and implied actions also run for duplicates | Compute the fingerprint incrementally from the delta and probe the seen-set before materializing or copying |
| 4 | EXCEPT copies the whole array, O(n) | TRUE, plus a linear `equals` key search for non-interval domains and one copy per pair | Use persistent or chunked arrays, or in-place update when uniquely owned, plus hashed or indexed domains. Batch multi-pair EXCEPT into one copy |
| 5 | Set membership by normalize plus binary search | PARTLY: binary search only if already normalized, else linear `equals`; no hashing; Interval, `[D -> R]` and `SUBSET` are symbolic | Keep the symbolic forms. Make enumerated sets canonical at construction and use hashed membership on interned elements |
| 6 | Deep equals/compareTo; no hash-consing | TRUE: only strings are interned (token compare), ints 0..9 cached, no identity fast path | Interning gives O(1) equality and makes caches keyable |
| 7 | Invariants re-evaluated in full on each new state; no memo | TRUE: whole body per invariant, new states only; only constant-level definitions are precomputed | Memoize state-independent and per-variable subterms, and skip conjuncts whose read-set is unchanged from the parent |
| 8 | LazyValue caching; INSTANCE resolved dynamically via Context | TRUE: per-call thunks, O(state) guard on each hit, two state copies on store; INSTANCE conses Context per evaluation; linked-list lookup | Resolve substitutions and parameters statically to slots at compile time, so laziness is needed only where priming demands it |
| 9 | FPSet, queue and trace synchronization | PARTLY: FPSet is striped (default) or CAS (opt-in) with stop-the-world flush; queue is one global monitor with single-state dequeue; trace is per worker | The global queue is TLC's clearest scaling limit. Use per-worker queues or batched or work-stealing frontiers. A lock-free seen-set already exists, so match it without global flush pauses |
| 10 | Allocation per successor | about 10 to 30 objects typical; floor of 2 even for duplicates | An arena or handle-based value store and delta successors remove most of it |

## Opportunities TLC leaves on the table

Each item is supported by the evidence above.

- **Deduplicate before materializing.** Successor construction, constraints, the
  full fingerprint and the shallow copy all run for duplicates (claims 1 and 3).
  An incremental fingerprint over a delta lets duplicates exit after a hash
  update and a seen-set probe.
- **Incremental, cacheable fingerprints.** FP64 is chained left to right, so
  TLC cannot cache subtree hashes, and it re-walks unchanged variables, field
  names and the integers of an interval every time (claim 2). A compositional
  hash per value, cached once per interned value and combined per variable, makes
  unchanged variables free.
- **Interning and hash-consing of compound values.** Only strings are interned
  (claim 6). Interning would turn deep `equals`, `compareTo` and `isSubset`
  guards into pointer comparisons, and would make memo keys cheap.
- **Cheaper EXCEPT.** Whole-array copy per pair and per level, plus a linear
  domain search for non-interval domains (claim 4).
- **Hashed membership for enumerated sets.** TLC uses binary search if the set
  is normalized and a linear scan otherwise, and rebuilds set literals on every
  evaluation (claim 5).
- **Static resolution of INSTANCE and operator parameters.** TLC allocates a
  `Context` and a `LazyValue` per argument per call and per substitution per
  evaluation, and uses `cachable = false` inside actions (claim 8).
- **Invariant reuse across states.** Each invariant is evaluated whole on every
  new state, with no conjunct-level or read-set-based reuse (claim 7).
- **Implied actions on duplicates.** TLC evaluates them for every successor
  (claim 3). This is semantically required (the transition is new even when the
  state is not), but a memo keyed on (parent, successor) fingerprints could skip
  repeats.
- **Frontier concurrency.** Every new state crosses one global monitor twice.
  Spills can serialize while that monitor is held (claim 9).
- **No stop-the-world seen-set flush.** Both FPSet variants pause every worker
  to evict to disk (claim 9).
- **Allocation.** There is a floor of two objects per generated successor, plus
  about two per operator argument and three or more per EXCEPT pair (claim 10).
