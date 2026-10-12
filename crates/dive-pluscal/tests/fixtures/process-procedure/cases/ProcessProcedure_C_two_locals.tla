---- MODULE ProcessProcedure_C_two_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
{ F:x:=u+v; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
