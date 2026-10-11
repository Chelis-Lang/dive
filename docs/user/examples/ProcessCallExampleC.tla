---- MODULE ProcessCallExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Count(a = 0) {
    Entry: if (a > 0) { call Count(a - 1); } else { x := x + 1; };
    Restore: x := x + 1; return;
  };
  process (Worker = 1) {
    Start: call Count(2); Finish: skip;
  }
} *)
====
