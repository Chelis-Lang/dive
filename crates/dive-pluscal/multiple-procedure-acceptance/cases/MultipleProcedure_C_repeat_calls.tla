---- MODULE MultipleProcedure_C_repeat_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)

{ L:call First(1); E:call First(2); Z:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
