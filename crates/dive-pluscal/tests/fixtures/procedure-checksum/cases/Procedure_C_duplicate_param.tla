---- MODULE Procedure_C_duplicate_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0,a=1)

{
P: x:=a; return;
};
{
L: call Put(2,3); E: skip;
}
} *)
====
