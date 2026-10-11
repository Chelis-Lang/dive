---- MODULE Procedure_C_return_same_label ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: x:=a; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
