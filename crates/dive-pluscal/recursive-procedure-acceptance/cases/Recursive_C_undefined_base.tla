---- MODULE Recursive_C_undefined_base ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call First(a-1); }else{ x:=Missing; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
====
