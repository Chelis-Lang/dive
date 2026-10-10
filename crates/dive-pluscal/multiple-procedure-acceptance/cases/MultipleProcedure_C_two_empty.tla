---- MODULE MultipleProcedure_C_two_empty ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {})

{ L:call First(1); E:skip; }
process (Two \in 2..1)

{ M:call First(2); N:skip; }
} *)
====
