---- MODULE MultipleProcedure_C_strong_process ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
fair + process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
