---- MODULE ProcessProcedure_C_missing_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
