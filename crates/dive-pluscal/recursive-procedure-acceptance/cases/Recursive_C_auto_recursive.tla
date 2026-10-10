---- MODULE Recursive_C_auto_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
if(a>0){ call First(a-1); }else{ x:=a; }; return;
};
{
call First(2); skip;
}
} *)
====
