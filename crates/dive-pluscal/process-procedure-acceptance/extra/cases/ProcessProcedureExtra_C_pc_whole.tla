---- MODULE ProcessProcedureExtra_C_pc_whole ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=pc; R:return; };
process (Worker \in {1,2}) { L:call First(2); E:skip; }
} *)
====
