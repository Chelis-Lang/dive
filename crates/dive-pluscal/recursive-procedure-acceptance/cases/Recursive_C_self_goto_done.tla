---- MODULE Recursive_C_self_goto_done ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call First(a-1); goto Done;
};
{
L:call First(2); E:skip;
}
} *)
====
