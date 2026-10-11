---- MODULE ProcessHeader_C_member_nested_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=a; R:return; };
process (Worker \in {1,2})
{ L:with(t=2){call First(t);} E:skip; }

} *)
====
