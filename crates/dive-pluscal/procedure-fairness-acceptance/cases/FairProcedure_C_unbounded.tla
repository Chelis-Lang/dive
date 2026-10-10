---- MODULE FairProcedure_C_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
fair process (One \in Nat)

{ L:call First(1); E:skip; }
} *)
====
