---- MODULE ProcessHeader_C_member_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker \in {1,2})
{ L:call First(2); goto E; E:skip; }

} *)
====
