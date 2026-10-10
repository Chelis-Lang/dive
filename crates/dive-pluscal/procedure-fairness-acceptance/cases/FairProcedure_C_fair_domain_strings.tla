---- MODULE FairProcedure_C_fair_domain_strings ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
fair process (One \in {"z","a"})

{ L:call First(1); E:skip; }
} *)
====
