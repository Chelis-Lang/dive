---- MODULE FairProcedure_C_invalid_no_param_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:call First(); return; };
fair process (One = 1)

{ L:call First(); E:skip; }
} *)
====
