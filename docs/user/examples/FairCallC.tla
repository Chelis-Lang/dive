---- MODULE FairCallC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Add(a = 0) { Entry: + x := x + a; Finish: return; };
  fair process (One = 1) { Start: call Add(1); DoneOne: skip; }
} *)
====
