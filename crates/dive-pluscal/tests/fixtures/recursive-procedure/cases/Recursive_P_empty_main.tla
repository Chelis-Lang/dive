---- MODULE Recursive_P_empty_main ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:if a>0 then call First(a-1); else x:=a; end if; R:return;
end procedure;
begin

end algorithm; *)
====
