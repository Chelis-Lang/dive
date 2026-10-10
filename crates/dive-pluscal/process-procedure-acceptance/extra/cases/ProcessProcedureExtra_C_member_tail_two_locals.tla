---- MODULE ProcessProcedureExtra_C_member_tail_two_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
{ F:if(a>0){ call First(a-1); return; }else{ x:=u+v; }; R:return; };
process (Worker \in {1,2}) { L:call First(2); E:skip; }
} *)
====
