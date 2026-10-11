---- MODULE Procedure_C_unused_invalid ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: x:=Missing; return;
};
{
L: skip;
}
} *)
====
