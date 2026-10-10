---- MODULE Recursive_C_unused_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:x:=a; R:return;
};
procedure Second(b=0)

{
S:call Second(b); T:return;
};
{
L:call First(2); E:skip;
}
} *)
====
