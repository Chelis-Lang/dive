---- MODULE Recursive_C_duplicate_scope ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(a=1)

{
S:call First(a); T:return;
};
{
L:call First(2); E:skip;
}
} *)
====
