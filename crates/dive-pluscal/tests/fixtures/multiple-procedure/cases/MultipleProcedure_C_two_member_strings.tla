---- MODULE MultipleProcedure_C_two_member_strings ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One \in {"z","b"})

{ L:call First(1); E:skip; }
process (Two \in {"a","c"})

{ M:call First(2); N:skip; }
} *)
====
