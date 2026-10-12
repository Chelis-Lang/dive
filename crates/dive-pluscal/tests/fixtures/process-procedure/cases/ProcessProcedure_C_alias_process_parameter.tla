---- MODULE ProcessProcedure_C_alias_process_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)
variable a=1;
{ L:call First(2); E:skip; }
} *)
====
