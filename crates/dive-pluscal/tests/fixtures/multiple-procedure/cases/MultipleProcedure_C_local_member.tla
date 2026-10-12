---- MODULE MultipleProcedure_C_local_member ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u \in {1,2};
{ F:x:=u; R:return; };
process (One = 1)

{ L:call First(1); E:skip; }
process (Two = 2)

{ M:call First(2); N:skip; }
} *)
====
