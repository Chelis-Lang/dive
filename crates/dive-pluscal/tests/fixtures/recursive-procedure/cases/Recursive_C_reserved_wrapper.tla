---- MODULE Recursive_C_reserved_wrapper ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Next(a=0)

{
F:call Next(a); R:return;
};
{
L:call Next(2); E:skip;
}
} *)
====
