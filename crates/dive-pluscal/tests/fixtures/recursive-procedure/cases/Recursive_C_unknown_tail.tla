---- MODULE Recursive_C_unknown_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Missing(a); return;
};
{
L:call First(2); E:skip;
}
} *)
====
