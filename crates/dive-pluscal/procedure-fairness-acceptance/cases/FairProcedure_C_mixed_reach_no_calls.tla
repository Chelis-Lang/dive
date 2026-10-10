---- MODULE FairProcedure_C_mixed_reach_no_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
fair process (One = 1)

{ L:x:=x+1; E:skip; }
fair + process (Two \in {2,3})

{ M:x:=x+2; N:skip; }
} *)
====
