---- MODULE FairProcedure_C_constants_reach_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F+:if(a>0){ call First(a-1); return; }else{ x:=x+1; }; R-:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
