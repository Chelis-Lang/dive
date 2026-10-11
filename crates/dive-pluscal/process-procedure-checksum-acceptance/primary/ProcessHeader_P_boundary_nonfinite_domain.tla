---- MODULE ProcessHeader_P_boundary_nonfinite_domain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; R:return;
end procedure;
process Worker \in Nat
begin
L:call First(2); E:skip;
end process;

end algorithm; *)
====
