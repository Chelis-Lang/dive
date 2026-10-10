---- MODULE FairProcedure_C_mixed_reach_false_branch ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(FALSE){ call Second(a); }else{ x:=x+a; }; R:return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two \in {2,3})

{ M:call First(2); N:skip; }
} *)
====
