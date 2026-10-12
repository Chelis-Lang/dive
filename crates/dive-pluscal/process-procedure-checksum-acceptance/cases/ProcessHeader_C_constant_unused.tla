---- MODULE ProcessHeader_C_constant_unused ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker = 1)
{ L:x:=1; E:skip; }

} *)
====
