---- MODULE ProcessHeader_C_member_ordinary ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:x:=1; R:return; };
process (Worker \in {1,2})
{ L:call First(); E:skip; }

} *)
====
