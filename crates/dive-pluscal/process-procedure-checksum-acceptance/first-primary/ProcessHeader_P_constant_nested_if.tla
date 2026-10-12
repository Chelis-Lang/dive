---- MODULE ProcessHeader_P_constant_nested_if ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker = 1
begin
L:if TRUE then call First(2); else x:=0; end if; E:skip;
end process;

end algorithm; *)
====
