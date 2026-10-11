---- MODULE MultipleProcedure_C_membership_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)
variable y \in {1,2};
{ L:call First(1); E:skip; }
process (Two \in {2})
variable z \in {1,2};
{ M:call First(2); N:skip; }
} *)
====
