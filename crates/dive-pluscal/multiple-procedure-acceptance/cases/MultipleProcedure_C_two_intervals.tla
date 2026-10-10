---- MODULE MultipleProcedure_C_two_intervals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in 1..2)

{ L:call First(1); E:skip; }
process (Two \in 3..4)

{ M:call First(2); N:skip; }
} *)
====
