---- MODULE Procedure_C_loop ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: while(a<3){ a:=a+1; }; R: x:=a; return;
};
{
L: call Put(0); E: skip;
}
} *)
====
