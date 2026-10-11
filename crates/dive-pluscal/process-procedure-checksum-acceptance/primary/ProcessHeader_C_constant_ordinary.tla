---- MODULE ProcessHeader_C_constant_ordinary ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:x:=1; R:return; };
process (Worker = 1)
{ L:call First(); E:skip; }

} *)
====
