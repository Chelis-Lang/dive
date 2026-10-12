---- MODULE Procedure_C_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: either{x:=a;}or{x:=1;}; return;
};
{
L: call Put(2); E: skip;
}
} *)
====
