---- MODULE MultipleProcedure_C_three_interleaved ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {3})

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
process (Three \in {1})

{ O:call First(self); P:skip; }
} *)
====
