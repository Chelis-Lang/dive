---- MODULE ProcessHeader_C_boundary_duplicate_global ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variables x=0,a=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker = 1)
{ L:call First(2); E:skip; }

} *)
====
