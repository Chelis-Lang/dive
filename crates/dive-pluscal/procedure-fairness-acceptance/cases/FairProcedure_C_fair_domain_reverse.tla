---- MODULE FairProcedure_C_fair_domain_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
fair process (One \in 2..1)

{ L:call First(1); E:skip; }
} *)
====
