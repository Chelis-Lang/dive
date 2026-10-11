---- MODULE Procedure_C_arity_low ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: return;
};
{
L: call Put(); E: skip;
}
} *)
====
