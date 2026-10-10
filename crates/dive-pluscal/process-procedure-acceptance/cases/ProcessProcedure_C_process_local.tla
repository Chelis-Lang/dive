---- MODULE ProcessProcedure_C_process_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };

process (Worker = 1)
variable y=1;
{ L:call First(y); E:y:=x; }
} *)
====
