---- MODULE ProcessProcedure_C_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1;
{ F:x:=u; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
