---- MODULE MultipleCallExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Add(a = 0) {
    Entry: x := x + a; Return: return;
  };
  process (One = 1) { First: call Add(1); FinishOne: skip; }
  process (Two \in {2}) { Second: call Add(self); FinishTwo: skip; }
} *)
====
