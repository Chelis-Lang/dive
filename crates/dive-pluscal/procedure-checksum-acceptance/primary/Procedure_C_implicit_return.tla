---- MODULE Procedure_C_implicit_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: x:=a;
};
{
L: call Put(2); E: skip;
}
} *)
====
