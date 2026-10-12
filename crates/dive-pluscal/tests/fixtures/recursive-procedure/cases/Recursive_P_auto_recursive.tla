---- MODULE Recursive_P_auto_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
if a>0 then call First(a-1); else x:=a; end if; return;
end procedure;
begin
call First(2); skip;
end algorithm; *)
====
