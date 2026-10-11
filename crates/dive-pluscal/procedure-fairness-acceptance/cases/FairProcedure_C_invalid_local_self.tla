---- MODULE FairProcedure_C_invalid_local_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=self;
{ F:x:=x+u; R:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
} *)
====
