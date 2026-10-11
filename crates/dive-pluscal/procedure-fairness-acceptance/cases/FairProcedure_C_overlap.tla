---- MODULE FairProcedure_C_overlap ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two = 1)

{ M:call First(2); N:skip; }
} *)
====
