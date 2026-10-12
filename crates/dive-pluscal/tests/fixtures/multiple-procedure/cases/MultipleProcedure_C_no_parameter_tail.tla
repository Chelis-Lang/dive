---- MODULE MultipleProcedure_C_no_parameter_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:call First(); return; };
process (One = 1)

{ L:call First(); E:skip; }
process (Two = 2)

{ M:call First(); N:skip; }
} *)
====
