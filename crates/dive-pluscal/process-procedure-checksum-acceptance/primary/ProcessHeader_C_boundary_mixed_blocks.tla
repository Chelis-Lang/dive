---- MODULE ProcessHeader_C_boundary_mixed_blocks ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker = 1)
{ L:call First(2); E:skip; }
process (Other = 2) { M:call First(3); Z:skip; }
} *)
====
