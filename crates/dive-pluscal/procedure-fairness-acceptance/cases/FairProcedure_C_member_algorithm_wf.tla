---- MODULE FairProcedure_C_member_algorithm_wf ----
EXTENDS Integers, Sequences, TLC
(* --fair algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {1,2})

{ L:call First(1); E:skip; }
} *)
====
