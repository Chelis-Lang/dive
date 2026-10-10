---- MODULE Procedure_C_return_only ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: return;
};
{
L: call Put(2); E: skip;
}
} *)
====
