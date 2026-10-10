---- MODULE Recursive_C_self_tail_two_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=1)

{
F:if(a>0){ call First(a-1,c+1); return; }else{ x:=c; }; R:return;
};
{
L:call First(2,3); E:skip;
}
} *)
====
