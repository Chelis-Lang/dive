---- MODULE MultipleProcedure_C_mutual_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:if(a>0){ call Second(a-1); return; }else{ x:=x+1; }; R:return; };
procedure Second(b=0)

{ S:if(b>0){ call First(b-1); return; }else{ x:=x+1; }; T:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
