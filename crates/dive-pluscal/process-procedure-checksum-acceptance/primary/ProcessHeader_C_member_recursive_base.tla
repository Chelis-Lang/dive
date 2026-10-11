---- MODULE ProcessHeader_C_member_recursive_base ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(a>0){call First(a-1); goto R;}else{x:=a; goto R;}; R:return; };
process (Worker \in {1,2})
{ L:call First(2); E:skip; }

} *)
====
