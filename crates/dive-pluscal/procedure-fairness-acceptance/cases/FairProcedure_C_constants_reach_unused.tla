---- MODULE FairProcedure_C_constants_reach_unused ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
procedure Third(c=0)

{ U:x:=x+c; V:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
