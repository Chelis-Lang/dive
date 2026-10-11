---- MODULE MultipleProcedure_C_no_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:x:=x+1; R:return; };
process (One = 1)

{ L:call First(); E:skip; }
process (Two = 2)

{ M:call First(); N:skip; }
} *)
====
