---- MODULE Recursive_C_self_tail_counter_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call First(a+1); return;
};
{
L:call First(2); E:skip;
}
} *)
====
