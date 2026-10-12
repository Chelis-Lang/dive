---- MODULE ProcessProcedureExtra_C_member_two_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=a+1)

{ F:x:=a+c; R:return; };
process (Worker \in {1,2}) { L:call First(2,3); E:skip; }
} *)
====
