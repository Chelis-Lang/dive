---- MODULE ProcessHeader_C_boundary_label_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ First:x:=a; R:return; };
process (Worker = 1)
{ L:call First(2); E:skip; }

} *)
====
