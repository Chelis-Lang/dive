---- MODULE ProcessHeader_C_constant_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=1;
{ F:x:=u+a; R:return; };
process (Worker = 1)
{ L:call First(2); E:skip; }

} *)
====
