---- MODULE ProcessHeader_C_member_two_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,b=0)

{ F:x:=a+b; R:return; };
process (Worker \in {1,2})
{ L:call First(2,3); E:skip; }

} *)
====
