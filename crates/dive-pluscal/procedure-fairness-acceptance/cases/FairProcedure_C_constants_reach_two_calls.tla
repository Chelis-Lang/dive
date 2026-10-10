---- MODULE FairProcedure_C_constants_reach_two_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
fair process (One = 1)

{ L:call Second(1); E:call First(2); Z:skip; }
fair + process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
