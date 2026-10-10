---- MODULE Recursive_C_self_base_negative ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call First(a-1); }else{ x:=x+1; }; R:return;
};
{
L:call First(0-1); E:skip;
}
} *)
====
