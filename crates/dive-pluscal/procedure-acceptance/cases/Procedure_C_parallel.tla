---- MODULE Procedure_C_parallel ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0,b=1)

{
P: a:=b || b:=a; R: x:=a; return;
};
{
L: call Put(2,3); E: skip;
}
} *)
====
