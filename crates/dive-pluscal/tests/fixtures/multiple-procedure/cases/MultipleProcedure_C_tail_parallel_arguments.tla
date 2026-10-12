---- MODULE MultipleProcedure_C_tail_parallel_arguments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,b=1)

{ F:if(a>0){ call First(a-1,a); return; }else{ x:=x+b; }; R:return; };
process (One = 1)

{ L:call First(1,3); E:skip; }
process (Two = 2)

{ M:call First(2,4); N:skip; }
} *)
====
