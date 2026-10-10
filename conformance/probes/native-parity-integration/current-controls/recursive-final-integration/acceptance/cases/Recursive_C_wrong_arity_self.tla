---- MODULE Recursive_C_wrong_arity_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call First(); R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
