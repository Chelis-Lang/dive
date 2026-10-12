---- MODULE ProcessHeader_C_member_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=1;
{ F:x:=u+a; R:return; };
process (Worker \in {1,2})
{ L:call First(2); E:skip; }

} *)
====
