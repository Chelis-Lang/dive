---- MODULE Procedure_C_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: with(b=a+1){ x:=b; }; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
