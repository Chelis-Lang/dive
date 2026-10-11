---- MODULE ProcessProcedureNames_C_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(ProcSet=0)

{ F:x:=ProcSet; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
