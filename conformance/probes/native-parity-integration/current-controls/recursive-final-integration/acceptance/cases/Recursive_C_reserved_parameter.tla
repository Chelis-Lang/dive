---- MODULE Recursive_C_reserved_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(Next=0)

{
F:call First(Next); R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
