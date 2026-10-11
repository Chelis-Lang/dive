---- MODULE ProcessProcedure_C_tail_parallel_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=1)

{ F:if(a>0){ call First(a-1,a); return; }else{ x:=c; }; R:return; };

process (Worker = 1)

{ L:call First(2,3); E:skip; }
} *)
====
