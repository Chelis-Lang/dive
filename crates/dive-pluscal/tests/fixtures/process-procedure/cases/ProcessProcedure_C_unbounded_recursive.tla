---- MODULE ProcessProcedure_C_unbounded_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:call First(a+1); R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
