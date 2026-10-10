---- MODULE Procedure_C_source_assert ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: assert a>0; x:=a; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
