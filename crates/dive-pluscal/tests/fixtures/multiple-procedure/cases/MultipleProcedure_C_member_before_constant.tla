---- MODULE MultipleProcedure_C_member_before_constant ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {2,3})

{ L:call First(1); E:skip; }
process (Two = 1)

{ M:call First(2); N:skip; }
} *)
====
