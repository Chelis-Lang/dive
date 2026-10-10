---- MODULE MultipleProcedure_C_parameter_earlier ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,b=a+1)

{ F:x:=x+a+b; R:return; };
process (One = 1)

{ L:call First(1,2); E:skip; }
process (Two = 2)

{ M:call First(2,3); N:skip; }
} *)
====
