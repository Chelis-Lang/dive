---- MODULE Procedure_C_arity_high ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: return;
};
{
L: call Put(1,2); E: skip;
}
} *)
====
