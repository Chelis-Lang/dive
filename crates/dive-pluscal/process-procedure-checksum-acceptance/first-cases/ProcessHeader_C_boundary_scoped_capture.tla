---- MODULE ProcessHeader_C_boundary_scoped_capture ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker = 1)
{ L:with(a=2){call First(a);} E:skip; }

} *)
====
