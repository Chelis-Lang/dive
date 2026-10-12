---- MODULE Recursive_C_self_tail_changed_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:a:=a-1; call First(a); return;
};
{
L:call First(2); E:skip;
}
} *)
====
