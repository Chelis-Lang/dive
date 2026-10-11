---- MODULE FairProcedure_C_alias_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(x=0)

{ F:x:=x+a; R:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
} *)
====
