---- MODULE Procedure_C_reserved_stack ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(stack=0)

{
P: return;
};
{
L: call Put(2); E: skip;
}
} *)
====
