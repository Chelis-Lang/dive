---- MODULE ProcessProcedure_C_process_local_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:y:=a; R:return; };

process (Worker = 1)
variable y=1;
{ L:call First(2); E:skip; }
} *)
====
