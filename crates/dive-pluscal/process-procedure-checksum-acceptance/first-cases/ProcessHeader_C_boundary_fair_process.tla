---- MODULE ProcessHeader_C_boundary_fair_process ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
fair process (Worker = 1)
{ L:call First(2); E:skip; }

} *)
====
