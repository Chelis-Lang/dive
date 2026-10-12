---- MODULE MultipleProcedure_C_overlap_constants ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 1)

{ M:call First(2); N:skip; }
} *)
====
