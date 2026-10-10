---- MODULE MultipleProcedure_C_cross_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:call Second(a); return; };
procedure Second(b=0)

{ S:x:=x+b; T:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
