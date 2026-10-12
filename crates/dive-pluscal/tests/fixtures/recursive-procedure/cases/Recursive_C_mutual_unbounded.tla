---- MODULE Recursive_C_mutual_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a+1); R:return;
};
procedure Second(b=0)

{
S:call First(b+1); T:return;
};
{
L:call First(2); E:skip;
}
} *)
====
