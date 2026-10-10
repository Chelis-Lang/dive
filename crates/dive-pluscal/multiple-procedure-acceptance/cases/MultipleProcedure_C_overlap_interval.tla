---- MODULE MultipleProcedure_C_overlap_interval ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in 1..3)

{ L:call First(1); E:skip; }
process (Two = 3)

{ M:call First(2); N:skip; }
} *)
====
