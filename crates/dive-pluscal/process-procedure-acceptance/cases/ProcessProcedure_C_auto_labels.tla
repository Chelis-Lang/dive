---- MODULE ProcessProcedure_C_auto_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ x:=a; return; };

process (Worker = 1)

{ call First(2); skip; }
} *)
====
