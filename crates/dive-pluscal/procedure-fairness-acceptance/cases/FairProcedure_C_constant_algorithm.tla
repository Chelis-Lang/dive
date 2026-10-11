---- MODULE FairProcedure_C_constant_algorithm ----
EXTENDS Integers, Sequences, TLC
(* --fair algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
} *)
====
