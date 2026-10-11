---- MODULE ProcessHeader_C_constant_prior_initializer ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variables u=a,v=u+1;
{ F:x:=v; R:return; };
process (Worker = 1)
{ L:call First(2); E:skip; }

} *)
====
