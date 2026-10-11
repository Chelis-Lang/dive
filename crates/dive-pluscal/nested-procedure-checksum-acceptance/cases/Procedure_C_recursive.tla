---- MODULE Procedure_C_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: call Put(a); R: return;
};
{
L: call Put(2); E: skip;
}
} *)
====
