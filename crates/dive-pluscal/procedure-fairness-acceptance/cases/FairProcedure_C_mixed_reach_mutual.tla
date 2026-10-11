---- MODULE FairProcedure_C_mixed_reach_mutual ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(a>0){ call Second(a-1); }else{ x:=x+1; }; R:return; };
procedure Second(b=0)

{ S:if(b>0){ call First(b-1); }else{ x:=x+1; }; T:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two \in {2,3})

{ M:call Second(2); N:skip; }
} *)
====
