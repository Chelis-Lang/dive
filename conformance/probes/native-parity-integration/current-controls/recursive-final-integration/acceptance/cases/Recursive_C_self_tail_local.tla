---- MODULE Recursive_C_self_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+10;
{
F:if(a>0){ call First(a-1); return; }else{ x:=u; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
