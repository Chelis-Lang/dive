---- MODULE ProcessProcedure_C_no_params_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:call First(); return; };

process (Worker = 1)

{ L:call First(); E:skip; }
} *)
====
