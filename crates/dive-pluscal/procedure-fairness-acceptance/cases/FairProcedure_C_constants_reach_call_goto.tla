---- MODULE FairProcedure_C_constants_reach_call_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
fair process (One = 1)

{ L:call First(1); goto Z; Z:skip; }
fair + process (Two = 2)

{ M:call Second(2); N:skip; }
} *)
====
