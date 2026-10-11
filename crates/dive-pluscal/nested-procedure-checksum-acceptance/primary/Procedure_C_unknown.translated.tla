---- MODULE Procedure_C_unknown ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: return;
};
{
L: call Missing(2); E: skip;
}
} *)
====
