---- MODULE ProcessProcedure_C_two_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=a+1)

{ F:x:=a+c; R:return; };

process (Worker = 1)

{ L:call First(2,3); E:skip; }
} *)
====
