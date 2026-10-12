---- MODULE ProcessProcedure_C_self_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1;
{ F:if(a>0){ call First(a-1); return; }else{ x:=u; }; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)
====
