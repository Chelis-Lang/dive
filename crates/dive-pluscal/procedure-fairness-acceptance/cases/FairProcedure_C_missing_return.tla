---- MODULE FairProcedure_C_missing_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; };
fair process (One = 1)

{ L:call First(1); E:skip; }
} *)
====
