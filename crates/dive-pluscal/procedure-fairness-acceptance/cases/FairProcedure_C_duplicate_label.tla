---- MODULE FairProcedure_C_duplicate_label ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
fair process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two = 2)

{ L:call First(2); E:skip; }
} *)
====
