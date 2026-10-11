---- MODULE ProcessProcedureFair_C_global ----
EXTENDS Integers, Sequences, TLC
(* --fair algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
