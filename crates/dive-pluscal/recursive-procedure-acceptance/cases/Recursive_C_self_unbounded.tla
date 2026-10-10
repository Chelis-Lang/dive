---- MODULE Recursive_C_self_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call First(a+1); R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
