---- MODULE ProcessProcedureNames_C_two_blocks ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
process (Other = 2) { O:call First(1); Z:skip; }
} *)
====
