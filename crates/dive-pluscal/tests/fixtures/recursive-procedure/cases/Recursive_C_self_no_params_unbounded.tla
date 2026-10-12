---- MODULE Recursive_C_self_no_params_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{
F:call First(); R:return;
};
{
L:call First(); E:skip;
}
} *)
====
