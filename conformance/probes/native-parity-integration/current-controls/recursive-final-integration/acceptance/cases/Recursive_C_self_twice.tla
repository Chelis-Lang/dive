---- MODULE Recursive_C_self_twice ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call First(a-1); }else{ skip; }; R:if(a>0){ call First(a-1); }else{ skip; }; Z:x:=x+1; return;
};
{
L:call First(2); E:skip;
}
} *)
====
