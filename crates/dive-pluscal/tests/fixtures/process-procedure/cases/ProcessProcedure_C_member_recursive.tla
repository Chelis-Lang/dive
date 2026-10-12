---- MODULE ProcessProcedure_C_member_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(a>0){ call First(a-1); }else{ x:=a; }; R:return; };

process (Worker \in {1,2})

{ L:call First(2); E:skip; }
} *)
====
