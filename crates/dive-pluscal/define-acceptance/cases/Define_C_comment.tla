---- MODULE Define_C_comment ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A {
 variables x=0, y=0;
 define {
 F == 1 \* comment
 G == F+1
 }
{ L: x := G; }
}
*)
====
