---- MODULE MultipleProcedure_C_two_string_constants ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = "z")

{ L:call First(1); E:skip; }
process (Two = "a")

{ M:call First(2); N:skip; }
} *)
====
