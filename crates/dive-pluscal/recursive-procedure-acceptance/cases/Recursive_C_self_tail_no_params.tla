---- MODULE Recursive_C_self_tail_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{
F:call First(); return;
};
{
L:call First(); E:skip;
}
} *)
====
