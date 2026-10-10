---- MODULE Recursive_C_fair_self ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call First(a-1); }else{ x:=a; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
