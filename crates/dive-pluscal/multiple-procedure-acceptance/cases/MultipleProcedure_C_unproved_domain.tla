---- MODULE MultipleProcedure_C_unproved_domain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in Nat)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
