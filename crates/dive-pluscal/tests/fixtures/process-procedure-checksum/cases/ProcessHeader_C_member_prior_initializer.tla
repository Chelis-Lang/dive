---- MODULE ProcessHeader_C_member_prior_initializer ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variables u=a,v=u+1;
{ F:x:=v; R:return; };
process (Worker \in {1,2})
{ L:call First(2); E:skip; }

} *)
====
