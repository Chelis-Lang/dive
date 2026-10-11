---- MODULE ProcessHeader_C_member_nested_if ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker \in {1,2})
{ L:if(TRUE){call First(2);}else{x:=0;}; E:skip; }

} *)
====
