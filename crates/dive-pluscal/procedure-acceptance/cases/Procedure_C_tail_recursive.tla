---- MODULE Procedure_C_tail_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: call Put(a); return;
};
{
L: call Put(2); E: skip;
}
} *)
====
