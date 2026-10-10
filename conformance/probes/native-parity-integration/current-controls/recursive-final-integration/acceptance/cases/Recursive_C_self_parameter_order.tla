---- MODULE Recursive_C_self_parameter_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=a+1)

{
F:if(a>0){ call First(a-1,c+1); }else{ x:=c; }; R:return;
};
{
L:call First(2,3); E:skip;
}
} *)
====
