---- MODULE Procedure_C_cross_goto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: goto E;
};
{
L: call Put(2); E: skip;
}
} *)
====
