---- MODULE ProcedureExampleC ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
  variable x = 0;
  procedure Put(a = 0)
  variable b = a + 1;
  { Entry: x := b; Return: return; };
  { Start: call Put(2); Finish: skip; }
} *)
====
